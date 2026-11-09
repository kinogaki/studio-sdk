//! The engine trait and what passes through it: views, frames, one-shot
//! results and the wake callback.
//!
//! An engine owns its render thread. [`RenderEngine::update`] and
//! [`RenderEngine::view`] queue work and return at once; the engine applies
//! it at a pass boundary and publishes each whole pass as a [`ViewPass`] (its pixels an Image), then
//! calls the host's [`Wake`] once. The host never waits on the engine.

use std::sync::{Arc, Mutex};

use crate::aov::AovDesc;
use crate::description::EngineDescription;
use crate::id::Id;
use crate::material::preview::PreviewRequest;
use crate::prim::Camera;
use crate::update::SceneUpdate;
use kinogaki_geometry::image::{Encoding, Format, HeldPixels, Image, Rect, Sample};
use kinogaki_geometry::{Diagnostic, Error, ErrorKind};
use kinogaki_value::Parameters;

/// Called once per published result (a frame, a final read). An editor's
/// is a repaint request.
pub type Wake = Arc<dyn Fn() + Send + Sync>;

/// A view's id, chosen by the host: any but `u32::MAX`, which is
/// [`thread::PREVIEW`](crate::thread::PREVIEW), the view an engine's thread
/// renders previews through.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ViewId(pub u32);

/// Where a view looks from.
#[derive(Clone, Debug, PartialEq)]
pub enum ViewCamera {
    /// Through a Camera prim.
    Prim(Id<Camera>),
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

/// A view's camera, film size, mode and settings.
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
    /// The engine's settings for this film, by its descriptors' names; one
    /// missing takes its descriptor's default.
    pub values: Arc<Parameters>,
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
}

/// The last whole pass of a view: its image and stats.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewPass {
    /// The view.
    pub view: ViewId,
    /// Which edit of the view this frame shows (each [`RenderEngine::view`]
    /// or scene update that restarts the film starts the next).
    pub generation: u64,
    /// The view desc its pass rendered, as the host gave it: a host tells a
    /// pass of the camera it sent last from one of an earlier camera that
    /// finished before the engine took the new one.
    pub desc: ViewDesc,
    /// The pass at the display's size: its main layer RGBA, linear ACEScg,
    /// premultiplied; tiles of half floats ([`ViewPass::pixels`]: an engine
    /// rendering on the CPU, or asked for its pixels), or held as one of the
    /// engine's display images ([`EngineImage`]), which a host shows where it
    /// is ([`Display`](crate::display::Display)).
    pub image: Image,
    /// What it cost.
    pub stats: RenderStats,
}

impl ViewPass {
    /// A frame's image of `size` pixels holding `halves`, RGBA half floats'
    /// bits ([`f32_to_half`](crate::f32_to_half)), rows from the top.
    ///
    /// # Errors
    /// No pixels, or `halves` not four a pixel.
    pub fn pixels(size: [u32; 2], halves: &[u16]) -> Result<Image, Error> {
        let window = Rect::new([0, 0], size.map(|v| v as i32))?;
        let bytes: Vec<u8> = halves.iter().flat_map(|h| h.to_ne_bytes()).collect();
        let format = Format::rgba(Sample::F16, Encoding::color(crate::RENDER_SPACE, true));
        Image::builder(window).plane(format, &bytes).build()
    }

    /// Its width and height, in pixels.
    pub fn size(&self) -> [u32; 2] {
        self.image.display().size()
    }

    /// The engine's display image holding it, when one does.
    pub fn engine_image(&self) -> Option<u64> {
        let held = self.image.layers().first()?.held()?;
        let held: &dyn std::any::Any = &**held;
        held.downcast_ref::<EngineImage>().map(|i| i.0)
    }
}

/// One of an engine's display images, by the engine's own identifier: a
/// frame's pixels where the engine keeps them on the host's device
/// ([`HeldPixels`]).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct EngineImage(pub u64);

impl EngineImage {
    /// A frame's image of `size` pixels held as this one.
    ///
    /// # Errors
    /// No pixels.
    pub fn image(self, size: [u32; 2]) -> Result<Image, Error> {
        let window = Rect::new([0, 0], size.map(|v| v as i32))?;
        let format = Format::rgba(Sample::F16, Encoding::color(crate::RENDER_SPACE, true));
        Image::builder(window).held(format, Arc::new(self)).build()
    }
}

impl HeldPixels for EngineImage {
    fn key(&self) -> u64 {
        self.0
    }
}

