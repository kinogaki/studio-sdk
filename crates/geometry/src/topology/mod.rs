//! [`Topology`]: the primitives of a [`Polygons`](crate::Polygons) value,
//! polygons then polylines, as one compressed row table; and what is
//! derived from it once: the canonical [`Index`] of edges and adjacency,
//! and the [`fingerprint`](Topology::fingerprint).
mod fingerprint;
mod index;
mod validate;

pub use index::{Adjacency, Index};

use crate::column::Column;
use crate::derived::Derived;
use crate::error::GeometryError;
use std::fmt;
use std::ops::Range;
use std::sync::{Arc, OnceLock};

/// One unique edge: the two points it joins, lower id first. The edges of
/// a topology are its polygons' sides and its polylines' segments, each
/// once, in ascending `(low, high)` order ([`Topology::edges`]); an edge's
/// index in that order is its id.
#[repr(C)]
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    bytemuck::Pod,
    bytemuck::Zeroable,
)]
pub struct Edge {
    /// The points, `points[0] < points[1]`.
    pub points: [u32; 2],
}

/// Primitives as rows of point ids: the first [`polygons`](Self::polygons)
/// are closed polygons, the rest open polylines.
///
/// Primitive `p`'s vertices are `vertices[offsets[p]..offsets[p + 1]]`,
/// each naming a point. Because polygons come first, the polygons'
/// primitive ids and vertex ids are those of a polygon-only value, and a
/// loop over polygons leaves the polylines alone.
///
/// **Rules** ([`Topology::new`] checks them):
/// - `offsets` starts at 0, never decreases and ends at `vertices.len()`;
/// - `polygons` is at most the number of primitives;
/// - a polygon has at least 3 vertices and never repeats a point;
/// - a polyline has at least 2 vertices and never repeats a point
///   consecutively (it may come back to one);
/// - every vertex names a point below `points`;
/// - the counts are within the [product limits](crate::limits).
///
/// A topology is immutable and shared (`Arc<Topology>` in a value): a
/// points-only edit keeps it, with everything derived from it.
pub struct Topology {
    points: usize,
    offsets: Column<u32>,
    vertices: Column<u32>,
    polygons: usize,
    index: OnceLock<Index>,
    fingerprint: OnceLock<[u8; 32]>,
    derived: Derived,
}

impl Topology {
    /// The primitives in `offsets` and `vertices` over `points` points, the
    /// first `polygons` of them polygons, checked. Large topologies are
    /// checked in parallel; the error is the first fault in primitive
    /// order either way.
    ///
    /// ```
    /// use kinogaki_geometry::Topology;
    ///
    /// // A quad, then a polyline from its corner.
    /// let t = Topology::new(5, vec![0, 4, 6].into(), vec![0, 1, 2, 3, 2, 4].into(), 1).unwrap();
    /// assert_eq!((t.primitives(), t.polygons(), t.edges().len()), (2, 1, 5));
    /// let bad = Topology::new(4, vec![0, 3].into(), vec![0, 1, 1].into(), 1).unwrap_err();
    /// assert_eq!(bad.message(), "polygon 0 repeats a point");
    /// ```
    ///
    /// # Errors
    /// The first rule broken, naming the primitive.
    pub fn new(
        points: usize,
        offsets: Column<u32>,
        vertices: Column<u32>,
        polygons: usize,
    ) -> Result<Self, GeometryError> {
        validate::topology(points, &offsets, &vertices, polygons)?;
        Ok(Self::unchecked(points, offsets, vertices, polygons))
    }

    fn unchecked(
        points: usize,
        offsets: Column<u32>,
        vertices: Column<u32>,
        polygons: usize,
    ) -> Self {
        Self {
            points,
            offsets,
            vertices,
            polygons,
            index: OnceLock::new(),
            fingerprint: OnceLock::new(),
            derived: Derived::default(),
        }
    }

    /// No primitives over no points.
    pub fn empty() -> Self {
        Self::unchecked(0, Column::new(vec![0]), Column::default(), 0)
    }

    /// The points its vertices name: what its value's positions number.
    pub fn points(&self) -> usize {
        self.points
    }

    /// Polygons and polylines.
    pub fn primitives(&self) -> usize {
        self.offsets.len() - 1
    }

