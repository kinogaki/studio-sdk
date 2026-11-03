//! The engine trait and what passes through it: views, frames, one-shot
//! results and the wake callback.
//!
//! An engine owns its render thread. [`RenderEngine::update`] and
//! [`RenderEngine::view`] queue work and return at once; the engine applies
//! it at a pass boundary and publishes each whole pass as a [`Frame`], then
//! calls the host's [`Wake`] once. The host never waits on the engine.

use std::sync::{Arc, Mutex};

use crate::aov::AovDesc;
use crate::description::EngineDescription;
use crate::id::PrimId;
use crate::prim::Camera;
use crate::update::SceneUpdate;

/// Called once per published result (a frame, a final read). An editor's
/// is a repaint request.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// A view's id, chosen by the host.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewId(pub u32);

/// Where a view looks from.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewCamera {
    /// Through a Camera prim.
    Prim(PrimId),
    /// Through a camera the host places (a viewport's own).
    Free(Camera),
}

/// How a view is used.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ViewMode {
    /// Progressive, shown as it refines; may run at a reduced size.
    Interactive,
    /// Interactive while its camera moves: one pass in flight, so a new
    /// camera waits behind at most one short pass, and a finished pass of an
    /// earlier camera is still shown when nothing newer has been.
    Navigating,
    /// A final frame at the settings' sample count.
    Final,
}

/// A view's camera, film size and mode.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewDesc {
    /// Where it looks from.
    pub camera: ViewCamera,
    /// Film width, in pixels.
    pub width: u32,
    /// Film height, in pixels.
    pub height: u32,
    /// How it is used.
    pub mode: ViewMode,
}

/// What a pass cost and found.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct RenderStats {
    /// Samples a pixel so far.
    pub samples: u32,
    /// The samples a pixel takes in all: the view's goal (0: not known).
    pub target: u32,
    /// The last pass's GPU time, in milliseconds.
    pub pass_ms: f32,
    /// Problems to report once (an unsupported node, a missing texture).
    pub problems: Vec<Arc<str>>,
}

/// The last whole pass of a view: its display image and stats.
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// The view.
    pub view: ViewId,
    /// Which edit of the view this frame shows (each [`RenderEngine::view`]
    /// or scene update that restarts the film starts the next).
    pub generation: u64,
    /// The view desc its pass rendered, as the host gave it: a host tells a
    /// pass of the camera it sent last from one of an earlier camera that
    /// finished before the engine took the new one.
    pub desc: ViewDesc,
    /// Display width, in pixels.
    pub width: u32,
    /// Display height, in pixels.
    pub height: u32,
    /// RGBA half floats (their bits), linear ACEScg, rows from the top;
    /// empty when the image reaches the display another way (a shared GPU
    /// texture).
    pub pixels: Arc<[u16]>,
    /// Which of the engine's display images holds it, for hosts that show
    /// those images where they are (the engine's own identifier; 0 when it
    /// shares none).
    pub image: u64,
    /// What it cost.
    pub stats: RenderStats,
}

/// A final frame's outputs.
#[derive(Clone, Debug, PartialEq)]
pub struct Outputs {
    /// Width, in pixels.
    pub width: u32,
    /// Height, in pixels.
    pub height: u32,
    /// Each requested output: its values, interleaved per pixel
    /// ([`crate::Aov::channels`] a pixel), f32 or u32 bits as its
    /// format says (f16 widened).
    pub planes: Vec<(AovDesc, Arc<[f32]>)>,
    /// Cryptomatte names: the paths whose hashes appear in its planes.
    pub names: Vec<Arc<str>>,
}

impl Default for Outputs {
    /// No outputs, one pixel a film pixel.
    fn default() -> Outputs {
        Outputs {
            width: 0,
            height: 0,
            planes: Vec::new(),
            names: Vec::new(),
        }
    }
}

/// Why a [`Pending`] result never came.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RenderError {
    /// The engine couldn't produce it, saying why (a readback failed, the
    /// device was lost).
    Failed(Arc<str>),
    /// The engine let the request go without answering: it dropped the
    /// [`Deliver`] (a view removed under it, the engine shut down).
    Abandoned,
}

impl std::fmt::Display for RenderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderError::Failed(why) => f.write_str(why),
            RenderError::Abandoned => f.write_str("the engine dropped the request"),
        }
    }
}

impl std::error::Error for RenderError {}

/// What a pending result holds: nothing yet, its outcome, or its outcome
/// taken.
enum Slot<T> {
    Waiting,
    Resolved(Result<T, RenderError>),
    Taken,
}

