//! The topology tables ([`BrepTopology`]) and their records, and the
//! entities' persistent names ([`Names`]).
use super::names::Name;
use super::{
    CoedgeId, CurveId, EdgeId, FaceId, LoopId, LumpId, PCurveId, ShellId, SurfaceId, VertexId,
};
use crate::column::Column;
use crate::derived::Derived;
use std::fmt;
use std::sync::{Arc, OnceLock};

/// Whether an entity runs with its carrier or against it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Sense {
    /// With it.
    Same,
    /// Against it.
    Reversed,
}

impl Sense {}

/// What a body is, from its shells.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum BodyKind {
    /// Every shell is closed: a solid, possibly with voids and several
    /// lumps. A body without shells is a solid too.
    Solid,
    /// At least one shell is open: surfaces (a quilt).
    Sheet,
}

/// A vertex of the body. Where it is is geometry
/// ([`Geometry::point`](super::Geometry::point)), so moving it keeps the
/// topology.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vertex {
    pub(crate) precision: Option<f64>,
}

impl Vertex {
    /// The distance within which it is tolerant, when it is.
    pub fn precision(&self) -> Option<f64> {
        self.precision
    }
}

/// An interval `[t0, t1]` of a carrier curve, from `vertices[0]` (at
/// `t0`) to `vertices[1]` (at `t1`). A pole edge has no curve: it lies in
/// its faces' charts only.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Edge {
    pub(crate) curve: Option<CurveId>,
    pub(crate) interval: [f64; 2],
    pub(crate) vertices: [VertexId; 2],
    pub(crate) precision: Option<f64>,
    pub(crate) coedge: CoedgeId,
}

impl Edge {
    /// The carrier, or `None` for a pole edge.
    pub fn curve(&self) -> Option<CurveId> {
        self.curve
    }

    /// `[t0, t1]`, `t0 < t1`, in the carrier's parameter.
    pub fn interval(&self) -> [f64; 2] {
        self.interval
    }

    /// Start (at `t0`) and end (at `t1`); one vertex twice for a pole edge
    /// or a closed edge.
    pub fn vertices(&self) -> [VertexId; 2] {
        self.vertices
    }

    /// The distance within which it is tolerant, when it is.
    pub fn precision(&self) -> Option<f64> {
        self.precision
    }

    /// The first coedge of its radial ring.
    pub fn coedge(&self) -> CoedgeId {
        self.coedge
    }
}

/// One side of an edge, in one loop of one face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Coedge {
    pub(crate) edge: EdgeId,
    pub(crate) sense: Sense,
    pub(crate) owner: LoopId,
    pub(crate) radial: CoedgeId,
    pub(crate) pcurve: Option<PCurveId>,
    pub(crate) precision: Option<f64>,
}

impl Coedge {
    /// Its edge.
    pub fn edge(&self) -> EdgeId {
        self.edge
    }

    /// `Same` when it runs from its edge's start to its end.
    pub fn sense(&self) -> Sense {
        self.sense
    }

    /// Its edge in its face's chart, sharing the edge's parameter.
    pub fn pcurve(&self) -> Option<PCurveId> {
        self.pcurve
    }

    /// The distance within which it is tolerant, when it is.
    pub fn precision(&self) -> Option<f64> {
        self.precision
    }
}

/// A cycle of coedges bounding a face, stored in order.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Loop {
    pub(crate) face: FaceId,
    pub(crate) first: u32,
    pub(crate) len: u32,
}

impl Loop {
    /// The face it bounds.
    pub fn face(&self) -> FaceId {
        self.face
    }
}

/// A surface trimmed by loops.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Face {
    pub(crate) surface: SurfaceId,
    pub(crate) sense: Sense,
    pub(crate) first_loop: u32,
    pub(crate) loops: u32,
    pub(crate) shell: ShellId,
    pub(crate) precision: Option<f64>,
}

impl Face {
    /// Its surface.
    pub fn surface(&self) -> SurfaceId {
        self.surface
    }

    /// `Same` when its outward normal is the surface's normal.
    pub fn sense(&self) -> Sense {
        self.sense
    }

    /// The shell it is in.
    pub fn shell(&self) -> ShellId {
        self.shell
    }

    /// The distance within which it is tolerant, when it is.
    pub fn precision(&self) -> Option<f64> {
        self.precision
    }
}

/// A connected set of faces.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Shell {
    pub(crate) lump: LumpId,
    pub(crate) first: u32,
    pub(crate) len: u32,
    pub(crate) closed: bool,
}

impl Shell {
    /// The lump it is in.
    pub fn lump(&self) -> LumpId {
        self.lump
    }

    /// Whether it is closed: every edge of its faces but a pole edge has
    /// two coedges.
    pub fn closed(&self) -> bool {
        self.closed
    }
}

