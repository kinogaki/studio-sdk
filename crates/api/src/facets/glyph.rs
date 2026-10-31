//! [`Glyphs`]: an object kind drawn as lines where its objects are (a
//! light's shape, a camera's frustum), which the viewport places, picks by
//! distance, boxes, frames, ghosts around a context and tints as a
//! template, as it does its own Cameras'.
use crate::math::Vec3;
use crate::object::ObjectBody;

/// How large a glyph without a size of its own draws (an arrow, a dome's
/// globe): one step of the ground grid, in project units, and the
/// project's metres a unit (a millimetre aperture is a length).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GlyphScale {
    /// The grid step, in project units.
    pub size: f64,
    /// Metres a project unit.
    pub meters: f64,
}

/// An object kind's line glyphs ([`ObjectKindSpec::glyphs`](crate::ObjectKindSpec::glyphs)).
pub trait Glyphs: Send + Sync {
    /// `body`'s segments in its object's own space, sized by `scale`.
    fn lines(&self, body: &dyn ObjectBody, scale: GlyphScale) -> Vec<[Vec3; 2]>;
}
