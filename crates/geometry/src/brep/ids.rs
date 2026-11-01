//! Typed entity ids, and [`Entity`]: any one of them, as a refusal names
//! it.
use super::Body;

macro_rules! ids {
    ($($(#[$doc:meta])* $name:ident: $table:ident;)*) => {$(
        $(#[$doc])*
        ///
        /// A dense index into its table, made only by [`BrepBuilder`]
        /// (in the order entities are added) or by
        #[doc = concat!("[`", stringify!($name), "::from_index`]")]
        /// checked against a body.
        ///
        /// [`BrepBuilder`]: super::BrepBuilder
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
        pub struct $name(u32);

        impl $name {
            /// Its index in its table.
            #[inline]
            pub fn index(self) -> usize {
                self.0 as usize
            }

            /// The id of entity `index` of `body`, when `body` has one.
            pub fn from_index(body: &Body, index: usize) -> Option<Self> {
                (index < body.$table()).then(|| Self::new(index))
            }

            /// An id the crate has checked, or will check, against its
            /// table (tables hold at most 2²⁸ entities).
            #[inline]
            pub(crate) fn new(index: usize) -> Self {
                Self(index as u32)
            }
        }
    )*};
}

ids! {
    /// A vertex.
    VertexId: vertex_count;
    /// An edge.
    EdgeId: edge_count;
    /// A coedge: one side of an edge, in one loop.
    CoedgeId: coedge_count;
    /// A loop of coedges bounding a face.
    LoopId: loop_count;
    /// A face.
    FaceId: face_count;
    /// A shell: a connected set of faces.
    ShellId: shell_count;
    /// A lump: an outer shell and its voids.
    LumpId: lump_count;
    /// A carrier surface.
    SurfaceId: surface_count;
    /// A carrier curve in space.
    CurveId: curve_count;
    /// A curve in a face's chart.
    PCurveId: pcurve_count;
}

/// One entity of a B-rep value: what a refusal blames
/// ([`GeometryError::entity`](crate::GeometryError::entity)).
///
/// New kinds may be added; a caller that doesn't know one reports the
/// refusal without pointing at an entity.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Entity {
    /// A vertex of the body being built or checked.
    Vertex(VertexId),
    /// An edge.
    Edge(EdgeId),
    /// A coedge.
    Coedge(CoedgeId),
    /// A loop.
    Loop(LoopId),
    /// A face.
    Face(FaceId),
    /// A shell.
    Shell(ShellId),
    /// A lump.
    Lump(LumpId),
    /// A surface.
    Surface(SurfaceId),
    /// A curve.
    Curve(CurveId),
    /// A p-curve.
    PCurve(PCurveId),
    /// Body `index` of a [`Brep`](super::Brep).
    Body(usize),
    /// Datum `index` of a [`Brep`](super::Brep).
    Datum(usize),
}

impl Entity {
    /// The entity's index in its table.
    pub fn index(self) -> usize {
        match self {
            Entity::Vertex(id) => id.index(),
            Entity::Edge(id) => id.index(),
            Entity::Coedge(id) => id.index(),
            Entity::Loop(id) => id.index(),
            Entity::Face(id) => id.index(),
            Entity::Shell(id) => id.index(),
            Entity::Lump(id) => id.index(),
            Entity::Surface(id) => id.index(),
            Entity::Curve(id) => id.index(),
            Entity::PCurve(id) => id.index(),
            Entity::Body(i) | Entity::Datum(i) => i,
        }
    }
}
