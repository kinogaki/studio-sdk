//! [`Glyphs`]: an object kind drawn as lines where its objects are (a
//! light's shape, a camera's frustum), which the viewport places, picks by
//! distance, boxes, frames, ghosts around a context and tints as a
//! template, as it does its own Cameras'.
use crate::object::ObjectBody;
use kinogaki_geometry::{Units, Vec3};

/// How large a glyph without a size of its own draws (an arrow, a dome's
/// globe): one step of the ground grid, in project units, and the
/// project's units (a millimetre aperture is a length).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphScale {
    /// The grid step, in project units.
    pub size: f64,
    /// The project's units.
    pub units: Units,
}

/// An object kind's line glyphs ([`ObjectKindSpec::glyphs`](crate::ObjectKindSpec::glyphs)).
pub trait Glyphs: Send + Sync {
    /// `body`'s segments in its object's own space, sized by `scale`.
    fn lines(&self, body: &dyn ObjectBody, scale: GlyphScale) -> Vec<[Vec3; 2]>;
}
