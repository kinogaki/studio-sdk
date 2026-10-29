//! What the viewport draws and picks: a value's display [`Surface`],
//! component [`Level`]s, sets of packet [`Elements`] and pick [`Hit`]s.
//!
//! Studio's core owns the viewport (panes, cameras, drawing, picking). A
//! data type only says what to draw, as a [`Surface`] built by its
//! [`Displayable`](crate::Displayable) facet, and what a pick on it means,
//! through its [`Pickable`](crate::Pickable) facet. The host uploads a
//! surface's shared arrays as they are and, for a surface
//! [`derived`](Surface::derived) from the one on screen, only the points
//! that moved.
use crate::math::Vec3;
use crate::render::Ranges;
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::sync::Arc;

/// What a component selection or a pick addresses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Level {
    /// Points (a surface's markers).
    Point,
    /// Edges (a surface's line segments).
    Edge,
    /// Faces (a surface's corners' faces).
    Face,
    /// Polygon vertices (a surface's corners).
    Vertex,
    /// Whole objects.
    Object,
}

/// One corner of a polygon (a polygon vertex): the point it sits on, the
/// face it belongs to, and its shading normal.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Corner {
    /// Its point: an index into [`Surface::points`].
    pub point: u32,
    /// Its face, in the data type's face numbering (what face picks and
    /// face highlights name).
    pub face: u32,
    /// Its shading normal, unit length.
    pub normal: [f32; 3],
}

/// A value as the viewport draws it: immutable, its arrays shared.
///
/// - **Points** are positions in the value's own space; every other array
///   indexes them.
/// - **Corners** are polygon vertices; **triangles** index corners, so a
///   polygon's triangles share its corners' face and normals.
/// - **Lines** are segments, two line vertices each, a line vertex
///   `[point, edge id]` with the same edge id at both ends: what edge
///   picks and edge highlights name.
/// - **Markers** are drawn points, each `[point, point id]`: what point
///   picks name.
///
/// Its identity is the allocation of a token made with it: a clone shares
/// it, [`derived`](Self::derived) makes a new one.
#[derive(Clone)]
pub struct Surface {
    points: Arc<[[f32; 3]]>,
    corners: Arc<[Corner]>,
    triangles: Arc<[[u32; 3]]>,
    faces: usize,
    lines: Arc<[[u32; 2]]>,
    markers: Arc<[[u32; 2]]>,
    topology: u64,
    identity: Arc<()>,
    /// The surface this one was derived from and the points that moved
    /// since.
    parent: Option<(Arc<()>, Ranges)>,
}

impl Surface {
    /// A surface of `points`, `corners` and `triangles` (corner indices)
    /// with `faces` faces, and no lines or markers. `topology` is the
    /// type's key for the connectivity (corners, triangles, lines and
    /// markers): equal keys mean equal connectivity, so the host keeps its
    /// buffers.
    pub fn new(
        points: Arc<[[f32; 3]]>,
        corners: Arc<[Corner]>,
        triangles: Arc<[[u32; 3]]>,
        faces: usize,
        topology: u64,
    ) -> Self {
        Self {
            points,
            corners,
            triangles,
            faces,
            lines: Arc::from([]),
            markers: Arc::from([]),
            topology,
            identity: Arc::new(()),
            parent: None,
        }
    }

    /// Flat-shaded `polygons` over `points`: each polygon a face (its
    /// index the face number), its corners sharing the polygon's normal,
    /// fanned into triangles from its first corner, and every distinct
    /// polygon edge a line (edge ids in first-seen order). `topology` is as
    /// in [`new`](Self::new). Polygons are convex, wound counterclockwise
    /// seen from outside, and index `points`; a degenerate polygon's
    /// corners face +Z.
    ///
    /// ```
    /// use kinogaki_studio_api::Surface;
    ///
    /// let square = [[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]];
    /// let surface = Surface::polygons(square, [[0, 1, 2, 3]], 1);
    /// assert_eq!((surface.faces(), surface.triangles().len()), (1, 2));
    /// assert_eq!(surface.lines().len(), 8); // four edges, two vertices each
    /// assert_eq!(surface.corners()[0].normal, [0.0, 0.0, 1.0]);
    /// ```
    ///
    /// # Panics
    ///
    /// When a polygon indexes past `points`.
    pub fn polygons<P: AsRef<[u32]>>(
        points: impl Into<Arc<[[f32; 3]]>>,
        polygons: impl IntoIterator<Item = P>,
        topology: u64,
    ) -> Self {
        let points = points.into();
        let mut corners = Vec::new();
        let mut triangles = Vec::new();
        let mut edges = BTreeMap::new();
        let mut lines = Vec::new();
        let mut faces = 0;
        for (face, polygon) in polygons.into_iter().enumerate() {
            let polygon = polygon.as_ref();
            let normal = newell_normal(&points, polygon);
            let first = corners.len() as u32;
            corners.extend(polygon.iter().map(|&point| Corner {
                point,
                face: face as u32,
                normal,
            }));
            for i in 1..polygon.len().saturating_sub(1) as u32 {
                triangles.push([first, first + i, first + i + 1]);
            }
            for (i, &a) in polygon.iter().enumerate() {
                let b = polygon[(i + 1) % polygon.len()];
                let next = edges.len() as u32;
                let id = *edges.entry((a.min(b), a.max(b))).or_insert(next);
                if id == next {
                    lines.extend([[a, id], [b, id]]);
                }
            }
            faces += 1;
        }
        Self::new(points, corners.into(), triangles.into(), faces, topology)
            .with_lines(lines.into())
    }