/// What a pending result holds: nothing yet, its outcome, or its outcome
/// taken.
enum Slot<T> {
    Waiting,
    Resolved(Result<T, Error>),
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
/// result as a [`Cancelled`](ErrorKind::Cancelled) error: the engine let
/// the request go without answering (a view removed under it, the engine
/// shut down).
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
    /// Whether anyone still holds the result: dropping the [`Pending`] is how
    /// a host cancels. An engine asks between passes and lets go of a result
    /// no one wants, stopping its work.
    pub fn wanted(&self) -> bool {
        Arc::strong_count(&self.slot) > 1
    }

    /// Store the result and wake the host. Consumes the producer, so a
    /// result is resolved at most once.
    pub fn deliver(self, value: T) {
        self.resolve(Ok(value));
    }

    /// Resolve the result as failed with `error` (a readback that failed is
    /// invalid, a lost device missing), and wake the host.
    pub fn fail(self, error: Error) {
        self.resolve(Err(error));
    }

    /// Resolves the result as `outcome` unless it is resolved already, and
    /// wakes the host when it was.
    fn resolve(&self, outcome: Result<T, Error>) {
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

/// What a result whose producer was dropped unanswered says.
const ABANDONED: &str = "the engine let the request go";

impl<T> Drop for Deliver<T> {
    fn drop(&mut self) {
        self.resolve(Err(Error::new(ErrorKind::Cancelled, ABANDONED)));
    }
}

impl<T> Pending<T> {
    /// The outcome, the first time it is asked for after it was resolved.
    pub fn take(&self) -> Option<Result<T, Error>> {
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
    /// Add or change a view (its camera, size or mode). A view added starts
    /// unpaused; a view changed stays paused or not, as it was.
    fn view(&mut self, view: ViewId, desc: ViewDesc);
    /// Remove a view.
    fn drop_view(&mut self, view: ViewId);
    /// The view's last whole pass.
    fn latest(&self, view: ViewId) -> Option<Arc<ViewPass>>;
    /// Pause or resume a view's refinement, until told otherwise or the view
    /// is dropped: a new desc ([`view`](Self::view)) keeps it. A view not
    /// added yet is left alone (the host pauses it once added).
    fn pause(&mut self, view: ViewId, paused: bool);
    /// Name the view the user works with: after a restart it takes the
    /// first turn. Restarts nothing.
    fn focus(&mut self, view: ViewId);
    /// Why the view can't render, once it failed (it wakes the host once
    /// when it does): its camera isn't in the scene, its film is more than
    /// the device holds, a pass failed. Its neighbours render on.
    fn problem(&self, view: ViewId) -> Option<Diagnostic>;
    /// What the engine said of the scene since the host last asked, each
    /// once: what it doesn't render as sent (a node adoption cut, a missing
    /// texture), apart from a view's failure ([`problem`](Self::problem)).
    /// It wakes the host once when it says something.
    fn said(&mut self) -> Vec<Diagnostic>;
    /// Read a view's outputs once its samples are done. The result
    /// resolves once: the outputs as one Image, a layer each (the beauty the
    /// main one; [`crate::aov::image`] makes it), or why there are none (a
    /// view that can't render fails at once, saying its problem; a failed
    /// read; the request dropped with the view or the engine).
    fn finish(&mut self, view: ViewId, outputs: &[AovDesc]) -> Pending<Image>;
    /// A picture of a material, rendered off the caller's thread; never
    /// blocks. The host drops the result to cancel it ([`Deliver::wanted`]).
    /// A host asks previews of an engine it made for them, never of one
    /// showing views: each request's scene replaces the last one's, and the
    /// engine renders it as it renders a view, with no second engine.
    ///
    /// The picture is an Image of the request's side a side, its main layer
    /// RGBA floats, linear ACEScg, premultiplied.
    fn preview(&mut self, request: PreviewRequest) -> Pending<Image>;
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
        assert_eq!(
            result.take(),
            Some(Err(Error::new(ErrorKind::Cancelled, ABANDONED)))
        );
        assert_eq!(result.take(), None);
    }

    #[test]
    fn a_failure_resolves_once_saying_why() {
        let (wake, wakes) = counted();
        let (deliver, result) = pending::<u32>(wake);
        let failed = Error::invalid("the film is wider than the device allows");
        deliver.fail(failed.clone());
        assert_eq!(
            wakes.load(Ordering::SeqCst),
            1,
            "failing wakes; the drop after it doesn't"
        );
        assert_eq!(result.take(), Some(Err(failed)));
    }
}
