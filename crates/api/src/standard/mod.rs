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
//! assert!(standard::brep().get(&data).is_none());
//! ```
//!
//! **Saving.** The project encoding of each type is the SDK's: the codecs
//! here ([`GeometryCodec`], [`ImageCodec`], [`InstancesCodec`], [`BrepCodec`], [`CameraCodec`],
//! [`LightCodec`], [`MaterialsCodec`]) are the ones core registers,
//! and they also save the standard [`Selection`]s a kept result carries.
//! Project files save only frozen and held results, never cooked ones.
//! [`ColorConfiguration`], [`DisplayOutput`],
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
mod geometry;
mod image;
mod instances;
mod light;
mod materials;
mod output;
mod quality;
mod selection;
mod tolerances;

pub use brep::BrepCodec;
pub use camera::{CameraCodec, lens};
pub use color::ColorConfiguration;
pub use geometry::GeometryCodec;
pub use image::ImageCodec;
pub use instances::{Instances, InstancesCodec, MAX_DEPTH};
pub use light::LightCodec;
pub use materials::{ImportedMaterial, MaterialsCodec};
pub use output::{BackgroundSpace, DisplayOutput};

use crate::Error;
use crate::components::Components;
use crate::data::Typed;
use crate::facets::Combine;
use crate::id::DataTypeId;
use crate::viewport::ElementKind;
use kinogaki_geometry::brep::Brep;
use kinogaki_geometry::{Domain, Elements, Geometry, Selection};

/// The id of [`Geometry`] values: polygons, polylines and curves.
pub(crate) const GEOMETRY: &str = "kinogaki.core.geometry";

/// The id of [`Brep`] values.
pub const BREP: &str = "kinogaki.core.brep";

/// The id of a file's materials as a format reads them beside its
/// geometry ([`materials`](fn@materials)).
pub(crate) const MATERIALS: &str = "kinogaki.core.materials";

/// The handle on [`Geometry`] values (`GEOMETRY`).
pub const fn geometry() -> Typed<Geometry> {
    Typed::new(DataTypeId::declared(GEOMETRY))
}

/// The id of [`Image`](kinogaki_geometry::image::Image) values: tiled layers
/// over a data and a display window.
pub(crate) const IMAGE: &str = "kinogaki.core.image";

/// The handle on [`Image`](kinogaki_geometry::image::Image) values (`IMAGE`).
pub const fn image() -> Typed<kinogaki_geometry::image::Image> {
    Typed::new(DataTypeId::declared(IMAGE))
}

/// The id of [`Instances`] values: placements of prototypes of any type.
pub(crate) const INSTANCES: &str = "kinogaki.core.instances";

/// The handle on [`Instances`] values (`INSTANCES`).
pub const fn instances() -> Typed<Instances> {
    Typed::new(DataTypeId::declared(INSTANCES))
}

/// The id of a [`Selection`] passed by value: an input of a kernel's verb
/// saying which elements it works on.
pub(crate) const SELECTION: &str = "kinogaki.core.selection";

/// The handle on a [`Selection`] passed by value (`SELECTION`).
pub const fn selection() -> Typed<Selection> {
    Typed::new(DataTypeId::declared(SELECTION))
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
pub const fn materials() -> Typed<Vec<ImportedMaterial>> {
    Typed::new(DataTypeId::declared(MATERIALS))
}

/// `selection` of a value of data type `ty`, as the [`Components`] the
/// host keeps, shows and counts.
pub fn components(ty: DataTypeId, selection: Selection) -> Components {
    let level = level(selection.domain());
    Components::new(ty, level, selection.len(), selection)
}

/// `hit` combined into the sets `current` as `how` says, one set per element
/// kind: Replace keeps the hit's kind alone; the others combine it into its
/// kind's set and keep the other kinds' sets (Shift-picking an edge beside
/// faces keeps both). What a context's levels do with a pick of a standard
/// value.
pub fn combined(current: &[Components], hit: &Components, how: Combine) -> Vec<Components> {
    if how == Combine::Replace {
        return vec![combined_set(None, hit, how)];
    }
    let mut sets = current.to_vec();
    match sets.iter().position(|c| c.level() == hit.level()) {
        Some(at) => sets[at] = combined_set(Some(&sets[at]), hit, how),
        None => sets.push(combined_set(None, hit, how)),
    }
    sets
}

/// `hit` combined into `current`, a set of its kind (of another value's
/// count: nothing kept of it).
fn combined_set(current: Option<&Components>, hit: &Components, how: Combine) -> Components {
    let Some(new) = hit.selection() else {
        return hit.clone();
    };
    let count = new.members().count();
    let old = (current.and_then(Components::selection))
        .map(|s| s.members().clone())
        .filter(|m| m.count() == count)
        .unwrap_or_else(|| Elements::none(count));
    let members = match how {
        Combine::Replace | Combine::Add => old.union(new.members()),
        Combine::Remove => old.difference(new.members()),
        Combine::Toggle => old.symmetric_difference(new.members()),
    }
    .expect("sets of one count");
    let selection = Selection::new(new.domain(), members, None).expect("no active one");
    components(hit.ty(), selection)
}

/// The viewport's element kind that shows elements of `domain`: Primitive
/// is faces, Detail and Object whole objects.
pub fn level(domain: Domain) -> ElementKind {
    match domain {
        Domain::Point => ElementKind::Point,
        Domain::Edge => ElementKind::Edge,
        Domain::Vertex => ElementKind::Vertex,
        Domain::Primitive => ElementKind::Face,
        Domain::Instance => ElementKind::Instance,
        _ => ElementKind::Object,
    }
}

/// A refused value read from `at`, as the API's error.
fn invalid(at: &str, error: Error) -> Error {
    Error::invalid(format_args!("{at}: {error}"))
}
