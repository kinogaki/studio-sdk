//! Geometry as triangles: the display [`Triangulation`] every viewport,
//! render engine and kernel shares. Ears are clipped; a polygon that
//! doesn't clip shows as a fan.
mod clip;

use crate::column::Column;
use crate::geometry::Geometry;
use crate::kept::Kept;
use crate::parallel;
use crate::primitives::{Kind, Primitives};
use crate::vector::V3;
use clip::{Outline, clip};
use std::ops::Range;
use std::sync::Arc;

/// Geometry per parallel block.
const BLOCK: usize = 8192;

/// One triangle of a polygon: its corners as polygon-vertex ids (the
/// polygon's first vertex id plus the corner's place in it), their points,
/// and the polygon. It winds as the polygon does, so its normal agrees
/// with the polygon's area normal.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Triangle {
    /// The polygon vertices at its corners.
    pub vertices: [u32; 3],
    /// The points at its corners.
    pub points: [u32; 3],
    /// The polygon it is part of.
    pub primitive: u32,
}

/// A polygons value's **display triangulation**: every polygon as exactly
/// n − 2 triangles (other primitives have none), in primitive order.
///
/// Each polygon is ear clipped in the plane facing it most. Where that
/// fails it is clipped again tolerantly: outlines whose points coincide
/// within rounding or run straight, as cuts make them, still triangulate
/// exactly. A polygon that can't be clipped either way (a crossing or
/// collapsed outline) is shown as a fan from its first vertex and listed
/// in [`fans`](Self::fans). So the triangle layout depends on the topology
/// alone: polygon `p`'s triangles are [`of(p)`](Self::of).
///
/// It is the one triangulation the viewport, render engines and kernels
/// show polygons with, so Solid and Render views always agree.
#[derive(Clone, Debug, PartialEq)]
pub struct Triangulation {
    triangles: Column<[u32; 3]>,
    corners: Column<[u32; 3]>,
    primitives: Column<u32>,
    fans: Column<u32>,
    /// Where each polygon's triangles start.
    layout: Layout,
}

/// Where each primitive's triangles start: from the table's own offsets
/// (shared) when every primitive is a polygon, else a column of its own.
#[derive(Clone, Debug, PartialEq)]
enum Layout {
    /// Every primitive is a polygon; these are the table's offsets.
    Geometry(Column<u32>),
    /// Each primitive's first triangle, and the triangle count last.
    Each(Column<u32>),
}

impl Layout {
    fn of(t: &Primitives) -> Layout {
        if t.uniform() == Some(Kind::POLYGON) {
            return Layout::Geometry(t.offsets().clone());
        }
        let mut first = Vec::with_capacity(t.len() + 1);
        let mut total = 0u32;
        first.push(0);
        for p in 0..t.len() {
            if t.kind(p).is_polygon() {
                total += t.primitive(p).len() as u32 - 2;
            }
            first.push(total);
        }
        Layout::Each(first.into())
    }

    /// How many triangles the primitives before `p` have.
    fn before(&self, p: usize) -> usize {
        match self {
            Layout::Geometry(offsets) => offsets[p] as usize - 2 * p,
            Layout::Each(first) => first[p] as usize,
        }
    }

    /// How many primitives it places.
    fn primitives(&self) -> usize {
        match self {
            Layout::Geometry(offsets) | Layout::Each(offsets) => offsets.len() - 1,
        }
    }
}

impl Triangulation {
    /// Each triangle's corner points.
    pub fn triangles(&self) -> &Column<[u32; 3]> {
        &self.triangles
    }

    /// Each triangle's corner polygon vertices (what per-vertex values,
    /// such as shading normals and texture coordinates, are read through).
    pub fn corners(&self) -> &Column<[u32; 3]> {
        &self.corners
    }

    /// Each triangle's polygon.
    pub fn primitives(&self) -> &Column<u32> {
        &self.primitives
    }

