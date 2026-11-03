//! [`Primitives`]: the one primitive table polygons, polylines and curves
//! share — rows of point ids with a [`Kind`] each, in any order — and
//! what is derived from it once: the canonical [`Index`] of edges and
//! adjacency, the edge count, the polygons and the
//! [`fingerprint`](Primitives::fingerprint).
mod components;
mod curves;
mod fingerprint;
mod index;
mod kind;
pub(crate) mod validate;
mod writer;

pub use components::Components;
pub use curves::{CurveData, Knots};
pub use index::{Index, Walk};
pub use kind::{Basis, Kind};
pub use writer::TableWriter;

use crate::Error;
use crate::column::Column;
use crate::derived::Derived;
use crate::elements::Elements;
use std::fmt;
use std::ops::Range;
use std::sync::{Arc, OnceLock};

/// One unique edge: the two points it joins, lower id first. The edges of
/// a table are the consecutive vertex pairs of every primitive (wrapping
/// when it is closed): polygon sides, polyline segments and curve hulls,
/// each once, in ascending `(low, high)` order ([`Primitives::edges`]); an
/// edge's index in that order is its id.
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

/// Every primitive's kind: one for all (the common all-polygon table, no
/// column), or one each.
#[derive(Clone, Debug)]
pub(crate) enum Kinds {
    All(Kind),
    Each(Column<Kind>),
}

/// Primitives as rows of point ids, each of a [`Kind`]: polygons,
/// polylines and curves over shared points, in any order.
///
/// Primitive `p`'s vertices are `vertices[offsets[p]..offsets[p + 1]]`,
/// each naming a point. A Merge appends; nothing is renumbered. Code that
/// works on polygons iterates [`polygons`](Self::polygons), which costs
/// nothing when every primitive is one.
///
/// **Rules** ([`Primitives::new`] checks them):
/// - `offsets` starts at 0, never decreases and ends at `vertices.len()`;
///   one kind per primitive;
/// - every vertex names a point below `points`;
/// - a polygon has at least 3 vertices and never repeats a point; a
///   polyline at least 2 and never repeats a point consecutively;
/// - a curve follows its basis's rules: its degree (1 to
///   [`MAX_DEGREE`](crate::limits::MAX_DEGREE); 3 for Catmull-Rom), at
///   least 2 control points (`degree + 1` for B-spline and NURBS; an open
///   Bézier curve `k·degree + 1`, a closed one `k·degree`), and for NURBS
///   the knot rule the B-rep's NURBS share (`count + degree + 1` knots,
///   finite, never decreasing, spanning an interval) and weights above 0;
///   other primitives have degree 1, no knots and weight 1;
/// - [`CurveData`] is present exactly when a primitive is a curve;
/// - the counts are within the [product limits](crate::limits).
///
/// A table is immutable and shared (`Arc<Primitives>` in a value): a
/// points-only edit keeps it, with everything derived from it.
pub struct Primitives {
    points: usize,
    offsets: Column<u32>,
    vertices: Column<u32>,
    kinds: Kinds,
    curves: Option<Arc<CurveData>>,
    polygons: OnceLock<Elements>,
    index: OnceLock<Index>,
    fingerprint: OnceLock<[u8; 32]>,
    /// Its cells' digests, once its fingerprint is asked for.
    digests: OnceLock<fingerprint::Digests>,
    /// The rows crossing from one run of the vertices to another.
    stitches: OnceLock<Vec<(u32, Box<[u32]>)>>,
    derived: Derived,
}

impl Primitives {
    /// The primitives in `offsets` and `vertices` over `points` points,
    /// of `kinds` (one each), with `curves` when any is a curve, checked.
    /// Large tables are checked in parallel; the error is the first fault
    /// in primitive order either way.
    ///
    /// ```
    /// use kinogaki_geometry::{Kind, Primitives};
    ///
    /// // A polyline from a quad's corner, then the quad.
    /// let kinds = vec![Kind::POLYLINE, Kind::POLYGON];
    /// let t = Primitives::new(5, vec![0, 2, 6].into(), vec![2, 4, 0, 1, 2, 3].into(), kinds.into(), None).unwrap();
    /// assert_eq!((t.len(), t.polygons().len(), t.edge_count()), (2, 1, 5));
    /// assert_eq!(t.polygons().iter().collect::<Vec<_>>(), [1]);
    /// let bad = Primitives::of_polygons(4, vec![0, 3].into(), vec![0, 1, 1].into()).unwrap_err();
    /// assert_eq!(bad.message(), "polygon 0 repeats a point");
    /// ```
    ///
    /// # Errors
    /// The first rule broken, naming the primitive.
    pub fn new(
        points: usize,
        offsets: Column<u32>,
        vertices: Column<u32>,
        kinds: Column<Kind>,
        curves: Option<CurveData>,
    ) -> Result<Self, Error> {
        let primitives = offsets.len().saturating_sub(1);
        if kinds.len() != primitives {
            return Err(Error::invalid(format!(
                "{} kinds for {primitives} primitives",
                kinds.len()
            )));
        }
        let kinds = match kinds.first() {
            Some(&first) if kinds.iter().all(|&k| k == first) => Kinds::All(first),
            Some(_) => Kinds::Each(kinds),
            None => Kinds::All(Kind::POLYGON),
        };
        validate::primitives(
            points,
            offsets.as_slice(),
            vertices.as_slice(),
            &kinds,
            primitives,
            curves.as_ref(),
        )?;
        Ok(Self::unchecked(
            points,
            offsets,
            vertices,
            kinds,
            curves.map(Arc::new),
        ))
    }

