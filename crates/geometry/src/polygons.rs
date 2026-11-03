//! [`Polygons`]: polygons and polylines over shared points, with
//! attributes — the standard value `kinogaki.polygons`.
use crate::attributes::{Attributes, Domain};
use crate::column::Column;
use crate::error::GeometryError;
use crate::positions;
use crate::primitives::Primitives;
use std::sync::Arc;

/// Polygons, polylines and curves in one value: positions, a shared
/// [`Primitives`] table and [`Attributes`].
///
/// **Domains** ([`count`](Self::count)): Point (each position), Vertex
/// (each corner), Primitive (each polygon or polyline), Edge (each
/// canonical edge), Detail (one). It has no Object or Instance domain.
///
/// **Rules** ([`Polygons::new`] checks them, beside the table's own):
/// - one position per point of the table, each finite and at most
///   1e100 in each component;
/// - every attribute and group has one entry per element of its domain
///   (its values checked themselves when it was made);
/// - no Object or Instance attributes.
///
/// **Sharing.** A value is immutable and cheap to clone. The `with_*`
/// edits return a new value sharing everything they don't replace, so a
/// points-only edit keeps the table and every cache derived from it
/// (the crate's cache rule), and [`with_attributes`](Self::with_attributes)
/// checks only the entries it replaced. Conventions: `.hidden` (a
/// Primitive group) hides primitives.
///
/// ```
/// use kinogaki_geometry::{Attributes, Domain, Polygons, Primitives};
///
/// let quad = Primitives::of_polygons(4, vec![0, 4].into(), vec![0, 1, 2, 3].into()).unwrap();
/// let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]];
/// let square = Polygons::new(positions.into(), quad, Attributes::new()).unwrap();
/// assert_eq!(square.count(Domain::Edge), Some(4));
///
/// // Move a point: the table is shared, not checked or rebuilt.
/// let mut moved = square.positions().to_vec();
/// moved[2][2] = 1.0;
/// let lifted = square.with_positions(moved.into()).unwrap();
/// assert!(std::sync::Arc::ptr_eq(lifted.primitives(), square.primitives()));
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Polygons {
    positions: Column<[f64; 3]>,
    primitives: Arc<Primitives>,
    attributes: Arc<Attributes>,
}

impl Polygons {
    /// `positions` over `primitives`, with `attributes`, checked.
    ///
    /// # Errors
    /// The first rule broken.
    pub fn new(
        positions: Column<[f64; 3]>,
        primitives: Primitives,
        attributes: Attributes,
    ) -> Result<Self, GeometryError> {
        Self::with_primitives(positions, Arc::new(primitives), Arc::new(attributes))
    }

    /// `positions` and `attributes` over a table already checked
    /// (another value's): only the positions and the attributes' sizes are
    /// checked, never the table again. What a kernel uses when an edit
    /// keeps the primitives of its input.
    ///
    /// # Errors
    /// A bad position, a position count not the table's points, or a
    /// missized or misplaced attribute.
    pub fn with_primitives(
        positions: Column<[f64; 3]>,
        primitives: Arc<Primitives>,
        attributes: Arc<Attributes>,
    ) -> Result<Self, GeometryError> {
        check_positions(&positions, &primitives)?;
        check_attributes(&attributes, None, &primitives)?;
        Ok(Self {
            positions,
            primitives,
            attributes,
        })
    }

    /// This value with its points at `positions`, sharing the table and
    /// the attributes. Only the positions are checked.
    ///
    /// # Errors
    /// A bad position, or not one per point.
    pub fn with_positions(&self, positions: Column<[f64; 3]>) -> Result<Self, GeometryError> {
        check_positions(&positions, &self.primitives)?;
        Ok(Self {
            positions,
            primitives: self.primitives.clone(),
            attributes: self.attributes.clone(),
        })
    }

    /// This value with `attributes`, sharing the positions and the
    /// primitives. Only the entries this value doesn't already hold (the
    /// same shared attribute or group under the same name) are checked, so
    /// replacing one column checks that column alone.
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
    pub(crate) fn with_shared_attributes(
        &self,
        attributes: Arc<Attributes>,
    ) -> Result<Self, GeometryError> {
        check_attributes(&attributes, Some(&self.attributes), &self.primitives)?;
        Ok(Self {
            positions: self.positions.clone(),
            primitives: self.primitives.clone(),
            attributes,
        })
    }

    /// No points and no primitives.
    pub fn empty() -> Self {
        Self {
            positions: Column::default(),
            primitives: Arc::new(Primitives::empty()),
            attributes: Arc::default(),
        }
    }

    /// The points' positions.
    pub fn positions(&self) -> &Column<[f64; 3]> {
        &self.positions
    }

    /// The primitives.
    pub fn primitives(&self) -> &Arc<Primitives> {
        &self.primitives
    }

    /// The attributes and groups.
    pub fn attributes(&self) -> &Arc<Attributes> {
        &self.attributes
    }

    /// How many elements `domain` has: points, vertices, primitives,
    /// edges (the canonical edges, derived once from the table), 1 for
    /// Detail; `None` for a domain it doesn't have.
    pub fn count(&self, domain: Domain) -> Option<usize> {
        count(&self.primitives, domain)
    }

    /// The smallest box holding every position; `None`
    /// without points. Cached on the positions column.
    pub fn bounds(&self) -> Option<crate::Bounds> {
        positions::cached_bounds(&self.positions)
    }

    /// Whether `self` and `other` are one value: the same positions,
    /// primitives and attribute allocations (one is a clone of the other),
    /// known without comparing them.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        self.positions.ptr_eq(&other.positions)
            && Arc::ptr_eq(&self.primitives, &other.primitives)
            && Arc::ptr_eq(&self.attributes, &other.attributes)
    }

    /// The points whose position differs from `before`'s bit for bit,
    /// ascending: what moved between two values of one table (a drag's
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

fn count(primitives: &Primitives, domain: Domain) -> Option<usize> {
    Some(match domain {
        Domain::Point => primitives.points(),
        Domain::Vertex => primitives.vertex_count(),
        Domain::Primitive => primitives.len(),
        Domain::Edge => primitives.edge_count(),
        Domain::Detail => 1,
        _ => return None,
    })
}

fn check_positions(
    positions: &Column<[f64; 3]>,
    primitives: &Primitives,
) -> Result<(), GeometryError> {
    if positions.len() != primitives.points() {
        return Err(GeometryError::whole(
            crate::ErrorKind::Position,
            format!(
                "{} positions for a table of {} points",
                positions.len(),
                primitives.points()
            ),
        ));
    }
    positions::check(positions)
}

fn check_attributes(
    attributes: &Attributes,
    previous: Option<&Attributes>,
    primitives: &Primitives,
) -> Result<(), GeometryError> {
    attributes.check("polygons", previous, |domain| count(primitives, domain))?;
    Ok(())
}
