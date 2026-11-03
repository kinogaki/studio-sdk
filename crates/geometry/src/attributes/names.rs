//! The standard attribute and group names, and the rule every name
//! follows ([`check_name`]).
//!
//! A standard name fixes its whole type and the domains it may live on
//! (the table below): [`Attributes::with`] and [`Attributes::with_group`]
//! refuse it written otherwise, so every reader can rely on it. A
//! kernel's own attributes use names in its namespace (`acme:weight`) or
//! conventions its documentation states.
//!
//! | Name | Domains | Type |
//! |---|---|---|
//! | [`P`] | none | the value's own positions column, `[f64; 3]` (never an attribute) |
//! | [`N`] | Point, Vertex, Primitive | f32 × 3 normal |
//! | [`UV`] | Point, Vertex | f32 × 2 texture coordinate |
//! | [`CD`] | any | f32 × 3 color (scene-linear, working space) |
//! | [`ALPHA`] | any | f32 |
//! | [`V`] | Point, Instance | f32 × 3 vector, units per second |
//! | [`ID`] | Point, Primitive, Instance | i64 |
//! | [`PSCALE`] | Point, Instance | f32 |
//! | [`SCALE`] | Point, Instance | f32 × 3 |
//! | [`ORIENT`] | Point, Instance | f32 × 4 rotation |
//! | [`UP`] | Point | f32 × 3 vector |
//! | [`REST`] | Point | f64 × 3 point (positions' precision) |
//! | [`WIDTH`] | Point, Primitive | f32 |
//! | [`MATERIAL`], [`NAME`], [`PART`] | any | text |
//! | groups [`SHARP`], [`SMOOTH`] / [`HIDDEN`] | Edge / Primitive | group |
//!
//! [`Attributes::with`]: crate::Attributes::with
//! [`Attributes::with_group`]: crate::Attributes::with_group
use super::{AttributeType, Domain, Meaning, Storage};
use crate::error::{ErrorKind, GeometryError};
use crate::limits::MAX_NAME;

/// Position. A value's points are its own positions column, never an
/// attribute named `P`.
pub const P: &str = "P";
/// Authored normal. Shading normals follow one rule everywhere: authored
/// `N`, else by the [`SHARP`] edges and angle.
pub const N: &str = "N";
/// Velocity, in scene units per second, for motion blur.
pub const V: &str = "v";
/// Texture coordinates.
pub const UV: &str = "uv";
/// Color, scene-linear in the project's working space.
pub const CD: &str = "Cd";
/// Opacity, 0 to 1.
pub const ALPHA: &str = "Alpha";
/// A stable id, unique within its domain.
pub const ID: &str = "id";
/// A uniform scale (copies, instances, point sizes).
pub const PSCALE: &str = "pscale";
/// A scale per axis (copies, instances).
pub const SCALE: &str = "scale";
/// An orientation (copies, instances), a unit quaternion x y z w.
pub const ORIENT: &str = "orient";
/// The up direction a copy aims its y axis along.
pub const UP: &str = "up";
/// Rest positions, as precise as positions.
pub const REST: &str = "rest";
/// A curve's or point's width, in scene units.
pub const WIDTH: &str = "width";
/// The Material a primitive renders with, by its path.
pub const MATERIAL: &str = "material";
/// A name: an element's, or an instance's occurrence path.
pub const NAME: &str = "name";
/// The part an element belongs to.
pub const PART: &str = "part";
/// The hard edges (an [`Domain::Edge`] group): shading always breaks
/// across them, whatever the angle between their faces. An edge in both
/// this and [`SMOOTH`] is sharp. An edge in neither follows the 60° cusp
/// rule ([`ShadingNormals`](crate::ShadingNormals)).
pub const SHARP: &str = "sharp";
/// The soft edges (an [`Domain::Edge`] group): shading always joins the
/// two faces of such an edge, whatever the angle between them (Soft
/// Edges). Only an edge with exactly two faces joins; open and
/// non-manifold edges stay split. [`SHARP`] wins over it.
pub const SMOOTH: &str = "smooth";
/// The hidden primitives (a [`Domain::Primitive`] group): Hide.
pub const HIDDEN: &str = ".hidden";