    /// Geometry alone, checked: what most values are, held without a
    /// kind column.
    ///
    /// # Errors
    /// As [`new`](Self::new).
    pub fn of_polygons(
        points: usize,
        offsets: Column<u32>,
        vertices: Column<u32>,
    ) -> Result<Self, Error> {
        let kinds = Kinds::All(Kind::POLYGON);
        let primitives = offsets.len().saturating_sub(1);
        validate::primitives(
            points,
            offsets.as_slice(),
            vertices.as_slice(),
            &kinds,
            primitives,
            None,
        )?;
        Ok(Self::unchecked(points, offsets, vertices, kinds, None))
    }

    fn unchecked(
        points: usize,
        offsets: Column<u32>,
        vertices: Column<u32>,
        kinds: Kinds,
        curves: Option<Arc<CurveData>>,
    ) -> Self {
        Self {
            points,
            offsets,
            vertices,
            kinds,
            curves,
            polygons: OnceLock::new(),
            index: OnceLock::new(),
            fingerprint: OnceLock::new(),
            digests: OnceLock::new(),
            stitches: OnceLock::new(),
            derived: Derived::default(),
        }
    }

    /// These primitives over `points` points: the same rows, sharing their
    /// columns, so only the count is checked. What adding points no
    /// primitive names yet makes of a table. Its fingerprint and caches are
    /// its own (they speak of the points); its index shares what the first
    /// one built.
    ///
    /// # Errors
    /// Fewer points than now, or past the limit.
    pub fn with_points(&self, points: usize) -> Result<Self, Error> {
        crate::limits::check(points, crate::limits::MAX_POINTS, "points")?;
        if points < self.points {
            return Err(Error::invalid(format!(
                "{points} points for a table of {}",
                self.points
            )));
        }
        let table = Self::unchecked(
            points,
            self.offsets.clone(),
            self.vertices.clone(),
            self.kinds.clone(),
            self.curves.clone(),
        );
        if let Some(index) = self.index.get() {
            let _ = table.index.set(index::with_points(index, &table));
        }
        if let Some(digests) = self.digests.get() {
            let _ = table.digests.set(digests.clone());
        }
        Ok(table)
    }

    /// No primitives over no points.
    pub fn empty() -> Self {
        let kinds = Kinds::All(Kind::POLYGON);
        Self::unchecked(0, Column::new(vec![0]), Column::default(), kinds, None)
    }

    /// The points its vertices name: what its value's positions number.
    pub fn points(&self) -> usize {
        self.points
    }

    /// How many primitives.
    pub fn len(&self) -> usize {
        self.offsets.len() - 1
    }

    /// Whether it holds no primitive.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Primitive `p`'s kind.
    #[inline]
    pub fn kind(&self, p: usize) -> Kind {
        match &self.kinds {
            Kinds::All(kind) => *kind,
            Kinds::Each(kinds) => kinds[p],
        }
    }

    /// Every primitive's kind, one each, as a file saves them.
    pub fn kinds(&self) -> Column<Kind> {
        match &self.kinds {
            Kinds::All(kind) => Column::new(vec![*kind; self.len()]),
            Kinds::Each(kinds) => kinds.clone(),
        }
    }

    /// The kind every primitive has, when they all have one.
    pub fn uniform(&self) -> Option<Kind> {
        match self.kinds {
            Kinds::All(kind) => Some(kind),
            Kinds::Each(_) => None,
        }
    }

    /// The polygon primitives, a set of the primitives (every one, at no
    /// cost, when all are polygons); triangulation and shading cover these.
    pub fn polygons(&self) -> &Elements {
        self.polygons.get_or_init(|| match &self.kinds {
            Kinds::All(kind) if kind.is_polygon() => Elements::all(self.len()),
            Kinds::All(_) => Elements::none(self.len()),
            Kinds::Each(kinds) => {
                Elements::from_fn(kinds.len(), |p| kinds[p as usize].is_polygon())
            }
        })
    }

    /// The curves' degrees, knots and weights, when a primitive is a
    /// curve.
    pub fn curves(&self) -> Option<&Arc<CurveData>> {
        self.curves.as_ref()
    }

    /// Every vertex: the corners of all primitives.
    pub fn vertex_count(&self) -> usize {
        self.vertices.len()
    }

