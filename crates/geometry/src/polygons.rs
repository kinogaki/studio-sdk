//! [`Polygons`]: polygons and polylines over shared points, with
//! attributes — the standard value `kinogaki.polygons`.
use crate::attributes::{Attributes, Class};
use crate::column::Column;
use crate::error::GeometryError;
use crate::positions;
use crate::topology::Topology;
use std::sync::Arc;

/// Polygons and polylines in one value: positions, a shared [`Topology`]
/// and [`Attributes`].
///
/// **Classes** ([`count`](Self::count)): Point (each position), Vertex
/// (each corner), Primitive and Piece (each polygon or polyline), Edge
/// (each canonical edge), Detail (one). It has no Object or Instance
/// class.
///
/// **Rules** ([`Polygons::new`] checks them, beside the topology's own):
/// - one position per point of the topology, each finite and at most
///   1e100 in each component;
/// - every attribute and group has one entry per element of its class,
///   and every Float and Vector attribute value is finite;
/// - no Object or Instance attributes.
///
/// **Sharing.** A value is immutable and cheap to clone. The `with_*`
/// edits return a new value sharing everything they don't replace, so a
/// points-only edit keeps the topology and every cache derived from it
/// (the crate's cache rule). Conventions: `.hidden` (a Primitive group)
/// hides primitives; a primitive's value is read from Primitive, then
/// Piece.
///
/// ```
/// use kinogaki_geometry::{Attributes, Class, Polygons, Topology};
///
/// let quad = Topology::new(4, vec![0, 4].into(), vec![0, 1, 2, 3].into(), 1).unwrap();
/// let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]];
/// let square = Polygons::new(positions.into(), quad, Attributes::new()).unwrap();
/// assert_eq!(square.count(Class::Edge), Some(4));
///
/// // Move a point: the topology is shared, not checked or rebuilt.
/// let mut moved = square.positions().to_vec();
/// moved[2][2] = 1.0;
/// let lifted = square.with_positions(moved.into()).unwrap();
/// assert!(std::sync::Arc::ptr_eq(lifted.topology(), square.topology()));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Polygons {
    positions: Column<[f64; 3]>,
    topology: Arc<Topology>,
    attributes: Arc<Attributes>,
}

impl Polygons {
    /// `positions` over `topology`, with `attributes`, checked.
    ///
    /// # Errors
    /// The first rule broken.
    pub fn new(
        positions: Column<[f64; 3]>,
        topology: Topology,
        attributes: Attributes,
    ) -> Result<Self, GeometryError> {
        Self::with_topology(positions, Arc::new(topology), Arc::new(attributes))
    }

    /// `positions` and `attributes` over a topology already checked
    /// (another value's): only the positions and the attributes' sizes are
    /// checked, never the topology again. What a kernel uses when an edit
    /// keeps the primitives of its input.
    ///
    /// # Errors
    /// A bad position, a position count not the topology's points, or a
    /// missized or misplaced attribute.
    pub fn with_topology(
        positions: Column<[f64; 3]>,
        topology: Arc<Topology>,
        attributes: Arc<Attributes>,
    ) -> Result<Self, GeometryError> {
        check_positions(&positions, &topology)?;
        check_attributes(&attributes, &topology)?;
        Ok(Self {
            positions,
            topology,
            attributes,
        })
    }

    /// This value with its points at `positions`, sharing the topology and
    /// the attributes. Only the positions are checked.
    ///
    /// # Errors
    /// A bad position, or not one per point.
    pub fn with_positions(&self, positions: Column<[f64; 3]>) -> Result<Self, GeometryError> {
        check_positions(&positions, &self.topology)?;
        Ok(Self {
            positions,
            topology: self.topology.clone(),
            attributes: self.attributes.clone(),
        })
    }

    /// This value with `attributes`, sharing the positions and the
    /// topology. Only the attributes are checked.
    ///
    /// # Errors
    /// A missized or misplaced attribute.
    pub fn with_attributes(&self, attributes: Attributes) -> Result<Self, GeometryError> {
        self.with_shared_attributes(Arc::new(attributes))
    }

    /// [`with_attributes`](Self::with_attributes) with a table shared with
    /// another value.
    ///
    /// # Errors
    /// As [`with_attributes`](Self::with_attributes).
    pub fn with_shared_attributes(
        &self,
        attributes: Arc<Attributes>,
    ) -> Result<Self, GeometryError> {
        check_attributes(&attributes, &self.topology)?;
        Ok(Self {
            positions: self.positions.clone(),
            topology: self.topology.clone(),
            attributes,
        })
    }

    /// No points and no primitives.
    pub fn empty() -> Self {
        Self {
            positions: Column::default(),
            topology: Arc::new(Topology::empty()),
            attributes: Arc::default(),
        }
    }

    /// The points' positions.
    pub fn positions(&self) -> &Column<[f64; 3]> {
        &self.positions
    }

    /// The primitives.
    pub fn topology(&self) -> &Arc<Topology> {
        &self.topology
    }

    /// The attributes and groups.
    pub fn attributes(&self) -> &Arc<Attributes> {
        &self.attributes
    }

    /// How many elements `class` has: points, vertices, primitives (for
    /// Primitive and Piece), edges (the canonical edges, derived once from
    /// the topology), 1 for Detail; `None` for a class it doesn't have.
    pub fn count(&self, class: Class) -> Option<usize> {
        count(&self.topology, class)
    }

    /// The smallest box holding every position (`[min, max]`); `None`
    /// without points. Cached on the positions column.
    pub fn bounds(&self) -> Option<[[f64; 3]; 2]> {
        positions::cached_bounds(&self.positions)
    }

    /// Whether `self` and `other` are one value: the same positions,
    /// topology and attribute allocations (one is a clone of the other),
    /// known without comparing them.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.positions.ptr_eq(&other.positions)
            && Arc::ptr_eq(&self.topology, &other.topology)
            && Arc::ptr_eq(&self.attributes, &other.attributes)
    }

    /// The points whose position differs from `before`'s bit for bit,
    /// ascending: what moved between two values of one topology (a drag's
    /// steps), and what a display patches. Bitwise, so a point moved from
    /// `0.0` to `-0.0` moved (its bytes changed). Points past `before`'s
    /// count are all among them.
    pub fn moved_points(&self, before: &Polygons) -> Vec<u32> {
        if self.positions.ptr_eq(&before.positions) {
            return Vec::new();
        }
        positions::moved(&before.positions, &self.positions)
    }
}

impl Default for Polygons {
    fn default() -> Self {
        Self::empty()
    }
}

fn count(topology: &Topology, class: Class) -> Option<usize> {
    Some(match class {
        Class::Point => topology.points(),
        Class::Vertex => topology.vertex_count(),
        Class::Primitive | Class::Piece => topology.primitives(),
        Class::Edge => topology.edges().len(),
        Class::Detail => 1,
        _ => return None,
    })
}

fn check_positions(positions: &Column<[f64; 3]>, topology: &Topology) -> Result<(), GeometryError> {
    if positions.len() != topology.points() {
        return Err(GeometryError::whole(
            crate::ErrorKind::Position,
            format!(
                "{} positions for a topology of {} points",
                positions.len(),
                topology.points()
            ),
        ));
    }
    positions::check(positions)
}

fn check_attributes(attributes: &Attributes, topology: &Topology) -> Result<(), GeometryError> {
    attributes.check("polygons", |class| count(topology, class))
}