/// An outer shell and its voids.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Lump {
    pub(crate) first: u32,
    pub(crate) len: u32,
}

/// How many entities of each kind a topology holds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Counts {
    /// Vertices.
    pub vertices: usize,
    /// Edges.
    pub edges: usize,
    /// Coedges.
    pub coedges: usize,
    /// Loops.
    pub loops: usize,
    /// Faces.
    pub faces: usize,
    /// Shells.
    pub shells: usize,
    /// Lumps.
    pub lumps: usize,
}

/// A body's topology: vertices, edges, coedges, loops, faces, shells and
/// lumps, each table by its id, made only by
/// [`BrepBuilder`](super::BrepBuilder). Shared by every body that shares
/// it (a CV drag keeps it), with typed cache slots of its own
/// (`derived`) for what a kernel derives from it: a
/// vertex → edges index, adjacency, a mesher's face order.
pub struct BrepTopology {
    pub(crate) vertices: Column<Vertex>,
    pub(crate) edges: Column<Edge>,
    pub(crate) coedges: Column<Coedge>,
    pub(crate) loops: Column<Loop>,
    pub(crate) faces: Column<Face>,
    pub(crate) shells: Column<Shell>,
    pub(crate) shell_faces: Column<FaceId>,
    pub(crate) lumps: Column<Lump>,
    pub(crate) lump_shells: Column<ShellId>,
    pub(crate) derived: Derived,
}

impl BrepTopology {
    /// Vertex `v`.
    ///
    /// # Panics
    /// When `v` is not of a body sharing this topology (likewise for every
    /// accessor taking an id).
    pub fn vertex(&self, v: VertexId) -> &Vertex {
        &self.vertices[v.index()]
    }

    /// Edge `e`.
    pub fn edge(&self, e: EdgeId) -> &Edge {
        &self.edges[e.index()]
    }

    /// Coedge `c`.
    pub fn coedge(&self, c: CoedgeId) -> &Coedge {
        &self.coedges[c.index()]
    }

    /// Loop `l`.
    pub(crate) fn loop_(&self, l: LoopId) -> &Loop {
        &self.loops[l.index()]
    }

    /// Face `f`.
    pub fn face(&self, f: FaceId) -> &Face {
        &self.faces[f.index()]
    }

    /// Every vertex's id.
    pub fn vertex_ids(&self) -> impl ExactSizeIterator<Item = VertexId> + use<> {
        (0..self.vertices.len()).map(VertexId::new)
    }

    /// Every edge's id.
    pub fn edge_ids(&self) -> impl ExactSizeIterator<Item = EdgeId> + use<> {
        (0..self.edges.len()).map(EdgeId::new)
    }

    /// Every coedge's id.
    pub(crate) fn coedge_ids(&self) -> impl ExactSizeIterator<Item = CoedgeId> + use<> {
        (0..self.coedges.len()).map(CoedgeId::new)
    }

    /// Every face's id.
    pub fn face_ids(&self) -> impl ExactSizeIterator<Item = FaceId> + use<> {
        (0..self.faces.len()).map(FaceId::new)
    }

    /// Every shell's id.
    pub fn shell_ids(&self) -> impl ExactSizeIterator<Item = ShellId> + use<> {
        (0..self.shells.len()).map(ShellId::new)
    }

    /// Every lump's id.
    pub fn lump_ids(&self) -> impl ExactSizeIterator<Item = LumpId> + use<> {
        (0..self.lumps.len()).map(LumpId::new)
    }

    /// The coedges of loop `l`, in order round it.
    pub fn loop_coedges(&self, l: LoopId) -> impl ExactSizeIterator<Item = CoedgeId> + use<> {
        let l = self.loops[l.index()];
        (l.first..l.first + l.len).map(|c| CoedgeId::new(c as usize))
    }

    /// The loops of face `f`, the outer first.
    pub fn face_loops(&self, f: FaceId) -> impl ExactSizeIterator<Item = LoopId> + use<> {
        let f = self.faces[f.index()];
        (f.first_loop..f.first_loop + f.loops).map(|l| LoopId::new(l as usize))
    }

    /// The faces of shell `s`.
    pub fn shell_faces(&self, s: ShellId) -> &[FaceId] {
        let s = self.shells[s.index()];
        &self.shell_faces[s.first as usize..(s.first + s.len) as usize]
    }

    /// The shells of lump `l`, the outer first.
    pub fn lump_shells(&self, l: LumpId) -> &[ShellId] {
        let l = self.lumps[l.index()];
        &self.lump_shells[l.first as usize..(l.first + l.len) as usize]
    }