    /// With `lines`: line vertices, two a segment.
    pub fn with_lines(mut self, lines: Arc<[[u32; 2]]>) -> Self {
        debug_assert!(lines.len().is_multiple_of(2), "two line vertices a segment");
        self.lines = lines;
        self
    }

    /// With point `markers`.
    pub fn with_markers(mut self, markers: Arc<[[u32; 2]]>) -> Self {
        self.markers = markers;
        self
    }

    /// A surface like this one with `points` (as many) of which those in
    /// `moved` changed: a drag's next frame. The host patches only those
    /// points over this surface's upload. Corners keep their normals.
    pub fn derived(&self, points: Arc<[[f32; 3]]>, moved: Ranges) -> Self {
        debug_assert_eq!(points.len(), self.points.len());
        Self {
            points,
            identity: Arc::new(()),
            parent: Some((self.identity.clone(), moved)),
            ..self.clone()
        }
    }

    /// The point positions.
    pub fn points(&self) -> &Arc<[[f32; 3]]> {
        &self.points
    }

    /// The polygon corners.
    pub fn corners(&self) -> &Arc<[Corner]> {
        &self.corners
    }

    /// The triangles, three corner indices each.
    pub fn triangles(&self) -> &Arc<[[u32; 3]]> {
        &self.triangles
    }

    /// How many faces.
    pub fn faces(&self) -> usize {
        self.faces
    }

    /// The line vertices, two a segment.
    pub fn lines(&self) -> &Arc<[[u32; 2]]> {
        &self.lines
    }

    /// The point markers.
    pub fn markers(&self) -> &Arc<[[u32; 2]]> {
        &self.markers
    }

    /// The connectivity key.
    pub fn topology(&self) -> u64 {
        self.topology
    }

    /// The surface's identity.
    pub fn identity(&self) -> &Arc<()> {
        &self.identity
    }

    /// The points moved since the surface with identity `previous`, when
    /// this surface was [`derived`](Self::derived) from it.
    pub fn moved_since(&self, previous: &Arc<()>) -> Option<&Ranges> {
        self.parent
            .as_ref()
            .filter(|(parent, _)| Arc::ptr_eq(parent, previous))
            .map(|(_, moved)| moved)
    }

    /// The surface it was derived from and the points moved since, when it
    /// was.
    pub fn parent(&self) -> Option<(&Arc<()>, &Ranges)> {
        self.parent.as_ref().map(|(p, r)| (p, r))
    }

    /// Whether `self` and `other` are the same surface.
    pub fn same(&self, other: &Surface) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }
}

/// `polygon`'s unit normal by Newell's method; +Z when it has no area.
fn newell_normal(points: &[[f32; 3]], polygon: &[u32]) -> [f32; 3] {
    let mut n = [0.0f32; 3];
    for (i, &a) in polygon.iter().enumerate() {
        let p = points[a as usize];
        let q = points[polygon[(i + 1) % polygon.len()] as usize];
        n[0] += (p[1] - q[1]) * (p[2] + q[2]);
        n[1] += (p[2] - q[2]) * (p[0] + q[0]);
        n[2] += (p[0] - q[0]) * (p[1] + q[1]);
    }
    let length = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
    if length > 0.0 {
        n.map(|c| c / length)
    } else {
        [0.0, 0.0, 1.0]
    }
}

impl fmt::Debug for Surface {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Surface")
            .field("points", &self.points.len())
            .field("triangles", &self.triangles.len())
            .field("faces", &self.faces)
            .field("segments", &(self.lines.len() / 2))
            .field("markers", &self.markers.len())
            .finish()
    }
}

/// A set of surface elements at one level: ids sorted and unique, or all
/// of them. Ids are face numbers, edge ids, point ids or corner indices,
/// as [`Surface`] describes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Elements {
    /// What the ids are.
    pub level: Level,
    /// Every element of the level, whatever `ids` holds.
    pub all: bool,
    ids: Arc<[u32]>,
}