/// What a standard name must be.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Standard {
    /// An attribute of this type, on these domains only (empty: it is
    /// never an attribute, as [`P`]; `ANY`: every one).
    Attribute(AttributeType, &'static [Domain]),
    /// A group on this domain only.
    Group(Domain),
}

/// Every domain: a standard attribute that may live anywhere.
pub(crate) const ANY: &[Domain] = &Domain::ALL;

/// The standard meaning of `name`, if it has one: the one table every
/// reader and writer follows.
pub(crate) fn standard(name: &str) -> Option<Standard> {
    use Domain::{Edge, Instance, Point, Primitive, Vertex};
    let f32s = |width, meaning| AttributeType::new(Storage::F32, width, meaning);
    let attribute = |ty, domains| Some(Standard::Attribute(ty, domains));
    match name {
        P => attribute(AttributeType::new(Storage::F64, 3, Meaning::Point), &[]),
        N => attribute(f32s(3, Meaning::Normal), &[Point, Vertex, Primitive]),
        UV => attribute(f32s(2, Meaning::TextureCoord), &[Point, Vertex]),
        CD => attribute(f32s(3, Meaning::Color), ANY),
        ALPHA => attribute(f32s(1, Meaning::None), ANY),
        V => attribute(f32s(3, Meaning::Vector), &[Point, Instance]),
        ID => attribute(
            AttributeType::new(Storage::I64, 1, Meaning::None),
            &[Point, Primitive, Instance],
        ),
        PSCALE => attribute(f32s(1, Meaning::None), &[Point, Instance]),
        SCALE => attribute(f32s(3, Meaning::None), &[Point, Instance]),
        ORIENT => attribute(f32s(4, Meaning::Rotation), &[Point, Instance]),
        UP => attribute(f32s(3, Meaning::Vector), &[Point]),
        REST => attribute(
            AttributeType::new(Storage::F64, 3, Meaning::Point),
            &[Point],
        ),
        WIDTH => attribute(f32s(1, Meaning::None), &[Point, Primitive]),
        MATERIAL | NAME | PART => attribute(AttributeType::TEXT, ANY),
        SHARP | SMOOTH => Some(Standard::Group(Edge)),
        HIDDEN => Some(Standard::Group(Primitive)),
        _ => None,
    }
}

/// Whether `name` may name an attribute or a group: a letter, `_` or `.`
/// first, then letters, digits and `_ . : - /`, at most [`MAX_NAME`]
/// bytes.
///
/// ```
/// use kinogaki_geometry::check_name;
/// assert!(check_name("Cd").is_ok() && check_name(".hidden").is_ok());
/// assert!(check_name("acme:weight").is_ok() && check_name("set:top/left").is_ok());
/// assert!(check_name("").is_err() && check_name("2x").is_err() && check_name("a b").is_err());
/// ```
///
/// # Errors
/// Saying what is wrong with it.
pub fn check_name(name: &str) -> Result<(), GeometryError> {
    let refuse = |why: String| Err(GeometryError::whole(ErrorKind::Attribute, why));
    let Some(first) = name.chars().next() else {
        return refuse("an attribute needs a name".into());
    };
    if !(first.is_alphabetic() || matches!(first, '_' | '.')) {
        return refuse(format!("a name cannot start with `{first}`"));
    }
    if let Some(c) = name
        .chars()
        .find(|&c| !(c.is_alphanumeric() || matches!(c, '_' | '.' | ':' | '-' | '/')))
    {
        return refuse(format!("`{c}` cannot appear in a name"));
    }
    if name.len() > MAX_NAME {
        return refuse(format!("a name has at most {MAX_NAME} bytes"));
    }
    Ok(())
}
