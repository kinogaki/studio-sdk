//! [`Moving`]: the points a drag moves, which core previews over the
//! shown value while the drag lasts.
use crate::deform::Deform;
use std::sync::Arc;

/// A drag of some of the shown value's points, which core previews over
/// it: no cook, no cached value, and only the moved points' primitives
/// sent to the GPU each step.
///
/// The context resolves `points` (and the deform's weights) once, when the
/// drag begins, and hands the same `Arc`s every frame until it ends: core
/// gathers what the preview needs once per `points` allocation, so a new
/// allocation starts a new preview. The deform's step is the current one,
/// placed with the function the context's commit cooks with
/// ([`Deform::place`]), so the preview places each point where the edit
/// will. Once the drag ends the context hands no `Moving`; core keeps the
/// last step shown while the edit it committed cooks, so the committed
/// value replaces it.
#[derive(Clone, Debug, PartialEq)]
pub struct Moving {
    /// The moved points, by index into the shown value's points, sorted
    /// and without repeats.
    pub points: Arc<[u32]>,
    /// The move; its weights, when it has them, in `points`' order.
    pub deform: Deform,
}
