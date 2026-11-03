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
//! | [`HostSettings`] ([`HostSettingsExt`]) | packages' settings sections |
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
//! request matters runs on the host's lanes through [`Cx::latest`]. No
//! timers.
mod capabilities;
mod data;
mod network;

pub use capabilities::{
    HostDoc, HostDocExt, HostSelection, HostSettings, HostSettingsExt, HostShell, HostSlots,
    HostSlotsExt, HostStatus, HostTools, HostTransform, HostUndo, HostView,
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

/// A job [`Cx::latest`] runs off the UI thread: it polls its token and
/// stops early once cancelled (its result is dropped then anyway).
pub type LatestJob = Box<dyn FnOnce(&CancelToken) -> LatestResult + Send>;

/// What a [`LatestJob`] gives back, downcast by the package that ran it.
pub type LatestResult = Box<dyn Any + Send>;

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
    + HostSettings
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
    /// Runs `job` on the host's worker lanes as the latest job of `key`
    /// (one of the package's own names): a job of `key` still running has
    /// its token cancelled, and its result is dropped. When `job` lands the
    /// host wakes the package ([`UiPackage::poll`](crate::package::UiPackage::poll)),
    /// which takes the result with [`take_latest`](Self::take_latest). For
    /// work where only the newest request matters (soft-selection weights,
    /// a check of the shown value).
    fn latest(&mut self, key: &str, job: LatestJob);
    /// The result of `key`'s latest job, once it has landed; each result is
    /// given once.
    fn take_latest(&mut self, key: &str) -> Option<LatestResult>;
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
