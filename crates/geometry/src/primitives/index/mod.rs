//! The canonical [`Index`] of a primitive table: its edges and the
//! adjacency between points, edges and primitives, the same for every
//! kernel and for Studio's display.
//!
//! Both builds list segments: the one leaving each vertex, to the next
//! vertex of its primitive (wrapping to the first when the primitive is
//! closed; an open primitive's last vertex starts none). Normal surfaces
//! are counting-sorted by low point in parallel ([`bucket`]); very sparse
//! point domains sort their few segments serially ([`sparse`]). Both give
//! identical tables, and their shared [`Tables::finish`] splits each
//! edge's occurrences into polygon sides and line segments (polylines'
//! and curves' hulls).
mod bucket;
mod sparse;
#[cfg(test)]
mod tests;
mod walk;

pub use walk::Walk;

use super::{Edge, Kind, Primitives};
use std::ops;
use std::sync::OnceLock;

pub(super) fn build(t: &Primitives) -> Index {
    if sparse_points(t) {
        sparse::edges(t).finish(t)
    } else {
        bucket::build(t).finish(t)
    }
}

/// How many unique edges, without the rest of the index.
pub(super) fn count(t: &Primitives) -> usize {
    if sparse_points(t) {
        sparse::edges(t).edges.len()
    } else {
        bucket::count(t)
    }
}

/// Whether most point ids are unused: few segments sort faster than
/// point-sized buckets scan.
fn sparse_points(t: &Primitives) -> bool {
    t.points > t.vertices.len().saturating_mul(4)
}

/// A vertex that starts no segment: an open primitive's last.
const NO_EDGE: u32 = u32::MAX;

/// The segment leaving vertex `c` of primitive `prim`, as (low, high)
/// points.
fn segment(t: &Primitives, prim: usize, c: usize) -> Option<(u32, u32)> {
    let next = if c + 1 < t.offsets[prim + 1] as usize {
        c + 1
    } else if t.kind(prim).closed() {
        t.offsets[prim] as usize
    } else {
        return None;
    };
    let (a, b) = (t.vertices[c], t.vertices[next]);
    Some((a.min(b), a.max(b)))
}

/// Rows of ids, one row per point, edge or primitive, stored contiguously.
/// A table with no entries at all stores no rows: every row is empty
/// (the polyline tables of a value without polylines).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Adjacency {
    offsets: Vec<u32>,
    entries: Vec<u32>,
}

impl ops::Index<usize> for Adjacency {
    type Output = [u32];
    #[inline]
    fn index(&self, i: usize) -> &[u32] {
        self.row(i)
    }
}

impl Adjacency {
    /// Rows that are all empty.
    fn none() -> Self {
        Self {
            offsets: Vec::new(),
            entries: Vec::new(),
        }
    }

    /// This table, storing nothing when it has no entries.
    fn compact(self) -> Self {
        if self.entries.is_empty() {
            Self::none()
        } else {
            self
        }
    }

    /// Row `i` (empty in a table storing no rows).
    #[inline]
    pub(crate) fn row(&self, i: usize) -> &[u32] {
        if self.offsets.is_empty() {
            return &[];
        }
        &self.entries[self.offsets[i] as usize..self.offsets[i + 1] as usize]
    }

    /// The rows stored (none for a table without entries).
    pub(crate) fn rows(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    /// Drops repeated entries from each sorted row.
    fn dedup_rows(&mut self) {
        let mut write = 0;
        let mut start = 0;
        for row in 0..self.rows() {
            let end = self.offsets[row + 1] as usize;
            for read in start..end {
                if read == start || self.entries[read] != self.entries[read - 1] {
                    self.entries[write] = self.entries[read];
                    write += 1;
                }
            }
            start = end;
            self.offsets[row + 1] = write as u32;
        }
        self.entries.truncate(write);
    }

    fn bytes(&self) -> usize {
        4 * (self.offsets.capacity() + self.entries.capacity())
    }
}

/// A table's edges and adjacency ([`Primitives::index`]), the one place
/// topology is asked: which elements touch which, a point's neighbours,
/// [edge loops and rings](Self::edge_loop) and
/// [boundaries](Self::boundary_edges). Built once when first asked and
/// shared by every value holding the table, so a question costs what it
/// reads and nothing per frame. Rows are ascending unless said otherwise;
/// primitives are named by their ids. "Lines" are the primitives that
/// aren't polygons: polylines and curves (by their hulls).
///
/// ```
/// use kinogaki_geometry::Primitives;
///
/// // Two quads side by side: 0 1 4 3 and 1 2 5 4.
/// let t = Primitives::of_polygons(6, vec![0, 4, 8].into(), vec![0, 1, 4, 3, 1, 2, 5, 4].into()).unwrap();
/// let index = t.index();
/// assert_eq!(index.point_primitives(1).collect::<Vec<_>>(), [0, 1]);
/// assert_eq!(index.point_neighbours(1).collect::<Vec<_>>(), [0, 2, 4]);
/// let shared = t.edge_of(1, 4).unwrap() as usize;
/// assert_eq!(index.edge_primitives(shared).collect::<Vec<_>>(), [0, 1]);
/// assert!(!index.is_boundary(shared) && index.boundary_edges().len() == 6);
/// ```
#[derive(Debug)]
pub struct Index {
    edges: Vec<Edge>,
    edge_faces: Adjacency,
    edge_lines: Adjacency,
    primitive_edges: Adjacency,
    point_edges: Adjacency,
    point_line_edges: Adjacency,
    point_faces: Adjacency,
    point_lines: Adjacency,
    boundary: OnceLock<Vec<u32>>,
}

impl Index {
    /// Every primitive's segments, once each, in canonical order
    /// ([`Primitives::edges`]). Disjoint point ranges keep their own
    /// edges' order when concatenated.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// The id of the edge joining `a` and `b`, either way round.
    pub fn edge_of(&self, a: u32, b: u32) -> Option<u32> {
        let key = Edge {
            points: [a.min(b), a.max(b)],
        };
        self.edges.binary_search(&key).ok().map(|e| e as u32)
    }

