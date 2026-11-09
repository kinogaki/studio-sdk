//! [`Subdivider`]: a kernel's refinement of a polygons value into the
//! subdivision surface its [`Subdivision`] setting says.
//!
//! A value that subdivides is still polygons (its cage): the viewport shows
//! it so, and files save it so. What an engine renders of it is a
//! registered subdivider's output, made by the host when a render asks
//! ([`RenderCx::refine`](crate::facets::RenderCx::refine)), never part of
//! the value and never saved. With no subdivider installed a value renders
//! as its cage.
use crate::Error;
use crate::cancel::CancelToken;
use crate::id::SubdividerId;
use kinogaki_geometry::{Geometry, Subdivision};
use std::sync::Arc;

/// Refines polygons into subdivision surfaces, for rendering.
///
/// **The output** ([`Refined::polygons`]), in the cage's own space: the
/// cage's polygons refined `subdivision.level` times by its scheme, shaped
/// by every edge's and point's [`CREASE`](kinogaki_geometry::names::CREASE)
/// and the border as `subdivision.boundary` says; the faces of the
/// [`SUBDIVISION_HOLE`](kinogaki_geometry::names::SUBDIVISION_HOLE) group
/// (and, with no boundary, those touching the border) left out; attributes
/// following as an edit carries them, with no `N` (a subdivision surface
/// shades by its shape) and the edges a crease keeps sharp through every
/// level in the [`SHARP`](kinogaki_geometry::names::SHARP) group. Curves and
/// polylines pass through unchanged. What it can't honour of the setting (a
/// face-varying rule other than linear, smooth triangles) its documentation
/// states.
///
/// **State.** `refine` is a function of its arguments. What it gives keeps
/// whatever makes a drag cheap ([`Refined::moved`]): the next step of a
/// cage whose points alone moved is that refinement moved, not a new one.
pub trait Subdivider: Send + Sync {
    /// Its id (`kinogaki.mesh_kernel.subdivider`).
    fn id(&self) -> SubdividerId;

    /// What a UI calls it ("Mesh").
    fn label(&self) -> &str;

    /// `cage` refined as `subdivision` says. Long refinements poll `cancel`
    /// and stop with [`Error::cancelled()`].
    ///
    /// # Errors
    /// Cancelled, or a refinement past the product's limits.
    fn refine(
        &self,
        cage: &Geometry,
        subdivision: &Subdivision,
        cancel: &CancelToken,
    ) -> Result<Arc<dyn Refined>, Error>;
}

/// A [`Subdivider`]'s refinement of one cage.
pub trait Refined: Send + Sync {
    /// The refined polygons.
    fn polygons(&self) -> &Geometry;

    /// This refinement of `cage` when `cage` is the cage it refined with
    /// its points moved (the same table and attributes, other positions):
    /// the refined polygons' table and attributes kept, their points
    /// moved. `None` for any other cage, which [`Subdivider::refine`]
    /// refines afresh.
    ///
    /// # Errors
    /// Cancelled.
    fn moved(
        &self,
        cage: &Geometry,
        cancel: &CancelToken,
    ) -> Result<Option<Arc<dyn Refined>>, Error>;
}
