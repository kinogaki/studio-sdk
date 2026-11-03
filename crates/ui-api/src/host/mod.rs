//! The host interface: what a package's UI asks of Studio.
//!
//! A package's context code never sees the editor. Every call into it
//! gets a context, `&mut dyn Cx` (or the [`ViewCx`] and [`InspectorCx`]
//! variants a view's and the inspector's parts get), which reaches the
//! host's capabilities, one trait each:
//!
//! | Trait | What |
//! |---|---|
//! | [`HostUndo`] | undo, redo and gestures |
//! | [`HostSlots`] ([`HostSlotsExt`]) | the session's slots, typed |
//! | [`HostSelection`] | selected objects, picked nodes |
//! | [`HostDoc`] ([`HostDocExt`]) | where the user is, objects and their bodies, packages' stored values |
//! | [`HostNetwork`] | objects' networks, read and edited |
//! | [`HostView`] | views, cameras, framing, redraws |
//! | [`HostStatus`] | the error line and the Console |
//! | [`HostShell`] | actions and the keyboard |
//! | [`HostTransform`] | the transform tool and snapping |
//!
//! plus plain data ([`HostPrefs`]) and two services over the host's own
//! drawing ([`UnitsService`]).
//!
//! **Calls never nest.** The host keeps each package's state and lends it
//! for one call at a time; none of these methods calls back into a
//! package. What would (an undo ending a package's live tool, an action
//! running a package's command) is queued and runs after the call
//! returns. So a package may call anything on its `cx` while it holds its
//! state.
//!
//! **Workers.** A package's background work runs on its own services,
//! built with [`Cx::wake`]: the service calls the wake once per result,
//! and the host then calls the package's
//! [`poll`](crate::package::UiPackage::poll). Work where only the newest
//! request matters runs on the host's lanes through
//! [`HostLatestExt::latest`], typed by a [`LatestKey`]. No
//! timers.
mod capabilities;
mod data;
mod network;

pub use capabilities::{
    HostDoc, HostDocExt, HostSelection, HostShell, HostSlots, HostSlotsExt, HostStatus, HostTools,
    HostTransform, HostUndo, HostView,
};
pub use data::{
    Ground, HostPrefs, Inspected, NodeRef, NodeRows, ObjectRef, Place, PointerOver, View,
    ViewCamera,
};
pub use network::HostNetwork;

use crate::Pos;
use kinogaki_studio_api::facets::{SnapHit, SnapTargets};
use kinogaki_studio_api::params::Unit;
use kinogaki_studio_api::{CancelToken, RowsFor};
use std::any::Any;
use std::sync::Arc;

/// Wakes the host's UI once (a worker's result is ready). Cheap to clone;
/// callable from any thread.
pub type Waker = Arc<dyn Fn() + Send + Sync>;

/// A job [`HostLatestExt::latest`] runs off the UI thread: it polls its
/// token and stops early once cancelled (its result is dropped then
/// anyway).
pub type LatestJob = Box<dyn FnOnce(&CancelToken) -> LatestResult + Send>;

/// What a [`LatestJob`] gives back, its type erased.
pub type LatestResult = Box<dyn Any + Send>;

/// A kind of latest-wins job a package runs on the host's lanes
/// ([`HostLatestExt::latest`]): what a job is asked for, and what it
/// gives. Implemented by a type of the package's own, which names the
/// kind (a hover's pick, a check of the shown value).
///
/// The host runs a job only when what it is asked for changes, cancels
/// the one still running then, and hands back a result only for the
/// newest ask: a package asks every frame with what it wants now (an
/// element and the shown value's revision), and never compares a result
/// with what it asked itself.
pub trait LatestKey: 'static {
    /// What a job is asked for: the host compares it with the last ask.
    type Asked: Clone + PartialEq + Send + 'static;
    /// What a job gives.
    type Out: Send + 'static;
}

/// What a [`LatestJob`] was asked for, its type erased: compared by the
/// host with the next ask of its kind.
pub trait LatestAsked: Any + Send {
    /// Whether `other` is an ask equal to this one.
    fn same(&self, other: &dyn LatestAsked) -> bool;
    /// A copy of the ask (the host keeps one to compare the next with).
    fn copy(&self) -> Box<dyn LatestAsked>;
}

impl<T: Clone + PartialEq + Send + 'static> LatestAsked for T {
    fn same(&self, other: &dyn LatestAsked) -> bool {
        (other as &dyn Any).downcast_ref::<T>() == Some(self)
    }
    fn copy(&self) -> Box<dyn LatestAsked> {
        Box::new(self.clone())
    }
}

/// Typed latest-wins jobs ([`LatestKey`]) over [`Cx`]'s erased ones.
pub trait HostLatestExt: Cx {
    /// Runs `job` on the host's worker lanes as the latest job of kind `K`,
    /// asked for `asked`, unless the last ask of `K` was the same (its job
    /// stands, running or landed): a job of `K` still running then has its
    /// token cancelled, and its result is dropped. When `job` lands the
    /// host wakes the package ([`UiPackage::poll`](crate::package::UiPackage::poll)),
    /// which takes the result with [`take_latest`](Self::take_latest). For
    /// work where only the newest request matters (soft-selection weights,
    /// a check of the shown value), asked as often as wanted.
    fn latest<K: LatestKey>(
        &mut self,
        asked: K::Asked,
        job: impl FnOnce(&CancelToken) -> K::Out + Send + 'static,
    ) {
        let job: LatestJob = Box::new(move |token| Box::new(job(token)));
        self.latest_any(std::any::type_name::<K>(), Box::new(asked), job);
    }
    /// The result of kind `K`'s newest ask, with what it was asked for,
    /// once it has landed; each result is given once, and a result of an
    /// older ask never.
    fn take_latest<K: LatestKey>(&mut self) -> Option<(K::Asked, K::Out)> {
        let (asked, out) = self.take_latest_any(std::any::type_name::<K>())?;
        let asked = (asked as Box<dyn Any + Send>).downcast::<K::Asked>().ok()?;
        Some((*asked, *out.downcast::<K::Out>().ok()?))
    }
}