    /// Polygon `polygon`'s triangles: its n − 2, wherever its outline
    /// moves. Empty for another primitive or one out of range.
    pub fn of(&self, polygon: usize) -> Range<usize> {
        if polygon >= self.layout.primitives() {
            return 0..0;
        }
        self.layout.before(polygon)..self.layout.before(polygon + 1)
    }

    /// The polygons shown as a fan (approximately), in order: their
    /// outlines cross or collapse. For checks that flag damaged polygons.
    pub fn fans(&self) -> &Column<u32> {
        &self.fans
    }

    /// How many triangles.
    pub fn len(&self) -> usize {
        self.triangles.len()
    }

    /// Whether it has none.
    pub fn is_empty(&self) -> bool {
        self.triangles.is_empty()
    }
}

/// What a positions column keeps: the triangulation over each topology.
type Triangulations = Kept<Arc<Primitives>, Triangulation>;

impl Geometry {
    /// The display [`Triangulation`]. It is derived from the topology and
    /// the positions and kept on the positions column, so asking again
    /// costs nothing; a value with other positions triangulates anew
    /// (after a drag, [`triangulation_after`](Self::triangulation_after)
    /// does so for the moved polygons alone).
    pub fn triangulation(&self) -> Arc<Triangulation> {
        let kept = self.positions().derived(|_| Triangulations::default());
        if let Some(t) = kept.find(|k| Arc::ptr_eq(k, self.primitives())) {
            return t;
        }
        let built = Arc::new(build(self));
        kept.keep(self.primitives().clone(), built.clone());
        built
    }

    /// The same as [`triangulation`](Self::triangulation), made from
    /// `previous`, `before`'s: only the polygons through the points `moved`
    /// (which must include every point whose position differs) and the rows
    /// that differ between the two tables are clipped again, the other
    /// triangles stay and their cells stay shared. Built whole when most rows
    /// differ.
    pub fn triangulation_after(
        &self,
        before: &Geometry,
        previous: &Triangulation,
        moved: &[u32],
    ) -> Arc<Triangulation> {
        let kept = self.positions().derived(|_| Triangulations::default());
        if let Some(t) = kept.find(|k| Arc::ptr_eq(k, self.primitives())) {
            return t;
        }
        let built = Arc::new(rebuilt(self, before.primitives(), previous, moved));
        kept.keep(self.primitives().clone(), built.clone());
        built
    }
}

fn outline(topology: &Primitives, polygon: usize) -> Outline<'_> {
    Outline {
        points: topology.primitive(polygon),
        first: topology.offsets()[polygon],
        primitive: polygon as u32,
    }
}

/// Polygon `polygon` for display, appended to `out`: strictly, else
/// tolerantly, else as a fan. Whether it fell back to the fan.
fn display(
    topology: &Primitives,
    polygon: usize,
    position: &impl Fn(u32) -> V3,
    out: &mut Vec<Triangle>,
) -> bool {
    let o = outline(topology, polygon);
    let start = out.len();
    if clip(&o, position, false, out).is_ok() {
        return false;
    }
    out.truncate(start);
    if clip(&o, position, true, out).is_ok() {
        return false;
    }
    out.truncate(start);
    for i in 1..o.points.len() - 1 {
        out.push(Triangle {
            vertices: [o.first, o.first + i as u32, o.first + i as u32 + 1],
            points: [o.points[0], o.points[i], o.points[i + 1]],
            primitive: o.primitive,
        });
    }
    true
}

/// The triangles of the polygons among `primitives` (a range of
/// primitive ids), in order, with the fans among them.
fn triangles_of(
    value: &Geometry,
    layout: &Layout,
    primitives: Range<usize>,
) -> (Vec<Triangle>, Vec<u32>) {
    let topology = value.primitives();
    let positions = value.positions();
    let position = |p: u32| positions[p as usize];
    let count = layout.before(primitives.end) - layout.before(primitives.start);
    let mut out = Vec::with_capacity(count);
    let mut fans = Vec::new();
    for p in primitives.filter(|&p| topology.kind(p).is_polygon()) {
        if display(topology, p, &position, &mut out) {
            fans.push(p as u32);
        }
    }
    (out, fans)
}