    /// The polygons: primitives `0..polygons()`.
    pub fn polygons(&self) -> usize {
        self.polygons
    }

    /// The polylines' primitive ids: they follow the polygons.
    pub fn polylines(&self) -> Range<usize> {
        self.polygons..self.primitives()
    }

    /// Whether primitive `p` is a polyline.
    pub fn is_polyline(&self, p: usize) -> bool {
        p >= self.polygons
    }

    /// Every vertex: the corners of all primitives.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// The polygons' vertices are vertex ids `0..polygon_vertices()`;
    /// polyline vertices follow.
    pub fn polygon_vertices(&self) -> usize {
        self.offsets[self.polygons] as usize
    }

    /// Where each primitive's vertices start, and the vertex count last.
    pub fn offsets(&self) -> &Column<u32> {
        &self.offsets
    }

    /// Each vertex's point.
    pub fn vertices(&self) -> &Column<u32> {
        &self.vertices
    }

    /// Primitive `p`'s vertices' points, polygon or polyline.
    ///
    /// # Panics
    /// When `p` is not a primitive.
    #[inline]
    pub fn primitive(&self, p: usize) -> &[u32] {
        &self.vertices[self.offsets[p] as usize..self.offsets[p + 1] as usize]
    }

    /// Primitive `p`'s vertex ids.
    #[inline]
    pub fn primitive_vertices(&self, p: usize) -> Range<usize> {
        self.offsets[p] as usize..self.offsets[p + 1] as usize
    }

    /// The canonical index of edges and adjacency, built the first time it
    /// is asked for (in parallel on large topologies) and kept as long as
    /// the topology.
    pub fn index(&self) -> &Index {
        self.index.get_or_init(|| index::build(self))
    }

    /// The index when it has been built, without building it: for memory
    /// accounting and budgets.
    ///
    /// ```
    /// use kinogaki_geometry::Topology;
    ///
    /// let t = Topology::new(3, vec![0, 3].into(), vec![0, 1, 2].into(), 1).unwrap();
    /// assert!(t.built_index().is_none());
    /// assert_eq!(t.index().edges().len(), 3);
    /// assert!(t.built_index().is_some_and(|i| i.bytes() > 0));
    /// ```
    pub fn built_index(&self) -> Option<&Index> {
        self.index.get()
    }

    /// The unique edges in canonical order: polygon sides (each vertex to
    /// the next, wrapping) and polyline segments, once each, ascending by
    /// `(low point, high point)`. An edge's position here is its id, and
    /// [`Class::Edge`](crate::Class::Edge) attributes follow this order.
    pub fn edges(&self) -> &[Edge] {
        self.index().edges()
    }

    /// The id of the edge joining points `a` and `b`, either way round.
    pub fn edge_of(&self, a: u32, b: u32) -> Option<u32> {
        self.index().edge_of(a, b)
    }

    /// A 32-byte content hash of the points count, the polygon count, the
    /// offsets and the vertices (BLAKE3 of their little-endian bytes):
    /// equal topologies hash equal, in every session. Computed once.
    pub fn fingerprint(&self) -> [u8; 32] {
        *self.fingerprint.get_or_init(|| {
            fingerprint::of(self.points, self.polygons, &self.offsets, &self.vertices)
        })
    }

    /// The `D` derived from this topology, built by `build` the first
    /// time any holder asks; shared by every value holding this topology,
    /// and dropped with it. Only the crate that defines `D` asks for it
    /// (the crate's cache rule).
    pub fn derived<D: Send + Sync + 'static>(&self, build: impl FnOnce(&Topology) -> D) -> Arc<D> {
        self.derived.get_or_build(|| build(self))
    }
}

/// Topologies are equal when their primitives are.
impl PartialEq for Topology {
    fn eq(&self, other: &Self) -> bool {
        self.points == other.points
            && self.polygons == other.polygons
            && self.offsets == other.offsets
            && self.vertices == other.vertices
    }
}

impl fmt::Debug for Topology {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Topology")
            .field("points", &self.points)
            .field("polygons", &self.polygons)
            .field("polylines", &self.polylines().len())
            .field("vertices", &self.vertices.len())
            .finish_non_exhaustive()
    }
}
