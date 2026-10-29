//! What the viewport draws and picks: a value's display [`Surface`],
//! component levels ([`ElementKind`]), sets of packet elements
//! ([`ElementSet`]) and pick [`Hit`]s.
//!
//! Studio's core owns the viewport (panes, cameras, drawing, picking). A
//! data type only says what to draw, as packets its
//! [`Displayable`](crate::Displayable) facet prepares, and what a pick on
//! them means, through its [`Pickable`](crate::Pickable) facet. Packets
//! are `kinogaki-viewport-api`'s [`SurfaceSource`]s; a [`Surface`] is the
//! simple one, built from points, polygon corners, triangles, edges and
//! markers. The host uploads a packet's shared arrays as they are and, for
//! a surface [`derived`](Surface::derived) from the one on screen, only the
//! points that moved.
use crate::math::Vec3;
use crate::render::Ranges;
use kinogaki_viewport_api::{
    BufferRanges, Column, DrawUpdate, LineClass, LineClassId, LineStyle, Vertex,
};
pub use kinogaki_viewport_api::{ElementKind, ElementSet, OverlayVertex, SurfaceSource, packet_as};
use std::collections::{BTreeMap, BTreeSet};
use std::fmt;
use std::ops::Range;
use std::sync::Arc;

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
/// - **Lines** are segments, two line vertices each, a line vertex's
///   `component` the edge id, the same at both ends: what edge picks and
///   edge highlights name.
/// - **Markers** are drawn points, each a point and its point id (the
///   vertex's `component`): what point picks name.
///
/// It is a [`SurfaceSource`] itself, so the host draws it as it is. Its
/// identity is the allocation of a token made with it: a clone shares it,
/// [`derived`](Self::derived) makes a new one.
#[derive(Clone)]
pub struct Surface {
    points: Arc<[[f32; 3]]>,
    corners: Arc<[Corner]>,
    triangles: Arc<[[u32; 3]]>,
    faces: usize,
    lines: Arc<[OverlayVertex]>,
    /// The lines' one class, edges (none without lines).
    classes: Arc<[LineClass]>,
    markers: Arc<[OverlayVertex]>,
    topology: u64,
    identity: Arc<()>,
    /// What changed since the surface this one was derived from (and the
    /// few before it).
    update: Option<Arc<DrawUpdate>>,
}

