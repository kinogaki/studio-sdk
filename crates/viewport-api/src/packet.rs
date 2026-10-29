//! What a renderer draws: one immutable surface packet behind the
//! [`SurfaceSource`] trait, plus the element selection shown on it.
//!
//! A domain library (polygon meshes, CAD tessellations, SDF meshes, …) turns
//! its geometry into packets. Positions are one shared point array; vertex
//! vertices carry shading and element ids; triangles, edges and point markers
//! index points. Columns may be paged, so they are read through callbacks at
//! column granularity rather than per element.
use crate::{DrawUpdate, UpdateRanges};
use std::{
    any::Any,
    collections::BTreeSet,
    ops::Range,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
};

/// One triangle vertex: shading data and the ids it belongs to.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::Pod, bytemuck::Zeroable))]
pub struct Vertex {
    /// The face normal, unit length.
    pub normal: [f32; 3],
    /// The shaded (smoothed) normal, unit length.
    pub smooth_normal: [f32; 3],
    /// The two curvature measures the analysis shadings show (see
    /// [`SurfaceSource::curvature_range`]).
    pub curvature: [f32; 2],
    /// The face it belongs to (face picks and highlights).
    pub face: u32,
    /// The point it sits on: an index into the positions column.
    pub point: u32,
    /// The id vertex picks and vertex selections name (a mesh's polygon
    /// vertex index).
    pub component: u32,
}
impl Vertex {
    /// The record as a renderer uploads it: its fields in order, each in
    /// native byte order (what a byte cast of the `repr(C)` record gives).
    pub fn bytes(&self) -> [u8; 44] {
        let mut out = [0; 44];
        let words = self
            .normal
            .iter()
            .chain(&self.smooth_normal)
            .chain(&self.curvature)
            .map(|f| f.to_ne_bytes())
            .chain([self.face, self.point, self.component].map(u32::to_ne_bytes));
        for (at, word) in out.as_chunks_mut::<4>().0.iter_mut().zip(words) {
            *at = word;
        }
        out
    }
}
/// Connectivity only: surfaces, edges and markers fetch one shared point array.
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[cfg_attr(feature = "bytemuck", derive(bytemuck::Pod, bytemuck::Zeroable))]
pub struct OverlayVertex {
    /// An index into the positions column.
    pub point: u32,
    /// The line id (a segment) or the point id (a marker).
    pub component: u32,
}

/// Columns a renderer uploads from a packet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Column {
    /// `[f32; 3]` point positions.
    Positions,
    /// [`Vertex`] records, one per polygon vertex.
    Vertices,
}
impl Column {
    /// Bytes per element.
    pub fn element_size(self) -> usize {
        match self {
            Self::Positions => 12,
            Self::Vertices => std::mem::size_of::<Vertex>(),
        }
    }
}

/// Element kinds a selection or hover can address.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ElementKind {
    /// Point markers, by point id.
    Point,
    /// Lines, by packet-wide line id.
    Edge,
    /// Faces, by face id.
    Face,
    /// Polygon vertices: records of the vertex column.
    Vertex,
    /// Whole objects, addressed by any of their faces.
    Object,
}

/// Selected element ids, sorted and unique. Each value gets a fresh revision
/// when constructed; equal revisions compare equal without looking at the ids,
/// so build a new set only when the selection actually changes and steady
/// frames cost nothing. Different revisions fall back to comparing members.
#[derive(Clone, Debug)]
pub struct ElementSet {
    /// What the ids address.
    pub kind: ElementKind,
    /// Every element of `kind` is selected.
    pub all: bool,
    ids: Arc<[u32]>,
    revision: u64,
}
impl ElementSet {
    /// Sorts and deduplicates `ids`.
    pub fn new(kind: ElementKind, all: bool, ids: impl IntoIterator<Item = u32>) -> Self {
        let ids: BTreeSet<u32> = ids.into_iter().collect();
        Self::from_sorted(kind, all, ids.into_iter().collect())
    }
    /// `ids` must already be sorted and unique (checked in debug builds).
    pub fn from_sorted(kind: ElementKind, all: bool, ids: Arc<[u32]>) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(1);
        debug_assert!(ids.windows(2).all(|w| w[0] < w[1]));
        Self {
            kind,
            all,
            ids,
            revision: NEXT.fetch_add(1, Ordering::Relaxed),
        }
    }
    /// Every element of `kind`.
    pub fn all(kind: ElementKind) -> Self {
        Self::from_sorted(kind, true, Arc::from([]))
    }
    /// Whether `id` is in the set.
    pub fn contains(&self, id: u32) -> bool {
        self.all || self.ids.binary_search(&id).is_ok()
    }
    /// Sorted unique ids.
    pub fn ids(&self) -> &[u32] {
        &self.ids
    }
    /// The set's revision: equal revisions are equal sets.
    pub fn revision(&self) -> u64 {
        self.revision
    }
    /// Whether nothing is selected.
    pub fn is_empty(&self) -> bool {
        !self.all && self.ids.is_empty()
    }
}
impl Default for ElementSet {
    fn default() -> Self {
        Self::new(ElementKind::Point, false, [])
    }
}
impl PartialEq for ElementSet {
    fn eq(&self, other: &Self) -> bool {
        self.revision == other.revision
            || self.kind == other.kind
                && self.all == other.all
                && (Arc::ptr_eq(&self.ids, &other.ids) || self.ids == other.ids)
    }
}

