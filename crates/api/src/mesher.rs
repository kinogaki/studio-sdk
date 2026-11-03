//! [`Mesher`]: a kernel's display mesh of a B-rep body.
//!
//! A [`Brep`](kinogaki_geometry::brep::Brep) holds exact geometry and no
//! triangles. What the viewport, an engine or an exporter draws of a body
//! is a registered mesher's output: a kernel operation, never part of the
//! value and never saved. Core asks an installed mesher
//! ([`Registry::meshers`](crate::Registry::meshers)) and caches the
//! answer per body and [`Detail`]; with no mesher installed a body shows
//! grey, without faces.
use crate::cancel::CancelToken;
use crate::id::{DataTypeId, MesherId};
use crate::{Diagnostic, Error};
use kinogaki_geometry::brep::Body;
use kinogaki_geometry::{Detail, Geometry, Tolerances};

/// The Primitive i32 attribute naming the B-rep face a polygon of a
/// [`Meshed`] mesh shows (the `FaceId`'s index); `-1` on its polylines.
pub const FACE: &str = "face";

/// The Primitive i32 attribute naming the B-rep edge a polyline of a
/// [`Meshed`] mesh shows (the edge id's index); `-1` on its polygons.
pub const EDGE: &str = "edge";

/// Meshes B-rep bodies for display, rendering and export.
///
/// **The output** ([`Meshed::polygons`]), in the body's own space (the
/// caller applies its placement and instances):
/// - first the faces' polygons, in face order, each with its face in
///   [`FACE`]; then one polyline per edge that has a carrier, in edge
///   order, with its edge in [`EDGE`] (pole edges have none);
/// - **shared points, watertight seams:** points are shared across
///   faces, so two faces meeting at an edge use the same point ids along
///   it, and the edge's polyline runs over those same points. Seams are
///   watertight by equal point ids, not by equal positions;
/// - normals (`N`) and texture coordinates (`uv`), when it makes them,
///   are Vertex attributes defined on the polygons' corners only, so a
///   crease keeps one point and two normals. Polyline corners carry
///   zeros, and consumers ignore them;
/// - `uv` is a texture coordinate. A face's chart coordinates are not
///   `uv`, and the mesh doesn't carry them.
///
/// **Sizes.** [`Detail::Viewport`] is the mesher's own interactive
/// preset for its [`DisplayQuality`](kinogaki_geometry::DisplayQuality),
/// one preset a step, coarse to fine: the project has no display chord
/// or angle, and its tangent tolerance is not one. [`Detail::Render`] gives the chord and angle; a
/// mesher may clamp them to the range it supports, and its documentation
/// states that range. Either way no facet is sized below the chord floor,
/// the larger of the project's positional tolerance (`tolerances`) and
/// the body's own ([`Body::tolerance`]).
///
/// **Failures.** A face it can't mesh is left out of the polygons and
/// listed in [`Meshed::failed`] with a short reason; the rest of the body
/// still shows. A body it can't mesh at all is an error:
/// [`Error::cancelled()`] when `cancel` was raised, otherwise
/// [`Invalid`](crate::ErrorKind::Invalid) error saying why.
///
/// **State.** `mesh` is a function of its arguments. A mesher may keep
/// its own state in the derived slots of the body's
/// [`BrepTopology`](kinogaki_geometry::brep::BrepTopology) and
/// [`Geometry`](kinogaki_geometry::brep::Geometry) (the SDK's cache
/// rule), so a control-point drag (the same topology over new geometry)
/// re-meshes incrementally with nothing more at this interface.
pub trait Mesher: Send + Sync {
    /// Its id (`kinogaki.cad.mesher`).
    fn id(&self) -> MesherId;

    /// What a UI calls it ("CAD").
    fn label(&self) -> &str;

    /// The data type whose values it takes: what the host shows meshed at
    /// a display quality while it is installed. A mesher meshes B-rep
    /// bodies ([`mesh`](Self::mesh)), so it is the standard B-rep's
    /// ([`standard::brep`](crate::standard::brep)).
    fn takes(&self) -> DataTypeId {
        crate::standard::brep().id()
    }

    /// `body` as polygons and edge polylines at `detail`. Long meshing
    /// polls `cancel` and stops with [`Error::cancelled()`].
    ///
    /// # Errors
    /// Cancelled, or a body it can't mesh at all.
    fn mesh(
        &self,
        body: &Body,
        detail: Detail,
        tolerances: &Tolerances,
        cancel: &CancelToken,
    ) -> Result<Meshed, Error>;
}

/// A [`Mesher`]'s answer for one body.
#[derive(Clone, Debug, PartialEq)]
pub struct Meshed {
    /// The faces' polygons, then the edges' polylines, over shared
    /// points ([`Mesher`]).
    pub polygons: Geometry,
    /// The faces it couldn't mesh, in face order: a warning for each, its
    /// element the face's index (`FaceId::index`) and its message a short
    /// phrase for the Console ("trim loops cross"), naming no id or number.
    pub failed: Vec<Diagnostic>,
}