/// Records written per upload piece (the vertex column is built on the
/// fly from the corners).
const VERTEX_PIECE: usize = 4096;

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
            classes: Arc::from([]),
            markers: Arc::from([]),
            topology,
            identity: Arc::new(()),
            update: None,
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
                    let vertex = |point| OverlayVertex {
                        point,
                        component: id,
                    };
                    lines.extend([vertex(a), vertex(b)]);
                }
            }
            faces += 1;
        }
        Self::new(points, corners.into(), triangles.into(), faces, topology)
            .with_lines(lines.into())
    }

    /// With `lines`: line vertices, two a segment, drawn and picked as
    /// edges.
    pub fn with_lines(mut self, lines: Arc<[OverlayVertex]>) -> Self {
        debug_assert!(lines.len().is_multiple_of(2), "two line vertices a segment");
        let segments = lines.len() / 2;
        self.classes = if segments == 0 {
            Arc::from([])
        } else {
            let ids = lines.iter().map(|v| v.component + 1).max().unwrap_or(0);
            Arc::from([LineClass {
                id: LineClassId(0),
                name: "edges",
                style: LineStyle::EDGES,
                segments: 0..segments as u32,
                first_id: 0,
                ids,
            }])
        };
        self.lines = lines;
        self
    }

    /// With point `markers`.
    pub fn with_markers(mut self, markers: Arc<[OverlayVertex]>) -> Self {
        self.markers = markers;
        self
    }

    /// A surface like this one with `points` (as many) of which those in
    /// `moved` changed: a drag's next frame. The host patches only those
    /// points over this surface's upload, or over one a few frames back it
    /// still holds. Corners keep their normals.
    pub fn derived(&self, points: Arc<[[f32; 3]]>, moved: Ranges) -> Self {
        debug_assert_eq!(points.len(), self.points.len());
        let mut positions = BufferRanges::new::<[f32; 3]>(points.len());
        if moved.is_whole() {
            positions.add(0..points.len());
        } else {
            for span in moved.spans() {
                positions.add(span.start as usize..span.end as usize);
            }
        }
        let unchanged = |len: usize| BufferRanges::new::<u32>(len);
        let mut update = DrawUpdate::new(
            self.identity.clone(),
            [
                positions,
                BufferRanges::new::<Vertex>(self.corners.len()),
                unchanged(self.triangles.len() * 3),
                BufferRanges::new::<OverlayVertex>(self.lines.len()),
                BufferRanges::new::<OverlayVertex>(self.markers.len()),
            ],
        );
        if let Some(held) = &self.update {
            update.inherit(held);
        }
        Self {
            points,
            identity: Arc::new(()),
            update: Some(Arc::new(update)),
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
    pub fn lines(&self) -> &Arc<[OverlayVertex]> {
        &self.lines
    }

    /// The point markers.
    pub fn markers(&self) -> &Arc<[OverlayVertex]> {
        &self.markers
    }

    /// The connectivity key.
    pub fn topology(&self) -> u64 {
        self.topology
    }

    /// Whether `self` and `other` are the same surface.
    pub fn same(&self, other: &Surface) -> bool {
        Arc::ptr_eq(&self.identity, &other.identity)
    }

    /// Corner `corner` as the vertex record the host uploads.
    fn vertex(&self, corner: usize) -> Vertex {
        let c = self.corners[corner];
        Vertex {
            normal: c.normal,
            smooth_normal: c.normal,
            curvature: [0.0; 2],
            face: c.face,
            point: c.point,
            component: corner as u32,
        }
    }
}

impl SurfaceSource for Surface {
    fn column_len(&self, column: Column) -> usize {
        match column {
            Column::Positions => self.points.len(),
            Column::Vertices => self.corners.len(),
        }
    }

    /// Both columns go in pieces of a few thousand records, written as
    /// their bytes.
    fn column_bytes(
        &self,
        column: Column,
        range: Range<usize>,
        write: &mut dyn FnMut(Range<usize>, &[u8]),
    ) {
        let size = column.element_size();
        let mut piece = Vec::with_capacity(VERTEX_PIECE.min(range.len()) * size);
        for start in range.clone().step_by(VERTEX_PIECE) {
            let end = (start + VERTEX_PIECE).min(range.end);
            piece.clear();
            match column {
                Column::Positions => piece.extend(
                    self.points[start..end]
                        .iter()
                        .flatten()
                        .flat_map(|c| c.to_ne_bytes()),
                ),
                Column::Vertices => piece.extend((start..end).flat_map(|c| self.vertex(c).bytes())),
            }
            let at = (start - range.start) * size;
            write(at..at + piece.len(), &piece);
        }
    }

    fn indices(&self) -> &[u32] {
        self.triangles.as_flattened()
    }

    fn lines(&self) -> &[OverlayVertex] {
        &self.lines
    }

    fn points(&self) -> &[OverlayVertex] {
        &self.markers
    }

    fn line_classes(&self) -> &[LineClass] {
        &self.classes
    }

    fn face_count(&self) -> usize {
        self.faces
    }

    fn bounds(&self) -> Option<([f64; 3], [f64; 3])> {
        self.points.iter().fold(None, |b, p| {
            let p = p.map(f64::from);
            Some(b.map_or((p, p), |(lo, hi): ([f64; 3], [f64; 3])| {
                (
                    std::array::from_fn(|i| lo[i].min(p[i])),
                    std::array::from_fn(|i| hi[i].max(p[i])),
                )
            }))
        })
    }

    fn topology_key(&self) -> [u8; 32] {
        let mut key = [0; 32];
        key[..8].copy_from_slice(&self.topology.to_le_bytes());
        key
    }

    fn identity(&self) -> &Arc<()> {
        &self.identity
    }

    fn update(&self) -> Option<&Arc<DrawUpdate>> {
        self.update.as_ref()
    }

    /// No curvature: every range is 1.
    fn curvature_range(&self, _measure: usize) -> f32 {
        1.0
    }

    fn object_faces(&self, face: u32) -> Result<BTreeSet<u32>, String> {
        Ok(BTreeSet::from([face]))
    }

    /// Selected edges outline themselves; faces have no outline here.
    fn selection_segments(
        &self,
        selection: &ElementSet,
        _budget: u64,
    ) -> Result<Vec<OverlayVertex>, String> {
        if selection.kind != ElementKind::Edge {
            return Ok(Vec::new());
        }
        Ok(self
            .lines
            .as_chunks::<2>()
            .0
            .iter()
            .filter(|s| selection.contains(s[0].component))
            .flatten()
            .copied()
            .collect())
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

    /// A unit square of two triangles, its four edges and four markers.
    fn square() -> Surface {
        let corner = |point| Corner {
            point,
            face: 0,
            normal: [0.0, 0.0, 1.0],
        };
        let vertex = |point, component| OverlayVertex { point, component };
        Surface::new(
            Arc::from([
                [0.0, 0.0, 0.0],
                [1.0, 0.0, 0.0],
                [1.0, 1.0, 0.0],
                [0.0, 1.0, 0.0],
            ]),
            (0..4).map(corner).collect(),
            Arc::from([[0, 1, 2], [0, 2, 3]]),
            1,
            42,
        )
        .with_lines(
            [
                (0, 0),
                (1, 0),
                (1, 1),
                (2, 1),
                (2, 2),
                (3, 2),
                (3, 3),
                (0, 3),
            ]
            .map(|(p, e)| vertex(p, e))
            .into(),
        )
        .with_markers((0..4).map(|i| vertex(i, i)).collect())
    }

    #[test]
    fn a_surface_is_a_packet_over_its_own_arrays() {
        let surface = square();
        assert!(surface.same(&surface.clone()));
        assert_eq!(surface.indices(), &[0, 1, 2, 0, 2, 3]);
        assert!(std::ptr::eq(
            surface.indices().as_ptr(),
            surface.triangles().as_ptr().cast()
        ));
        let lines = SurfaceSource::lines(&surface);
        assert_eq!((lines.len(), lines[2].point, lines[2].component), (8, 1, 1));
        assert_eq!(SurfaceSource::points(&surface)[3].component, 3);
        assert_eq!(surface.element_count(ElementKind::Edge), 4);
        assert_eq!(surface.element_count(ElementKind::Face), 1);
        assert_eq!(surface.bounds().unwrap().1, [1.0, 1.0, 0.0]);
        assert_eq!(surface.topology_key()[0], 42);
        assert!(surface.update().is_none());
        LineClass::validate(surface.line_classes(), lines.len() / 2).unwrap();
        assert!(format!("{surface:?}").contains("segments: 4"));
        // The vertex column, built from the corners.
        let mut bytes = Vec::new();
        surface.column_bytes(Column::Vertices, 1..4, &mut |at, b| {
            assert_eq!(at.start, bytes.len());
            bytes.extend_from_slice(b);
        });
        assert_eq!(bytes.len(), 3 * Column::Vertices.element_size());
        assert_eq!(&bytes[..44], &surface.vertex(1).bytes());
        let mut positions = Vec::new();
        surface.column_bytes(Column::Positions, 1..3, &mut |_, b| {
            positions.extend_from_slice(b)
        });
        assert_eq!(positions.len(), 24);
        assert_eq!(positions[..4], 1.0f32.to_ne_bytes());
        // Edge selections outline their segments.
        let edge = ElementSet::new(ElementKind::Edge, false, [2]);
        let outline = surface.selection_segments(&edge, u64::MAX).unwrap();
        assert_eq!(outline.iter().map(|v| v.point).collect::<Vec<_>>(), [2, 3]);
    }

    #[test]
    fn a_drag_patches_only_the_moved_points_even_over_skipped_frames() {
        let moved = |s: &Surface, point: usize| {
            let mut points = s.points().to_vec();
            points[point][2] += 1.0;
            s.derived(points.into(), Ranges::of(point as u32..point as u32 + 1))
        };
        let first = square();
        let second = moved(&first, 1);
        let third = moved(&second, 3);
        assert!(!second.same(&first) && second.topology() == first.topology());
        let spans = |r: &BufferRanges| r.iter().map(|r| (r.start, r.end)).collect::<Vec<_>>();
        let over_second = third.update_from(&second).unwrap();
        assert_eq!(spans(over_second.positions), [(3, 4)]);
        assert_eq!(over_second.vertices.iter().count(), 0);
        // The host never displayed the second: the third patches the first.
        let over_first = third.update_from(&first).unwrap();
        assert_eq!(spans(over_first.positions), [(1, 4)]);
        assert!(third.update_from(&square()).is_none());
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
        assert_eq!(surface.element_count(ElementKind::Edge), 7);
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
}