/// How a surface's colors (linear RGB9E5 words, 0: the grey clay) are laid
/// out: one a polygon (flat) or one a polygon vertex (interpolated linearly
/// across the polygon, as Houdini shows a point or vertex `Cd`). A polygon
/// has at least three vertices, so the count tells them apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ColorLayout {
    /// One color a face.
    Faces,
    /// One color a polygon vertex.
    Vertices,
}
impl ColorLayout {
    /// The layout of `colors` words on `surface`; none for another count
    /// (colors made for another packet, a cook on its way).
    pub fn of(colors: usize, surface: &dyn SurfaceSource) -> Option<Self> {
        if colors == surface.face_count() {
            Some(Self::Faces)
        } else if colors == surface.column_len(Column::Vertices) {
            Some(Self::Vertices)
        } else {
            None
        }
    }
    /// The shaders' code: 1 by polygon, 2 by vertex.
    pub fn code(self) -> u32 {
        match self {
            Self::Faces => 1,
            Self::Vertices => 2,
        }
    }
}

/// An immutable surface packet. Implementations are shared as
/// `Arc<dyn SurfaceSource>`; renderers treat the allocation as the packet's
/// identity (see [`same_surface`]), and its maker reads it back as its own
/// type with [`packet_as`].
pub trait SurfaceSource: Any + Send + Sync {
    /// Number of elements in a column.
    fn column_len(&self, column: Column) -> usize;
    /// Calls `write(bytes_range, bytes)` for consecutive pieces of the
    /// column's element `range`; byte ranges are relative to the range start.
    fn column_bytes(
        &self,
        column: Column,
        range: Range<usize>,
        write: &mut dyn FnMut(Range<usize>, &[u8]),
    );
    /// Triangle vertex indices into the vertex column.
    fn indices(&self) -> &[u32];
    /// Line segments (pairs) and point markers. A segment's `component`
    /// is its packet-wide line id (see [`crate::LineClass`]); a marker's is
    /// the id point picks and point selections use (a mesh's point index,
    /// a CAD control vertex's id).
    fn lines(&self) -> &[OverlayVertex];
    /// Point markers.
    fn points(&self) -> &[OverlayVertex];
    /// The classes the segments are drawn and picked in, in drawing order;
    /// segments outside every class are not drawn.
    fn line_classes(&self) -> &[crate::LineClass];
    /// The classes the point markers are drawn in (see
    /// [`crate::PointClass`]); none: every marker in the plain points'
    /// look.
    fn point_classes(&self) -> &[crate::PointClass] {
        &[]
    }
    /// Number of faces (face ids are below it).
    fn face_count(&self) -> usize;
    /// Bounds of the positions, `(min, max)`; none when empty.
    fn bounds(&self) -> Option<([f64; 3], [f64; 3])>;
    /// Equal keys mean equal connectivity: GPU allocations can be reused.
    fn topology_key(&self) -> [u8; 32];
    /// Exact-parent identity token of this packet.
    fn identity(&self) -> &Arc<()>;
    /// Ranges changed relative to the packet this one was derived from.
    fn update(&self) -> Option<&Arc<DrawUpdate>>;
    /// The display range of curvature measure `measure` (98th percentile).
    fn curvature_range(&self, measure: usize) -> f32;
    /// Faces of the object containing `face`, for object hover outlines.
    fn object_faces(&self, face: u32) -> Result<BTreeSet<u32>, String>;
    /// Point-id segments outlining `selection` on this surface (selected
    /// edges, face boundaries; none for points), within `budget` bytes.
    fn selection_segments(
        &self,
        selection: &ElementSet,
        budget: u64,
    ) -> Result<Vec<OverlayVertex>, String>;

