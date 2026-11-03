//! The standard data types: the geometry values every package shares
//! ([`geometry`](crate::geometry)), as data types.
//!
//! Studio's core registers them, so they exist with no package installed;
//! a kernel never registers a type of its own for the same data. A node
//! reads and writes them through the handles here:
//!
//! ```
//! use kinogaki_studio_api::geometry::Geometry;
//! use kinogaki_studio_api::standard;
//!
//! let data = standard::geometry().wrap(Geometry::empty());
//! assert_eq!(data.ty().as_str(), "kinogaki.core.geometry");
//! assert!(standard::geometry().get(&data).is_some());
//! assert!(standard::curves().get(&data).is_none());
//! ```
//!
//! **Saving.** The project encoding of each type is the SDK's: the codecs
//! here ([`GeometryCodec`], [`CurvesCodec`], [`BrepCodec`], [`CameraCodec`],
//! [`LightCodec`], [`PreviewMaterialsCodec`]) are the ones core registers,
//! and they also save the standard [`Selection`]s a kept result carries.
//! Project files save only frozen and held results, never cooked ones.
//! [`ColorConfiguration`],
//! [`Tolerances`](crate::geometry::Tolerances) and
//! [`DisplayQuality`](crate::geometry::DisplayQuality) are project values
//! core keeps ([`Stored`](crate::Stored), `color`, `tolerances` and
//! `display_quality` in the project's settings). A package keeps project
//! settings of its own the same way: a [`Stored`](crate::Stored) type of
//! [`Scope::Project`](crate::Scope::Project) it registers
//! (`Package::with_stored`), saved in the project under its id.
mod attributes;
mod brep;
mod camera;
mod color;
mod curves;
mod geometry;
mod light;
mod materials;
mod quality;
mod selection;
mod tolerances;

pub use brep::BrepCodec;
pub use camera::{CameraCodec, lens};
pub use color::ColorConfiguration;
pub use curves::CurvesCodec;
pub use geometry::GeometryCodec;
pub use light::LightCodec;
pub use materials::PreviewMaterialsCodec;

use crate::components::Components;
use crate::data::Typed;
use crate::error::Error;
use crate::id::DataTypeId;
use crate::viewport::ElementKind;
use kinogaki_geometry::brep::Brep;
use kinogaki_geometry::{Curves, Domain, Geometry, GeometryError, Selection};
use kinogaki_render_api::ImportedMaterial;

/// The id of [`Geometry`] values: polygons, polylines and curves.
pub(crate) const GEOMETRY: &str = "kinogaki.core.geometry";
/// The id of [`Curves`] values.
pub const CURVES: &str = "kinogaki.core.curves";

/// The id of [`Brep`] values.
pub const BREP: &str = "kinogaki.core.brep";

/// The id of a file's materials as a format reads them beside its
/// geometry ([`preview_materials`]).
pub(crate) const PREVIEW_MATERIALS: &str = "kinogaki.core.preview_materials";

/// The handle on [`Geometry`] values (`GEOMETRY`).
pub const fn geometry() -> Typed<Geometry> {
    Typed::new(DataTypeId::declared(GEOMETRY))
}

/// The handle on [`Curves`] values ([`CURVES`]).
pub const fn curves() -> Typed<Curves> {
    Typed::new(DataTypeId::declared(CURVES))
}

/// The id of camera values: an unplaced lens ([`lens`], [`CameraCodec`]).
pub const CAMERA: &str = "kinogaki.core.camera";

/// The handle on camera values ([`CAMERA`]): render-api's
/// [`Camera`](kinogaki_render_api::Camera), unplaced, in UsdGeomCamera's
/// units; make one with [`lens`].
pub const fn camera() -> Typed<kinogaki_render_api::Camera> {
    Typed::new(DataTypeId::declared(CAMERA))
}

/// The id of light values: an unplaced light ([`light_value`],
/// [`LightCodec`]).
pub(crate) const LIGHT: &str = "kinogaki.core.light";

/// The handle on light values (`LIGHT`): render-api's standard
/// [`Light`](kinogaki_render_api::standard::Light), unplaced, in UsdLux's
/// terms; make one with `light_value`.
pub const fn light() -> Typed<kinogaki_render_api::standard::Light> {
    Typed::new(DataTypeId::declared(LIGHT))
}

/// The handle on [`Brep`] values ([`BREP`]).
pub const fn brep() -> Typed<Brep> {
    Typed::new(DataTypeId::declared(BREP))
}

/// The handle on a file's materials as a format reads them beside its
/// geometry (`PREVIEW_MATERIALS`): each keyed by the name the file's
/// polygons' `material` values give it, with its preview surface.
pub const fn preview_materials() -> Typed<Vec<ImportedMaterial>> {
    Typed::new(DataTypeId::declared(PREVIEW_MATERIALS))
}

/// `selection` of a value of data type `ty`, as the [`Components`] the
/// host keeps, shows and counts.
pub fn components(ty: DataTypeId, selection: Selection) -> Components {
    let level = level(selection.domain());
    Components::new(ty, level, selection.len(), selection)
}

/// The viewport's element kind that shows elements of `domain`: Primitive
/// is faces, Detail and Object whole objects.
pub fn level(domain: Domain) -> ElementKind {
    match domain {
        Domain::Point => ElementKind::Point,
        Domain::Edge => ElementKind::Edge,
        Domain::Vertex => ElementKind::Vertex,
        Domain::Primitive => ElementKind::Face,
        _ => ElementKind::Object,
    }
}

/// A refused value read from `at`, as the API's error.
fn invalid(at: &str, error: GeometryError) -> Error {
    Error::invalid(format_args!("{at}: {error}"))
}
