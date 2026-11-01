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
//! drawing ([`GizmoService`], [`UnitsService`]).
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
pub use data::{Ground, HostPrefs, Inspected, NodeRef, ObjectRef, Place, View, ViewCamera};
pub use network::HostNetwork;

use crate::gizmo::{GizmoFrame, Handle, TransformTool};
use crate::{Pos, Projection, Rect};
use kinogaki_studio_api::CancelToken;
use kinogaki_studio_api::params::Unit;
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
    /// The host's current theme: the one a part's [`Ui::theme`](crate::Ui::theme)
    /// draws with, so what a viewport layer colors (edge marks from
    /// [`Theme::marks`](crate::Theme::marks)) follows a changed theme.
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
}

/// The context of a call about one view (a viewport layer's pointer
/// handling and overlays).
pub trait ViewCx: Cx {
    /// The view.
    fn view(&self) -> &View;
    /// The transform gizmo, drawn by the host in this view.
    fn gizmo(&self) -> &dyn GizmoService;
}

/// The context of an inspector part's call.
pub trait InspectorCx: Cx {
    /// What the inspector shows.
    fn inspected(&self) -> Inspected;
}

/// The host's transform gizmo, in one view: the same handles, colors and
/// hit tests as the host's own tools. The geometry is
/// [`gizmo`](crate::gizmo)'s; this paints it and hit-tests it with the
/// host's drawing.
pub trait GizmoService {
    /// The view's projection for a gizmo frame at view rectangle `at`.
    fn projection(&self, at: Rect) -> Projection;
    /// Paints `tool`'s gizmo at `frame`, `hot` lit, with the handles of
    /// `axes`.
    fn paint(&self, frame: &GizmoFrame, tool: TransformTool, hot: Option<Handle>, axes: [bool; 3]);
    /// The handle of `tool`'s gizmo at `frame` under screen point `p`.
    fn hit(
        &self,
        frame: &GizmoFrame,
        tool: TransformTool,
        p: Pos,
        axes: [bool; 3],
    ) -> Option<Handle>;
}

/// Numbers in the user's units, as the host's own fields read and show
/// them.
pub trait UnitsService {
    /// The value `text` means for a number measuring `unit` (arithmetic,
    /// unit suffixes, `+=` relative to `current`), in stored units; `None`
    /// when it does not read.
    fn parse(&self, text: &str, current: f64, unit: Unit) -> Option<f64>;
    /// `stored`, measuring `unit`, as text with `decimals` places and the
    /// unit's suffix.
    fn format(&self, stored: f64, unit: Unit, decimals: usize) -> String;
}