/// A result that arrives later, once: the value, or why it never will.
/// The producer holds the matching [`Deliver`]; delivering, failing or
/// dropping it resolves this one and wakes the host once, so a host never
/// waits on a result no one will give.
pub struct Pending<T> {
    slot: Arc<Mutex<Slot<T>>>,
}

/// The producing side of a [`Pending`]. Dropped without
/// [`deliver`](Self::deliver) or [`fail`](Self::fail), it resolves the
/// result as [`RenderError::Abandoned`].
pub struct Deliver<T> {
    slot: Arc<Mutex<Slot<T>>>,
    wake: Wake,
}

/// A pending result and the means to deliver it (waking `wake` once).
pub fn pending<T>(wake: Wake) -> (Deliver<T>, Pending<T>) {
    let slot = Arc::new(Mutex::new(Slot::Waiting));
    (
        Deliver {
            slot: slot.clone(),
            wake,
        },
        Pending { slot },
    )
}

impl<T> Deliver<T> {
    /// Store the result and wake the host. Consumes the producer, so a
    /// result is resolved at most once.
    pub fn deliver(self, value: T) {
        self.resolve(Ok(value));
    }

    /// Resolve the result as failed, saying `why`, and wake the host.
    pub fn fail(self, why: impl Into<Arc<str>>) {
        self.resolve(Err(RenderError::Failed(why.into())));
    }

    /// Resolves the result as `outcome` unless it is resolved already, and
    /// wakes the host when it was.
    fn resolve(&self, outcome: Result<T, RenderError>) {
        let resolved = {
            let mut slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
            let waiting = matches!(*slot, Slot::Waiting);
            if waiting {
                *slot = Slot::Resolved(outcome);
            }
            waiting
        };
        if resolved {
            (self.wake)();
        }
    }
}

impl<T> Drop for Deliver<T> {
    fn drop(&mut self) {
        self.resolve(Err(RenderError::Abandoned));
    }
}

impl<T> Pending<T> {
    /// The outcome, the first time it is asked for after it was resolved.
    pub fn take(&self) -> Option<Result<T, RenderError>> {
        let mut slot = self.slot.lock().unwrap_or_else(|e| e.into_inner());
        match std::mem::replace(&mut *slot, Slot::Taken) {
            Slot::Resolved(outcome) => Some(outcome),
            other => {
                *slot = other;
                None
            }
        }
    }
}

/// A render engine.
pub trait RenderEngine: Send {
    /// What it is: its settings, vocabulary and outputs.
    fn description(&self) -> &Arc<EngineDescription>;
    /// Queue a batch of scene edits; never blocks.
    fn update(&mut self, update: SceneUpdate);
    /// Add or change a view (its camera, size or mode).
    fn view(&mut self, view: ViewId, desc: ViewDesc);
    /// Remove a view.
    fn drop_view(&mut self, view: ViewId);
    /// The view's last whole pass.
    fn latest(&self, view: ViewId) -> Option<Arc<Frame>>;
    /// Pause or resume a view's refinement.
    fn pause(&mut self, view: ViewId, paused: bool);
    /// Read a view's outputs once its samples are done. The result
    /// resolves once: the outputs, or why there are none (a failed read,
    /// or the request dropped with the view or the engine).
    fn finish(&mut self, view: ViewId, outputs: &[AovDesc]) -> Pending<Outputs>;
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    fn counted() -> (Wake, Arc<AtomicUsize>) {
        let wakes = Arc::new(AtomicUsize::new(0));
        let count = wakes.clone();
        let wake: Wake = Arc::new(move || {
            count.fetch_add(1, Ordering::SeqCst);
        });
        (wake, wakes)
    }

    #[test]
    fn a_dropped_deliverer_resolves_its_result_and_wakes_once() {
        let (wake, wakes) = counted();
        let (deliver, result) = pending::<u32>(wake);
        std::thread::spawn(move || drop(deliver))
            .join()
            .expect("the producer runs");
        assert_eq!(wakes.load(Ordering::SeqCst), 1);
        assert_eq!(result.take(), Some(Err(RenderError::Abandoned)));
        assert_eq!(result.take(), None);
    }

    #[test]
    fn a_failure_resolves_once_saying_why() {
        let (wake, wakes) = counted();
        let (deliver, result) = pending::<u32>(wake);
        deliver.fail("the device was lost");
        assert_eq!(
            wakes.load(Ordering::SeqCst),
            1,
            "failing wakes; the drop after it doesn't"
        );
        let failed = RenderError::Failed(Arc::from("the device was lost"));
        assert_eq!(result.take(), Some(Err(failed)));
    }
}