impl<H: Cx + ?Sized> HostLatestExt for H {}

/// One call's context: every capability of the host.
pub trait Cx:
    HostUndo
    + HostSlots
    + HostSelection
    + HostDoc
    + HostNetwork
    + HostView
    + HostStatus
    + HostShell
    + HostTransform
    + HostTools
{
    /// The user's preferences now.
    fn prefs(&self) -> HostPrefs;
    /// The host's wake, for the package's workers.
    fn wake(&self) -> Waker;
    /// Numbers typed and shown in the user's units.
    fn units(&self) -> &dyn UnitsService;
    /// The host's color service: the applied project's, and in a section
    /// of Settings ▸ Project the one of the page's draft (so a config just
    /// chosen lists its own spaces before it is applied).
    fn colors(&self) -> &dyn kinogaki_studio_api::color::ColorService;
    /// The host's current theme: the one a part's [`Ui::theme`](crate::Ui::theme)
    /// draws with, so what a viewport layer colors follows a changed theme.
    fn theme(&self) -> &crate::Theme;
    /// [`HostLatestExt::latest`], erased: `key` names the job's kind (its
    /// [`LatestKey`]'s type), unique among the calling package's.
    fn latest_any(&mut self, key: &'static str, asked: Box<dyn LatestAsked>, job: LatestJob);
    /// [`HostLatestExt::take_latest`], erased.
    fn take_latest_any(
        &mut self,
        key: &'static str,
    ) -> Option<(Box<dyn LatestAsked>, LatestResult)>;
    /// Draws node `node`'s rows into `ui` as its type gives them for
    /// `purpose` (a card's: compact, at `ui`'s width), the node of
    /// `object`'s open network; nothing for another object or node. The
    /// host's own rows: an edit is applied as the inspector's are, one undo
    /// step, or part of the open gesture (a tool's run, its card's edits,
    /// its drags and typed values stay one step). A tool's card shows its
    /// node this way instead of drawing parameters of its own.
    fn node_rows(
        &mut self,
        ui: &mut dyn crate::Ui,
        object: ObjectRef,
        node: NodeRef,
        purpose: RowsFor,
    ) -> NodeRows;
}

/// The context of a call about one view: the open context's
/// [`LiveTools::tools`](crate::package::LiveTools::tools),
/// [`Panels::panels`](crate::package::Panels::panels),
/// [`Display::overlay`](crate::package::Display::overlay) and
/// [`Display::edited`](crate::package::Display::edited).
pub trait ViewCx: Cx {
    /// The view.
    fn view(&self) -> &View;
    /// This frame's input as the view's calls see it: the pointer, the
    /// modifiers, the keys and typing the tools didn't take
    /// ([`LiveTools::key`](crate::package::LiveTools::key)), the wheel.
    fn input(&self) -> &crate::Input;
    /// The best of `targets` within `radius` pixels of screen point `at`
    /// in this view, as the host's gizmo snaps: the value `on` names
    /// asked through its type's `Snappable`, nothing of it moving, hidden
    /// targets left out; a point before an edge before a surface, each the
    /// nearest. The hit is in the open object's space, whichever value it
    /// is on. `None`: nothing near, `on`'s object is hidden or its value
    /// shows nothing, or (a node's) the value is still being cooked: the
    /// host asks for it, and a later call finds it.
    /// The host marks the last hit of a frame in this view, as it marks
    /// its gizmo's: a tool draws no snap feedback of its own. Whether to
    /// snap at all (the user's [`HostTransform::snapping`], Command) is
    /// the tool's to decide.
    fn snap(&mut self, at: Pos, targets: SnapTargets, radius: f64, on: SnapOn) -> Option<SnapHit>;
    /// Display option `id`'s value in this view: as the user set it (this
    /// view's, or the context's for a
    /// [`Context`](crate::package::DisplayScope::Context) option), else its
    /// declaration's default ([`DisplayOption`](crate::package::DisplayOption)):
    /// a toggle's 1 or 0, a choice's index. 0 for an option the open
    /// context doesn't declare.
    fn displayed(&self, id: &str) -> usize;
}

/// The value a [`ViewCx::snap`] searches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapOn {
    /// The open object's shown value, with the targets its layer adds
    /// through [`LiveTools::own_targets`](crate::package::LiveTools::own_targets).
    Open,
    /// That object's shown value.
    Object(ObjectRef),
    /// That node's cooked value, the node of that object's network (a
    /// tool's input upstream of what the object shows).
    Node(ObjectRef, NodeRef),
}

/// The context of an inspector part's call.
pub trait InspectorCx: Cx {
    /// What the inspector shows.
    fn inspected(&self) -> Inspected;
}

/// Numbers in the user's units, as the host's own fields read and show
/// them.
pub trait UnitsService {
    /// The value `text` means for a number measuring `unit` (arithmetic,
    /// unit suffixes, `+=` relative to `current`), in stored units; `None`
    /// when it does not read.
    fn parse(&self, text: &str, current: f64, unit: &Unit) -> Option<f64>;
    /// `stored`, measuring `unit`, as text with `decimals` places and the
    /// unit's suffix.
    fn format(&self, stored: f64, unit: &Unit, decimals: usize) -> String;
}