    /// Where each primitive's vertices start, and the vertex count last.
    pub fn offsets(&self) -> &Column<u32> {
        &self.offsets
    }

    /// Each vertex's point.
    pub fn vertices(&self) -> &Column<u32> {
        &self.vertices
    }

    /// Primitive `p`'s vertices' points.
    ///
    /// # Panics
    /// When `p` is not a primitive.
    #[inline]
    pub fn primitive(&self, p: usize) -> &[u32] {
        let row = self.offsets[p] as usize..self.offsets[p + 1] as usize;
        match self.vertices.run(row) {
            Some(row) => row,
            None => self.stitched(p),
        }
    }

    /// The row of `p`, which crosses from one run of the vertices to
    /// another: a copy kept with the table, one per crossing at most (a few
    /// for an edit's table, none for a table made in one piece).
    #[cold]
    fn stitched(&self, p: usize) -> &[u32] {
        let stitches = self.stitches.get_or_init(|| {
            let mut rows: Vec<usize> = (self.vertices.breaks())
                .map(|b| self.offsets.partition_point(|&o| o as usize <= b) - 1)
                .collect();
            rows.dedup();
            (rows.into_iter())
                .filter(|&row| row + 1 < self.offsets.len())
                .map(|row| {
                    let (a, b) = (self.offsets[row] as usize, self.offsets[row + 1] as usize);
                    let corners = (a..b).map(|c| self.vertices[c]).collect();
                    (row as u32, corners)
                })
                .collect::<Vec<(u32, Box<[u32]>)>>()
        });
        let at = stitches.partition_point(|row| (row.0 as usize) < p);
        &stitches[at].1
    }

    /// Primitive `p`'s vertex ids.
    #[inline]
    pub fn primitive_vertices(&self, p: usize) -> Range<usize> {
        self.offsets[p] as usize..self.offsets[p + 1] as usize
    }

    /// The canonical index of edges and adjacency, built the first time it
    /// is asked for (in parallel on large tables) and kept as long as the
    /// table. A table an edit made has it from its source's when that was
    /// built, at the cost of the rows the edit changed.
    pub fn index(&self) -> &Index {
        self.index.get_or_init(|| index::build(self))
    }

    /// The unique edges in canonical order: each primitive's consecutive
    /// vertex pairs (wrapping when it is closed), once each, ascending by
    /// `(low point, high point)`. An edge's position here is its id, and
    /// [`Domain::Edge`](crate::Domain::Edge) attributes follow this order.
    pub fn edges(&self) -> impl Iterator<Item = Edge> + '_ {
        self.index().edges()
    }

    /// Edge `e`.
    ///
    /// # Panics
    /// When `e` is not an edge.
    pub fn edge(&self, e: usize) -> Edge {
        self.index().edge(e)
    }

    /// How many unique edges: what a value's Edge domain checks its columns
    /// and groups against.
    pub fn edge_count(&self) -> usize {
        self.index().edge_count()
    }

    /// The id of the edge joining points `a` and `b`, either way round.
    pub fn edge_of(&self, a: u32, b: u32) -> Option<u32> {
        self.index().edge_of(a, b)
    }

    /// The connected components of its points, found the first time they
    /// are asked for (without the [`index`](Self::index)) and kept as long
    /// as the table.
    pub fn components(&self) -> Arc<Components> {
        self.derived(components::of)
    }

    /// A 32-byte content hash of the points count, the offsets, the
    /// vertices, the kinds and the curve data (BLAKE3 of their
    /// little-endian bytes, cell by cell): equal tables hash equal, in every
    /// session. Computed once; a table an edit made hashes only the cells
    /// it doesn't share.
    pub fn fingerprint(&self) -> [u8; 32] {
        *self.fingerprint.get_or_init(|| fingerprint::of(self))
    }

    /// The `D` derived from this table, built by `build` the first time
    /// any holder asks; shared by every value holding this table, and
    /// dropped with it. Only the crate that defines `D` asks for it (the
    /// crate's cache rule).
    pub fn derived<D: Send + Sync + 'static>(
        &self,
        build: impl FnOnce(&Primitives) -> D,
    ) -> Arc<D> {
        self.derived.get_or_build(|| build(self))
    }
}

/// Tables are equal when their primitives are.
impl PartialEq for Primitives {
    fn eq(&self, other: &Self) -> bool {
        let kinds = match (&self.kinds, &other.kinds) {
            (Kinds::All(a), Kinds::All(b)) => a == b || self.is_empty(),
            (Kinds::Each(a), Kinds::Each(b)) => a == b,
            _ => false,
        };
        self.points == other.points
            && kinds
            && self.offsets == other.offsets
            && self.vertices == other.vertices
            && self.curves == other.curves
    }
}

impl fmt::Debug for Primitives {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Primitives")
            .field("points", &self.points)
            .field("primitives", &self.len())
            .field("polygons", &self.polygons().len())
            .field("vertices", &self.vertices.len())
            .finish_non_exhaustive()
    }
}