    /// The coedges of edge `e`'s radial ring, from [`Edge::coedge`].
    pub fn edge_coedges(&self, e: EdgeId) -> impl Iterator<Item = CoedgeId> + '_ {
        let first = self.edges[e.index()].coedge;
        let mut next = Some(first);
        std::iter::from_fn(move || {
            let c = next?;
            let radial = self.coedges[c.index()].radial;
            next = (radial != first).then_some(radial);
            Some(c)
        })
    }

    /// The face coedge `c` bounds.
    pub fn coedge_face(&self, c: CoedgeId) -> FaceId {
        self.loops[self.coedges[c.index()].owner.index()].face
    }

    /// Where coedge `c` starts: its edge's start when it runs with it.
    pub fn coedge_start(&self, c: CoedgeId) -> VertexId {
        let c = self.coedges[c.index()];
        let [start, end] = self.edges[c.edge.index()].vertices;
        match c.sense {
            Sense::Same => start,
            Sense::Reversed => end,
        }
    }

    /// Where coedge `c` ends.
    pub(crate) fn coedge_end(&self, c: CoedgeId) -> VertexId {
        let c = self.coedges[c.index()];
        let [start, end] = self.edges[c.edge.index()].vertices;
        match c.sense {
            Sense::Same => end,
            Sense::Reversed => start,
        }
    }

    /// How many entities of each kind it holds.
    pub fn counts(&self) -> Counts {
        Counts {
            vertices: self.vertices.len(),
            edges: self.edges.len(),
            coedges: self.coedges.len(),
            loops: self.loops.len(),
            faces: self.faces.len(),
            shells: self.shells.len(),
            lumps: self.lumps.len(),
        }
    }

    /// The `D` derived from this topology, built by `build` the first time
    /// any holder asks and dropped with it (the crate's cache rule; only
    /// the crate that defines `D` asks for it).
    pub(crate) fn derived<D: Send + Sync + 'static>(
        &self,
        build: impl FnOnce(&BrepTopology) -> D,
    ) -> Arc<D> {
        self.derived.get_or_build(|| build(self))
    }

    /// A stable 32-byte hash of the topology's structure, its counts and
    /// adjacency alone (no parameter ranges, precisions, carriers or other
    /// geometry), the same for equal structures in every session: whether
    /// its face, edge and vertex ids still mean the same elements, which a
    /// resize keeps and a face added or split changes. What a
    /// [`Selection`](crate::Selection) made on it records. Cached.
    pub(crate) fn fingerprint(&self) -> [u8; 32] {
        struct Fingerprint([u8; 32]);
        self.derived(|t| Fingerprint(super::fingerprint::of(t))).0
    }
}

/// Topologies are equal when their tables are.
impl PartialEq for BrepTopology {
    fn eq(&self, other: &Self) -> bool {
        self.vertices == other.vertices
            && self.edges == other.edges
            && self.coedges == other.coedges
            && self.loops == other.loops
            && self.faces == other.faces
            && self.shells == other.shells
            && self.shell_faces == other.shell_faces
            && self.lumps == other.lumps
            && self.lump_shells == other.lump_shells
    }
}

impl fmt::Debug for BrepTopology {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("BrepTopology")
            .field("counts", &self.counts())
            .finish_non_exhaustive()
    }
}

/// The persistent names of a body's faces, edges and vertices, each table
/// by its id; within a body, the names of one kind are distinct.
#[derive(Clone, Debug)]
pub struct Names {
    pub(crate) faces: Column<Name>,
    pub(crate) edges: Column<Name>,
    pub(crate) vertices: Column<Name>,
    /// Their fingerprint, once asked for.
    pub(crate) fingerprint: OnceLock<[u8; 32]>,
}

/// Names are equal when their tables are.
impl PartialEq for Names {
    fn eq(&self, other: &Self) -> bool {
        self.faces == other.faces && self.edges == other.edges && self.vertices == other.vertices
    }
}

impl Names {
    /// The names' fingerprint ([`super::fingerprint`]'s names stream):
    /// what [`Brep::fingerprint`](super::Brep::fingerprint) hashes beside
    /// each body's topology. Cached.
    pub(crate) fn fingerprint(&self) -> [u8; 32] {
        *(self.fingerprint).get_or_init(|| super::fingerprint::names(self))
    }

    /// Edge `e`'s name.
    pub fn edge(&self, e: EdgeId) -> &Name {
        &self.edges[e.index()]
    }

    /// Every face's name, by id.
    pub fn faces(&self) -> &Column<Name> {
        &self.faces
    }

    /// Every edge's name, by id.
    pub fn edges(&self) -> &Column<Name> {
        &self.edges
    }

    /// Every vertex's name, by id.
    pub fn vertices(&self) -> &Column<Name> {
        &self.vertices
    }
}
