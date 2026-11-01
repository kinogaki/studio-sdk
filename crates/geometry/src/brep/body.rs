//! [`Body`]: one B-rep body, and [`Placement`]: where it is drawn.
use super::consistency;
use super::geometry::Geometry;
use super::names::Name;
use super::topology::{BodyKind, BrepTopology, Names};
use crate::attributes::{Attributes, Class};
use crate::column::Column;
use crate::error::GeometryError;
use crate::limits::MAX_COORDINATE;
use std::sync::Arc;

/// How far `RᵀR` may stray from the identity for a placement to be
/// rigid: rotations composed from angles, or read from a file's rounded
/// decimals, stay within it; a scale or a shear does not. A structural
/// check, not a modeling tolerance.
const RIGID_SLACK: f64 = 1e-9;

/// A rigid placement: a rotation and a translation, as rows `[R | t]`. A
/// point `p` is placed at `R·p + t`.
///
/// ```
/// use kinogaki_geometry::brep::Placement;
///
/// let raised = Placement::new([[1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 5.0]]);
/// assert_eq!(raised.unwrap().apply([1.0, 2.0, 3.0]), [1.0, 2.0, 8.0]);
/// let mirror = Placement::new([[-1.0, 0.0, 0.0, 0.0], [0.0, 1.0, 0.0, 0.0], [0.0, 0.0, 1.0, 0.0]]);
/// assert!(mirror.is_err());
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Placement([[f64; 4]; 3]);

impl Placement {
    /// No motion.
    pub const IDENTITY: Placement = Placement([
        [1.0, 0.0, 0.0, 0.0],
        [0.0, 1.0, 0.0, 0.0],
        [0.0, 0.0, 1.0, 0.0],
    ]);

    /// The placement of rows `[R | t]`, checked rigid: `RᵀR` within 1e-9
    /// of the identity in each entry and `det R > 0`.
    ///
    /// # Errors
    /// A value not finite, or a linear part that isn't a rotation.
    pub fn new(rows: [[f64; 4]; 3]) -> Result<Self, GeometryError> {
        let column = |j: usize| [rows[0][j], rows[1][j], rows[2][j]];
        let columns = [column(0), column(1), column(2)];
        let dot = |a: [f64; 3], b: [f64; 3]| a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
        let orthonormal = (0..3).all(|i| {
            (0..3).all(|j| {
                let want = if i == j { 1.0 } else { 0.0 };
                (dot(columns[i], columns[j]) - want).abs() <= RIGID_SLACK
            })
        });
        let det = dot(columns[0], crate::vector::cross(columns[1], columns[2]));
        let finite = rows.iter().flatten().all(|c| c.abs() <= MAX_COORDINATE);
        if !(finite && orthonormal && det > 0.0) {
            return Err(GeometryError::brep(
                None,
                "placement not rigid",
                "a placement must be a rotation and a translation".into(),
            ));
        }
        Ok(Self(rows))
    }

    /// The rows `[R | t]`.
    pub fn rows(&self) -> &[[f64; 4]; 3] {
        &self.0
    }

    /// Where `p` is placed: `R·p + t`.
    pub fn apply(&self, p: [f64; 3]) -> [f64; 3] {
        self.0
            .map(|r| r[0] * p[0] + r[1] * p[1] + r[2] * p[2] + r[3])
    }
}

/// One B-rep body: exact geometry, its topology, the entities' persistent
/// names, attributes and placements, and the positional tolerance it was
/// built to. Made by [`BrepBuilder`](super::BrepBuilder); the rules are
/// [`super`]'s.
///
/// **Classes** ([`count`](Self::count)): Object (the body: one element),
/// Instance (each placement, the first included), Primitive (each face,
/// in face order), Edge (each edge, in edge order) and Point (each
/// vertex). Piece and Vertex (a polygon's corner) aren't used. Standard
/// names: `name` (Instance: the occurrence path), `part` (Object), `Cd`
/// and `material` (Object and Primitive).
///
/// **Sharing.** Immutable and cheap to clone. The geometry, topology,
/// names and attributes are each shared (`Arc`), and the `with_*` edits
/// share what they don't replace: a CV drag
/// ([`with_geometry`](Self::with_geometry)) keeps the topology and every
/// cache derived from it.
#[derive(Clone, Debug, PartialEq)]
pub struct Body {
    name: Name,
    placement: Placement,
    instances: Column<Placement>,
    tolerance: f64,
    geometry: Arc<Geometry>,
    topology: Arc<BrepTopology>,
    names: Arc<Names>,
    attributes: Arc<Attributes>,
}

