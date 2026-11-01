//! The standard data types: the geometry values every package shares
//! ([`geometry`](crate::geometry)), as data types.
//!
//! Studio's core registers them, so they exist with no package installed;
//! a kernel never registers a type of its own for the same data. A node
//! reads and writes them through the handles here:
//!
//! ```
//! use kinogaki_studio_api::geometry::Polygons;
//! use kinogaki_studio_api::standard;
//!
//! let data = standard::polygons().wrap(Polygons::empty());
//! assert_eq!(data.ty().as_str(), standard::POLYGONS);
//! assert!(standard::polygons().get(&data).is_some());
//! assert!(standard::curves().get(&data).is_none());
//! ```
//!
//! **Saving.** The project encoding of each type is the SDK's: the codecs
//! here ([`PolygonsCodec`], [`CurvesCodec`], [`BrepCodec`], [`CameraCodec`]) are the ones core registers,
//! and they also save the standard [`Selection`]s a kept result carries.
//! Project files save only frozen and held results, never cooked ones.
//! [`Tolerances`](crate::geometry::Tolerances) is the project value core
//! keeps ([`Stored`](crate::Stored), `tolerances` in the project's
//! settings).
mod attributes;
mod brep;
mod camera;
mod curves;
mod polygons;
mod selection;
mod tolerances;

pub use brep::BrepCodec;
pub use camera::{CameraCodec, lens};
pub use curves::CurvesCodec;
pub use polygons::PolygonsCodec;

use crate::components::Components;
use crate::data::Typed;
use crate::error::Error;
use crate::id::DataTypeId;
use crate::viewport::ElementKind;
use kinogaki_geometry::brep::Brep;
use kinogaki_geometry::{Class, Curves, GeometryError, Polygons, Selection};

/// The id of [`Polygons`] values.
pub const POLYGONS: &str = "kinogaki.core.polygons";
/// The id of [`Curves`] values.
pub const CURVES: &str = "kinogaki.core.curves";

/// The id of [`Brep`] values.
pub const BREP: &str = "kinogaki.core.brep";

/// The handle on [`Polygons`] values ([`POLYGONS`]).
pub fn polygons() -> Typed<Polygons> {
    Typed::new(DataTypeId::declared(POLYGONS))
}

/// The handle on [`Curves`] values ([`CURVES`]).
pub fn curves() -> Typed<Curves> {
    Typed::new(DataTypeId::declared(CURVES))
}

/// The id of camera values: an unplaced lens ([`lens`], [`CameraCodec`]).
pub const CAMERA: &str = "kinogaki.core.camera";

/// The handle on camera values ([`CAMERA`]): render-api's
/// [`Camera`](kinogaki_render_api::Camera), unplaced, in UsdGeomCamera's
/// units; make one with [`lens`].
pub fn camera() -> Typed<kinogaki_render_api::Camera> {
    Typed::new(DataTypeId::declared(CAMERA))
}

/// The handle on [`Brep`] values ([`BREP`]).
pub fn brep() -> Typed<Brep> {
    Typed::new(DataTypeId::declared(BREP))
}

/// How many elements of `class` a B-rep value holds: one Detail element,
/// and each other class's elements summed over its bodies in body order
/// (a body's faces follow the faces of the bodies before it), so a
/// selection of a value's faces, edges or vertices spans its bodies.
pub fn brep_count(value: &Brep, class: Class) -> Option<usize> {
    brep::count(value, class)
}

/// `selection` of a value of data type `ty` with `count` elements of its
/// class, as the [`Components`] the host keeps, shows and counts.
pub fn components(ty: DataTypeId, selection: Selection, count: usize) -> Components {
    let level = level(selection.class());
    Components::new(ty, level, selection.len(count), selection)
}

/// The viewport's element kind that shows elements of `class`: Primitive
/// and Piece are faces, Detail and Object whole objects.
pub fn level(class: Class) -> ElementKind {
    match class {
        Class::Point => ElementKind::Point,
        Class::Edge => ElementKind::Edge,
        Class::Vertex => ElementKind::Vertex,
        Class::Primitive | Class::Piece => ElementKind::Face,
        _ => ElementKind::Object,
    }
}

/// A refused value read from `at`, as the API's error.
fn invalid(at: &str, error: GeometryError) -> Error {
    Error::invalid(format_args!("{at}: {error}"))
}
