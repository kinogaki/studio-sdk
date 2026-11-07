//! [`Held`]: what the host made of values other values hold.
use super::{DisplayRequest, Drawn, RenderCx, Rendered};
use crate::Error;
use crate::data::Data;

/// The host's drawings and render items of values that other values hold
/// (instances' prototypes), by the held value's identity: a type whose
/// values hold others asks it for each one it draws or renders, so a value
/// many objects hold is prepared, uploaded and rendered once while any of
/// them shows it. The host keeps what it was given while a holder holds
/// it, and lets it go after.
///
/// The host gives one in [`DisplayRequest::held`] and [`RenderCx::held`];
/// without one, a type makes its own.
pub trait Held: Send + Sync {
    /// `data`'s drawing for `request`: the one made for the same value
    /// and request while a holder holds it, else what `make` makes (the
    /// type preparing it, from what its own value held before), kept.
    ///
    /// # Errors
    /// `make`'s.
    fn drawn(
        &self,
        data: &Data,
        request: &DisplayRequest,
        make: &mut dyn FnMut() -> Result<Drawn, Error>,
    ) -> Result<Drawn, Error>;

    /// `data`'s render items at `cx`'s detail and tolerances, as
    /// [`drawn`](Self::drawn) gives drawings.
    ///
    /// # Errors
    /// `make`'s.
    fn rendered(
        &self,
        data: &Data,
        cx: &RenderCx<'_>,
        make: &mut dyn FnMut() -> Result<Rendered, Error>,
    ) -> Result<Rendered, Error>;
}