impl Body {
    /// A body of checked topology over `geometry`, checking the rest.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn checked(
        name: Name,
        tolerance: f64,
        placement: Placement,
        instances: Column<Placement>,
        geometry: Arc<Geometry>,
        topology: Arc<BrepTopology>,
        names: Arc<Names>,
        attributes: Arc<Attributes>,
    ) -> Result<Self, GeometryError> {
        if !(tolerance.is_finite() && tolerance > 0.0) {
            return Err(GeometryError::brep(
                None,
                "tolerance not positive",
                "a body's positional tolerance must be finite and positive".into(),
            ));
        }
        consistency::check(&topology, &geometry, tolerance)?;
        let body = Self {
            name,
            placement,
            instances,
            tolerance,
            geometry,
            topology,
            names,
            attributes,
        };
        body.check_attributes()?;
        Ok(body)
    }

    fn check_attributes(&self) -> Result<(), GeometryError> {
        self.attributes.check("bodies", |class| self.count(class))
    }

    /// The body's persistent name.
    pub fn name(&self) -> &Name {
        &self.name
    }

    /// Where the body is drawn first, and where its picks resolve.
    pub fn placement(&self) -> &Placement {
        &self.placement
    }

    /// Where else it is drawn (assembly instances).
    pub fn instances(&self) -> &Column<Placement> {
        &self.instances
    }

    /// The positional tolerance P it was built to, in model units: within
    /// it, two points are one.
    pub fn tolerance(&self) -> f64 {
        self.tolerance
    }

    /// `Solid` when every shell is closed, else `Sheet`.
    pub fn kind(&self) -> BodyKind {
        if self.topology.shells.iter().all(|s| s.closed) {
            BodyKind::Solid
        } else {
            BodyKind::Sheet
        }
    }

    /// The carriers.
    pub fn geometry(&self) -> &Arc<Geometry> {
        &self.geometry
    }

    /// The topology.
    pub fn topology(&self) -> &Arc<BrepTopology> {
        &self.topology
    }

    /// The faces', edges' and vertices' names.
    pub fn names(&self) -> &Arc<Names> {
        &self.names
    }

    /// The attributes and groups.
    pub fn attributes(&self) -> &Arc<Attributes> {
        &self.attributes
    }

    /// How many elements `class` has: 1 for Object, the placements for
    /// Instance, the faces for Primitive, the edges for Edge, the vertices
    /// for Point; `None` for a class a body doesn't have.
    pub fn count(&self, class: Class) -> Option<usize> {
        let t = &self.topology;
        Some(match class {
            Class::Object => 1,
            Class::Instance => 1 + self.instances.len(),
            Class::Primitive => t.faces.len(),
            Class::Edge => t.edges.len(),
            Class::Point => t.vertices.len(),
            _ => return None,
        })
    }

    /// This body over `geometry` (tables of the same sizes: a CV drag,
    /// its corners moved with it),
    /// sharing the rest. The consistency rules are checked again; the
    /// topology isn't.
    ///
    /// # Errors
    /// A table of another size, or a consistency rule broken.
    pub fn with_geometry(&self, geometry: Geometry) -> Result<Self, GeometryError> {
        let (a, b) = (&self.geometry, &geometry);
        if a.points().len() != b.points().len()
            || a.surfaces().len() != b.surfaces().len()
            || a.curves().len() != b.curves().len()
            || a.pcurves().len() != b.pcurves().len()
        {
            return Err(GeometryError::brep(
                None,
                "geometry resized",
                "a body's new geometry has the same number of points, surfaces, curves and p-curves".into(),
            ));
        }
        consistency::check(&self.topology, &geometry, self.tolerance)?;
        Ok(Self {
            geometry: Arc::new(geometry),
            ..self.clone()
        })
    }

    /// This body with `attributes`, sharing the rest.
    ///
    /// # Errors
    /// A missized or misplaced attribute.
    pub fn with_attributes(&self, attributes: Attributes) -> Result<Self, GeometryError> {
        let body = Self {
            attributes: Arc::new(attributes),
            ..self.clone()
        };
        body.check_attributes()?;
        Ok(body)
    }

    /// This body at `placement` and `instances`, sharing the rest.
    ///
    /// # Errors
    /// Instance attributes sized for another number of placements.
    pub fn with_placement(
        &self,
        placement: Placement,
        instances: Column<Placement>,
    ) -> Result<Self, GeometryError> {
        let body = Self {
            placement,
            instances,
            ..self.clone()
        };
        body.check_attributes()?;
        Ok(body)
    }

    pub(crate) fn vertex_count(&self) -> usize {
        self.topology.vertices.len()
    }

    pub(crate) fn edge_count(&self) -> usize {
        self.topology.edges.len()
    }

    pub(crate) fn coedge_count(&self) -> usize {
        self.topology.coedges.len()
    }

    pub(crate) fn loop_count(&self) -> usize {
        self.topology.loops.len()
    }

    pub(crate) fn face_count(&self) -> usize {
        self.topology.faces.len()
    }

    pub(crate) fn shell_count(&self) -> usize {
        self.topology.shells.len()
    }

    pub(crate) fn lump_count(&self) -> usize {
        self.topology.lumps.len()
    }

    pub(crate) fn surface_count(&self) -> usize {
        self.geometry.surfaces().len()
    }

    pub(crate) fn curve_count(&self) -> usize {
        self.geometry.curves().len()
    }

    pub(crate) fn pcurve_count(&self) -> usize {
        self.geometry.pcurves().len()
    }
}