    /// Per edge, the polygons with it as a side (empty for an edge only
    /// lines use).
    pub fn edge_faces(&self) -> &Adjacency {
        &self.edge_faces
    }

    /// Per edge, the lines with a segment on it, once each.
    pub fn edge_lines(&self) -> &Adjacency {
        &self.edge_lines
    }

    /// Primitive `p`'s edges, in vertex order: the edge leaving each
    /// vertex (a closed primitive's last to its first; an open one has one
    /// fewer than its vertices).
    pub fn primitive_edges(&self, p: usize) -> &[u32] {
        &self.primitive_edges[p]
    }

    /// Per point, the edges polygons use (line segments along a polygon
    /// side included).
    pub fn point_edges(&self) -> &Adjacency {
        &self.point_edges
    }

    /// Per point, the polygons through it, each once.
    pub fn point_faces(&self) -> &Adjacency {
        &self.point_faces
    }

    /// The primitives on edge `e`: polygons, then lines.
    pub fn edge_primitives(&self, e: usize) -> impl Iterator<Item = u32> + '_ {
        self.edge_faces[e]
            .iter()
            .chain(&self.edge_lines[e])
            .copied()
    }

    /// The primitives through point `p`: polygons, then lines.
    pub fn point_primitives(&self, p: usize) -> impl Iterator<Item = u32> + '_ {
        self.point_faces[p]
            .iter()
            .chain(&self.point_lines[p])
            .copied()
    }

    /// Every edge at point `p`: those polygons use, then those only lines
    /// use.
    pub fn point_every_edge(&self, p: usize) -> impl Iterator<Item = u32> + '_ {
        self.point_edges[p]
            .iter()
            .chain(&self.point_line_edges[p])
            .copied()
    }

    /// The points one edge from point `p`, each once, in the order of
    /// [`point_every_edge`](Self::point_every_edge).
    pub fn point_neighbours(&self, p: usize) -> impl Iterator<Item = u32> + '_ {
        self.point_every_edge(p).map(move |e| {
            let [a, b] = self.edges[e as usize].points;
            if a as usize == p { b } else { a }
        })
    }

    /// Whether edge `e` is a side of exactly one polygon: an edge of the
    /// surface's border.
    pub fn is_boundary(&self, e: usize) -> bool {
        self.edge_faces[e].len() == 1
    }

    /// Every [boundary](Self::is_boundary) edge, ascending. Found once.
    pub fn boundary_edges(&self) -> &[u32] {
        self.boundary.get_or_init(|| {
            (0..self.edges.len() as u32)
                .filter(|&e| self.is_boundary(e as usize))
                .collect()
        })
    }

    /// The bytes it holds, for memory accounting.
    pub fn bytes(&self) -> usize {
        self.boundary.get().map_or(0, |b| b.capacity() * 4)
            + self.edges.capacity() * 8
            + self.edge_faces.bytes()
            + self.edge_lines.bytes()
            + self.primitive_edges.bytes()
            + self.point_edges.bytes()
            + self.point_line_edges.bytes()
            + self.point_faces.bytes()
            + self.point_lines.bytes()
    }
}

/// Edges and their segment occurrences, before the polygon/line split.
struct Tables {
    edges: Vec<Edge>,
    /// Edge `e`'s occurrences are `offsets[e]..offsets[e + 1]`, ascending
    /// by vertex.
    offsets: Vec<u32>,
    /// The primitive and starting vertex of each occurrence.
    prims: Vec<u32>,
    vertices: Vec<u32>,
    /// The edge each vertex starts, or [`NO_EDGE`].
    vertex_edges: Vec<u32>,
    /// The primitive of each vertex.
    prim_of: Vec<u32>,
}

