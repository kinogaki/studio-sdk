//! [`Body`]: one B-rep body.
use super::consistency;
use super::geometry::Geometry;
use super::names::Name;
use super::topology::{BodyKind, BrepTopology, Names};
use crate::attributes::{Attributes, Class};
use crate::column::Column;
use crate::error::GeometryError;
use crate::xform::Xform;
use std::sync::Arc;

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
    placement: Xform,
    instances: Column<Xform>,
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
        placement: Xform,
        instances: Column<Xform>,
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

    /// Its placements rigid ([`Xform::is_rigid`]), its attributes sized.
    fn check_attributes(&self) -> Result<(), GeometryError> {
        if !std::iter::once(&self.placement)
            .chain(self.instances.iter())
            .all(Xform::is_rigid)
        {
            return Err(GeometryError::brep(
                None,
                "placement not rigid",
                "a placement must be a rotation and a translation".into(),
            ));
        }
        self.attributes.check("bodies", |class| self.count(class))
    }

    /// The body's persistent name.
    pub fn name(&self) -> &Name {
        &self.name
    }

    /// Where the body is drawn first, and where its picks resolve.
    pub fn placement(&self) -> &Xform {
        &self.placement
    }

    /// Where else it is drawn (assembly instances).
    pub fn instances(&self) -> &Column<Xform> {
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
    /// A placement that isn't rigid, or instance attributes sized for
    /// another number of placements.
    pub fn with_placement(
        &self,
        placement: Xform,
        instances: Column<Xform>,
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
