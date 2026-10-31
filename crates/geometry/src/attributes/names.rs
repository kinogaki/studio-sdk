//! The standard attribute and group names, and the rule every name
//! follows ([`check_name`]).
//!
//! A standard name always has its standard type: [`Attributes::with`] and
//! [`Attributes::with_group`] refuse it written otherwise, so every reader
//! can rely on it. A kernel's own attributes use names in its namespace
//! (`acme:weight`) or conventions its documentation states.
//!
//! [`Attributes::with`]: crate::Attributes::with
//! [`Attributes::with_group`]: crate::Attributes::with_group
use super::{AttributeKind, Class, TypeInfo};
use crate::error::{ErrorKind, GeometryError};
use crate::limits::MAX_NAME;

/// Position (Vector, [`TypeInfo::Point`]). A value's points are its own
/// positions column, never a Point attribute named `P`.
pub const P: &str = "P";
/// Authored normal (Vector, [`TypeInfo::Normal`]). Shading normals follow
/// one rule everywhere: authored `N`, else by the [`SHARP`] edges and angle.
pub const N: &str = "N";
/// Velocity, in scene units per second, for motion blur (Vector,
/// [`TypeInfo::Vector`]).
pub const V: &str = "v";
/// Texture coordinates (Vector, [`TypeInfo::TextureCoord`]).
pub const UV: &str = "uv";
/// Color (Vector, [`TypeInfo::Color`]), scene-linear in the project's
/// working space.
pub const CD: &str = "Cd";
/// Opacity, 0 to 1 (Float).
pub const ALPHA: &str = "Alpha";
/// A curve's or point's width, in scene units (Float).
pub const WIDTH: &str = "width";
/// The Material a primitive renders with, by its path (Text).
pub const MATERIAL: &str = "material";
/// A name (Text): an element's, or an instance's occurrence path.
pub const NAME: &str = "name";
/// The part an element belongs to (Text).
pub const PART: &str = "part";
/// Which prototype an instance places (Integer).
pub const INSTANCE: &str = "instance";
/// The hard edges (an [`Class::Edge`] group): shading always breaks
/// across them, whatever the angle between their faces. An edge in both
/// this and [`SMOOTH`] is sharp. An edge in neither follows the 60° cusp
/// rule ([`ShadingNormals`](crate::ShadingNormals)).
pub const SHARP: &str = "sharp";
/// The soft edges (an [`Class::Edge`] group): shading always joins the
/// two faces of such an edge, whatever the angle between them (Soft
/// Edges). Only an edge with exactly two faces joins; open and
/// non-manifold edges stay split. [`SHARP`] wins over it.
pub const SMOOTH: &str = "smooth";
/// The hidden primitives (a [`Class::Primitive`] group): Hide.
pub const HIDDEN: &str = ".hidden";

/// What a standard name must be.
pub(crate) enum Standard {
    /// An attribute of this kind and type info, on any class.
    Attribute(AttributeKind, TypeInfo),
    /// A group of this class only.
    Group(Class),
}

/// The standard meaning of `name`, if it has one.
pub(crate) fn standard(name: &str) -> Option<Standard> {
    use AttributeKind::{Float, Integer, Text, Vector};
    Some(match name {
        P => Standard::Attribute(Vector, TypeInfo::Point),
        N => Standard::Attribute(Vector, TypeInfo::Normal),
        V => Standard::Attribute(Vector, TypeInfo::Vector),
        UV => Standard::Attribute(Vector, TypeInfo::TextureCoord),
        CD => Standard::Attribute(Vector, TypeInfo::Color),
        ALPHA | WIDTH => Standard::Attribute(Float, TypeInfo::None),
        MATERIAL | NAME | PART => Standard::Attribute(Text, TypeInfo::None),
        INSTANCE => Standard::Attribute(Integer, TypeInfo::None),
        SHARP | SMOOTH => Standard::Group(Class::Edge),
        HIDDEN => Standard::Group(Class::Primitive),
        _ => return None,
    })
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