/// The triangulation of every polygon, in parallel blocks.
fn build(value: &Geometry) -> Triangulation {
    let layout = Layout::of(value.primitives());
    let blocks = parallel::blocks(value.primitives().len(), BLOCK, |r| {
        triangles_of(value, &layout, r)
    });
    let total = blocks.iter().map(|(t, _)| t.len()).sum();
    let (mut triangles, mut corners, mut primitives) = (
        Vec::with_capacity(total),
        Vec::with_capacity(total),
        Vec::with_capacity(total),
    );
    let mut fans = Vec::new();
    for (block, block_fans) in blocks {
        for t in block {
            triangles.push(t.points);
            corners.push(t.vertices);
            primitives.push(t.primitive);
        }
        fans.extend(block_fans);
    }
    Triangulation {
        triangles: triangles.into(),
        corners: corners.into(),
        primitives: primitives.into(),
        fans: fans.into(),
        layout,
    }
}

/// `previous` (over the table `was`) with the rows that differ and the
/// polygons through `moved` clipped again at `value`'s positions, the rows
/// from the [tail](crate::TableChange::tail) on appended. A polygon below the
/// tail keeps its place, its triangles written only when they differ, so a
/// drag that changes no triangle changes no triangle column.
fn rebuilt(
    value: &Geometry,
    was: &Primitives,
    previous: &Triangulation,
    moved: &[u32],
) -> Triangulation {
    let topology = value.primitives();
    let change = topology.changes_from(was);
    let tail = change.tail as usize;
    if change.rewired.len() + topology.len() - tail > topology.len() / 2 + BLOCK {
        return build(value);
    }
    let index = topology.index();
    let mut polygons: Vec<u32> = moved
        .iter()
        .filter(|&&p| (p as usize) < topology.points())
        .flat_map(|&p| index.point_faces(p as usize).iter().copied())
        .chain(change.rewired)
        .filter(|&f| (f as usize) < tail && topology.kind(f as usize).is_polygon())
        .collect();
    polygons.sort_unstable();
    polygons.dedup();
    let layout = Layout::of(topology);
    let first = layout.before(tail);
    let mut changed: Vec<(usize, Triangle)> = Vec::new();
    let mut fans: Vec<u32> = (previous.fans.iter().copied())
        .filter(|&f| (f as usize) < tail && polygons.binary_search(&f).is_err())
        .collect();
    for &p in &polygons {
        let (one, fanned) = triangles_of(value, &layout, p as usize..p as usize + 1);
        fans.extend(fanned);
        let at = layout.before(p as usize);
        changed.extend((at..).zip(one).filter(|(i, t)| {
            previous.triangles[*i] != t.points || previous.corners[*i] != t.vertices
        }));
    }
    let (appended, tail_fans) = triangles_of(value, &layout, tail..topology.len());
    fans.extend(tail_fans);
    fans.sort_unstable();
    let shrinks = previous.len() != first;
    let (triangles, corners) = if changed.is_empty() && appended.is_empty() && !shrinks {
        (previous.triangles.clone(), previous.corners.clone())
    } else {
        let mut triangles = previous.triangles.edit();
        let mut corners = previous.corners.edit();
        triangles.truncate(first);
        corners.truncate(first);
        for (i, t) in changed {
            triangles[i] = t.points;
            corners[i] = t.vertices;
        }
        for t in &appended {
            triangles.push(t.points);
            corners.push(t.vertices);
        }
        (triangles.finish(), corners.finish())
    };
    let primitives = if appended.is_empty() && !shrinks {
        previous.primitives.clone()
    } else {
        let mut primitives = previous.primitives.edit();
        primitives.truncate(first);
        appended.iter().for_each(|t| primitives.push(t.primitive));
        primitives.finish()
    };
    Triangulation {
        triangles,
        corners,
        primitives,
        fans: fans.into(),
        layout,
    }
}