impl Elements {
    /// `ids` (sorted and deduplicated here) at `level`.
    pub fn new(level: Level, ids: impl IntoIterator<Item = u32>) -> Self {
        let ids: BTreeSet<u32> = ids.into_iter().collect();
        Self {
            level,
            all: false,
            ids: ids.into_iter().collect(),
        }
    }

    /// Every element at `level`.
    pub fn all(level: Level) -> Self {
        Self {
            level,
            all: true,
            ids: Arc::from([]),
        }
    }

    /// The ids, sorted and unique.
    pub fn ids(&self) -> &[u32] {
        &self.ids
    }

    /// Whether `id` is in the set.
    pub fn contains(&self, id: u32) -> bool {
        self.all || self.ids.binary_search(&id).is_ok()
    }

    /// Whether the set holds nothing.
    pub fn is_empty(&self) -> bool {
        !self.all && self.ids.is_empty()
    }
}

/// What a pick found on a [`Surface`].
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HitTarget {
    /// Triangle `triangle` (an index into the surface's triangles) of face
    /// `face`, at barycentric coordinates `bary` of its corners.
    Triangle {
        /// The face.
        face: u32,
        /// The triangle.
        triangle: u32,
        /// Where on it.
        bary: [f32; 3],
    },
    /// Segment `segment` (an index into the surface's segments) of edge
    /// `id`, at `t` from its first point (0) to its second (1).
    Line {
        /// The edge id.
        id: u32,
        /// The segment.
        segment: u32,
        /// Where along it.
        t: f32,
    },
    /// A point marker's id.
    Point {
        /// The point id.
        id: u32,
    },
}

/// One surface element under the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Hit {
    /// What was hit.
    pub target: HitTarget,
    /// Distance along the view ray.
    pub depth: f64,
    /// Where, in the value's own space.
    pub position: Vec3,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn triangle() -> Surface {
        let corner = |point| Corner {
            point,
            face: 0,
            normal: [0.0, 0.0, 1.0],
        };
        Surface::new(
            Arc::from([[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
            Arc::from([corner(0), corner(1), corner(2)]),
            Arc::from([[0, 1, 2]]),
            1,
            7,
        )
        .with_lines(Arc::from([[0, 0], [1, 0]]))
        .with_markers(Arc::from([[0, 0], [1, 1], [2, 2]]))
    }

    #[test]
    fn a_derived_surface_knows_what_moved_since_its_parent_only() {
        let base = triangle();
        let next = base.derived(
            Arc::from([[0.0; 3], [2.0, 0.0, 0.0], [0.0, 1.0, 0.0]]),
            Ranges::of(1..2),
        );
        assert!(!next.same(&base) && base.same(&base.clone()));
        assert_eq!(next.moved_since(base.identity()), Some(&Ranges::of(1..2)));
        assert_eq!(next.topology(), base.topology());
        let later = next.derived(next.points().clone(), Ranges::new());
        assert_eq!(later.moved_since(base.identity()), None);
        assert_eq!(base.moved_since(base.identity()), None);
        assert!(format!("{base:?}").contains("segments: 1"));
    }

    #[test]
    fn polygons_share_edges_and_face_their_normals() {
        // Two squares sharing the edge 1-2: one in z = 0, one in x = 1.
        let points = [
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
            [1.0, 0.0, -1.0],
            [1.0, 1.0, -1.0],
        ];
        let surface = Surface::polygons(points, [&[0, 1, 2, 3][..], &[1, 4, 5, 2]], 3);
        assert_eq!((surface.faces(), surface.topology()), (2, 3));
        assert_eq!(surface.corners().len(), 8);
        assert_eq!(
            &surface.triangles()[..],
            &[[0, 1, 2], [0, 2, 3], [4, 5, 6], [4, 6, 7]]
        );
        // Seven distinct edges: the shared one is drawn once.
        assert_eq!(surface.lines().len(), 14);
        let corner = surface.corners()[5];
        assert_eq!(
            (corner.point, corner.face, corner.normal),
            (4, 1, [1.0, 0.0, 0.0])
        );
        assert_eq!(surface.corners()[0].normal, [0.0, 0.0, 1.0]);
        // A degenerate polygon faces +Z.
        let flat = Surface::polygons([[0.0; 3]; 3], [[0, 1, 2]], 0);
        assert_eq!(flat.corners()[0].normal, [0.0, 0.0, 1.0]);
    }

    #[test]
    fn element_sets_sort_and_answer_membership() {
        let set = Elements::new(Level::Point, [3, 1, 3]);
        assert_eq!(set.ids(), &[1, 3]);
        assert!(set.contains(3) && !set.contains(2) && !set.is_empty());
        let all = Elements::all(Level::Face);
        assert!(all.contains(99) && !all.is_empty());
        assert!(Elements::new(Level::Edge, []).is_empty());
    }
}