impl Tables {
    fn finish(self, t: &Primitives) -> Index {
        let polygons_only = t.uniform() == Some(Kind::POLYGON);
        let is_polygon = |p: u32| polygons_only || t.kind(p as usize).is_polygon();
        let edge_count = self.edges.len();
        let (edge_faces, edge_lines) = if polygons_only {
            let faces = Adjacency {
                offsets: self.offsets,
                entries: self.prims,
            };
            (faces, Adjacency::none())
        } else {
            let (faces, lines) = split(&self.offsets, &self.prims, is_polygon);
            (faces, lines.compact())
        };
        // Each primitive's segments: its vertices' edges, an open one's
        // last vertex starting none.
        let primitive_edges = if polygons_only {
            Adjacency {
                offsets: t.offsets.to_vec(),
                entries: self.vertex_edges,
            }
        } else {
            let mut offsets = Vec::with_capacity(t.len() + 1);
            let mut open = 0u32;
            offsets.push(0);
            for p in 0..t.len() {
                open += u32::from(!t.kind(p).closed() && !t.primitive(p).is_empty());
                offsets.push(t.offsets[p + 1] - open);
            }
            let entries = (self.vertex_edges.into_iter())
                .filter(|&e| e != NO_EDGE)
                .collect();
            Adjacency { offsets, entries }
        };
        let (edges, prim_of) = (self.edges, self.prim_of);
        // Point rows hold the surface's edges; edges only lines use are
        // listed apart, so polygon tools see the valence a surface has.
        let line_only = |e: usize| edge_faces.offsets[e] == edge_faces.offsets[e + 1];
        let point_edges = if polygons_only {
            bucket::adjacency(t.points, edge_count, |e| {
                edges[e].points.map(|p| (p, e as u32))
            })
        } else {
            let surface: Vec<u32> = (0..edge_count as u32)
                .filter(|&e| !line_only(e as usize))
                .collect();
            bucket::adjacency(t.points, surface.len(), |i| {
                let e = surface[i];
                edges[e as usize].points.map(|p| (p, e))
            })
        };
        let line_edges: Vec<u32> = match polygons_only {
            true => Vec::new(),
            false => (0..edge_count as u32)
                .filter(|&e| line_only(e as usize))
                .collect(),
        };
        let point_line_edges = bucket::adjacency(t.points, line_edges.len(), |i| {
            let e = line_edges[i];
            edges[e as usize].points.map(|p| (p, e))
        })
        .compact();
        let (point_faces, point_lines) = if polygons_only {
            let faces = bucket::adjacency(t.points, t.vertices.len(), |c| {
                [(t.vertices[c], prim_of[c])]
            });
            (faces, Adjacency::none())
        } else {
            let (on_faces, on_lines): (Vec<u32>, Vec<u32>) =
                (0..t.vertices.len() as u32).partition(|&c| is_polygon(prim_of[c as usize]));
            let rows = |of: &[u32]| {
                bucket::adjacency(t.points, of.len(), |i| {
                    let c = of[i] as usize;
                    [(t.vertices[c], prim_of[c])]
                })
            };
            let mut lines = rows(&on_lines);
            lines.dedup_rows();
            (rows(&on_faces), lines.compact())
        };
        Index {
            edges,
            edge_faces,
            edge_lines,
            primitive_edges,
            point_edges,
            point_line_edges,
            point_faces,
            point_lines,
            boundary: OnceLock::new(),
        }
    }
}

/// Each edge's occurrences split by their primitive into polygon sides
/// (faces) and line segments (primitive ids, once each).
fn split(
    offsets: &[u32],
    prims: &[u32],
    is_polygon: impl Fn(u32) -> bool,
) -> (Adjacency, Adjacency) {
    let mut faces = Adjacency {
        offsets: Vec::with_capacity(offsets.len()),
        entries: Vec::with_capacity(prims.len()),
    };
    let mut lines = Adjacency {
        offsets: Vec::with_capacity(offsets.len()),
        entries: Vec::new(),
    };
    faces.offsets.push(0);
    lines.offsets.push(0);
    for w in offsets.windows(2) {
        let first = lines.entries.len();
        for &p in &prims[w[0] as usize..w[1] as usize] {
            if is_polygon(p) {
                faces.entries.push(p);
            } else if !lines.entries[first..].contains(&p) {
                // A line crossing the same edge twice is listed once.
                lines.entries.push(p);
            }
        }
        faces.offsets.push(faces.entries.len() as u32);
        lines.offsets.push(lines.entries.len() as u32);
    }
    (faces, lines)
}