    /// Runs of triangle indices drawn in a look other than shaded, in
    /// order (see [`crate::FaceRun`]); none: every face shaded, opaque.
    fn face_runs(&self) -> &[crate::FaceRun] {
        &[]
    }
    /// When the renderer derives this packet's vertex shading from its
    /// positions (see [`crate::ShadingPlan`]): the vertex column's normals
    /// are then stale and are recomputed after every position upload.
    fn derived_shading(&self) -> Option<&Arc<crate::ShadingPlan>> {
        None
    }
    /// Ranges to rewrite when `previous` is on the GPU: this packet derives
    /// from it directly or through a few undisplayed packets.
    fn update_from(&self, previous: &dyn SurfaceSource) -> Option<UpdateRanges<'_>> {
        self.update()
            .and_then(|update| update.ranges_from(previous.identity()))
    }
    /// Elements of `kind` in this packet.
    fn element_count(&self, kind: ElementKind) -> usize {
        match kind {
            ElementKind::Object | ElementKind::Face => self.face_count(),
            ElementKind::Edge => self
                .line_classes()
                .iter()
                .map(|c| (c.first_id + c.ids) as usize)
                .max()
                .unwrap_or(0),
            ElementKind::Point => self.points().len(),
            ElementKind::Vertex => self.column_len(Column::Vertices),
        }
    }
    /// Logical upload size, excluding GPU allocation padding.
    fn upload_bytes(&self) -> usize {
        self.column_len(Column::Positions) * Column::Positions.element_size()
            + self.column_len(Column::Vertices) * Column::Vertices.element_size()
            + self.indices().len() * 4
            + (self.lines().len() + self.points().len()) * std::mem::size_of::<OverlayVertex>()
    }
}

/// Whether two packets are the same allocation.
pub fn same_surface(a: &dyn SurfaceSource, b: &dyn SurfaceSource) -> bool {
    std::ptr::addr_eq(a, b)
}

/// `packet` as its concrete type `T`, sharing the allocation; none when it
/// is another type. A kernel reads back the packets it made this way (its
/// picking structures, the base of a live preview).
pub fn packet_as<T: SurfaceSource>(packet: &Arc<dyn SurfaceSource>) -> Option<Arc<T>> {
    let any: Arc<dyn Any + Send + Sync> = packet.clone();
    any.downcast().ok()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::LineClass;

    /// An empty packet; `N` makes distinct types of one shape.
    struct Empty<const N: u8>(Arc<()>);
    impl<const N: u8> SurfaceSource for Empty<N> {
        fn column_len(&self, _: Column) -> usize {
            0
        }
        fn column_bytes(&self, _: Column, _: Range<usize>, _: &mut dyn FnMut(Range<usize>, &[u8])) {
        }
        fn indices(&self) -> &[u32] {
            &[]
        }
        fn lines(&self) -> &[OverlayVertex] {
            &[]
        }
        fn points(&self) -> &[OverlayVertex] {
            &[]
        }
        fn line_classes(&self) -> &[LineClass] {
            &[]
        }
        fn face_count(&self) -> usize {
            0
        }
        fn bounds(&self) -> Option<([f64; 3], [f64; 3])> {
            None
        }
        fn topology_key(&self) -> [u8; 32] {
            [0; 32]
        }
        fn identity(&self) -> &Arc<()> {
            &self.0
        }
        fn update(&self) -> Option<&Arc<DrawUpdate>> {
            None
        }
        fn curvature_range(&self, _: usize) -> f32 {
            0.0
        }
        fn object_faces(&self, _: u32) -> Result<BTreeSet<u32>, String> {
            Ok(BTreeSet::new())
        }
        fn selection_segments(&self, _: &ElementSet, _: u64) -> Result<Vec<OverlayVertex>, String> {
            Ok(Vec::new())
        }
    }

    #[test]
    fn a_vertex_writes_its_fields_in_order_and_sets_answer_membership() {
        let v = Vertex {
            normal: [1.0, 2.0, 3.0],
            smooth_normal: [4.0, 5.0, 6.0],
            curvature: [7.0, 8.0],
            face: 9,
            point: 10,
            component: 11,
        };
        let bytes = v.bytes();
        assert_eq!(bytes.len(), std::mem::size_of::<Vertex>());
        assert_eq!(bytes[..4], 1.0f32.to_ne_bytes());
        assert_eq!(bytes[28..32], 8.0f32.to_ne_bytes());
        assert_eq!(bytes[40..], 11u32.to_ne_bytes());
        #[cfg(feature = "bytemuck")]
        assert_eq!(&bytes[..], bytemuck::bytes_of(&v));
        let set = ElementSet::new(ElementKind::Point, false, [3, 1, 3]);
        assert_eq!(set.ids(), &[1, 3]);
        assert!(set.contains(3) && !set.contains(2));
        let all = ElementSet::all(ElementKind::Face);
        assert!(all.contains(99) && !all.is_empty());
    }

    #[test]
    fn a_packet_reads_back_as_its_own_type_only() {
        let packet: Arc<dyn SurfaceSource> = Arc::new(Empty::<0>(Arc::new(())));
        let read = packet_as::<Empty<0>>(&packet).expect("its own type");
        assert!(same_surface(&*read, &*packet));
        assert!(packet_as::<Empty<1>>(&packet).is_none());
    }
}
