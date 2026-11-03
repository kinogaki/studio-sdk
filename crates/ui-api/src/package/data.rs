//! The plain values a context's parts and the host exchange: levels, the
//! stacked list's candidates, what live tools take and why they end, the
//! gizmo shown, panels, the numeric bar and a context's time. The host
//! draws all of them with its own components,
//! so every context's chrome looks and behaves alike.
use crate::{Id, Rect};
use kinogaki_studio_api::edit::select::Mode;
use kinogaki_studio_api::params::Unit;
use std::sync::Arc;

/// One selection level of a context (Face, Edge, CV…), what the levels
/// row, the number keys and the marking menus show.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelSpec {
    /// Its stable name for settings keys ("Face"), unique in its context.
    pub id: String,
    /// What the row's tooltip calls it.
    pub label: String,
    /// Its icon, by name (Studio's or a package's).
    pub icon: String,
    /// What it selects, and how a pick grows there (Mesh's Pieces: the
    /// Primitive domain, grown to connected pieces).
    pub mode: Mode,
    /// Whether it may be active together with others (Shift+number).
    pub combines: bool,
}

/// A button after a context's levels that turns several of them on at
/// once, on the number key after the last level's. Which levels it turns on is the context's own
/// [`active`](super::Levels::active) while it is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelPreset {
    /// Its stable name for settings keys.
    pub id: String,
    /// Its tooltip.
    pub label: String,
    /// Its icon, by name.
    pub icon: String,
}

/// One thing under the pointer, a row of the stacked list
/// (Ctrl+Shift+click): what a click on it would select.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Candidate {
    /// Its level, by index.
    pub level: usize,
    /// The context's own handle for it, meaningful only to the context
    /// that gave it ([`Levels::pick_candidate`](super::Levels::pick_candidate)
    /// gets it back): an element id, or an index into the context's own
    /// list.
    pub id: u32,
    /// The row's text ("Edge 412").
    pub label: String,
}

/// What a context's live tools take of a view's input this frame
/// ([`LiveTools::tools`](super::LiveTools::tools)), before core's own; each
/// left `false` stays core's.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Takes {
    /// Clicks, boxes and the stacked list: core picks nothing.
    pub picks: bool,
    /// The transform gizmo: core shows none in the view.
    pub gizmo: bool,
    /// The selected node's handles: core shows none in the view.
    pub handles: bool,
    /// What lies under the pointer: core shows nothing hovered.
    pub hover: bool,
    /// The wheel: the camera doesn't dolly.
    pub wheel: bool,
    /// The right-click: no views' menu.
    pub menu: bool,
}

impl Takes {
    /// Every one: a tool that owns the view's pointer.
    pub const ALL: Takes = Takes {
        picks: true,
        gizmo: true,
        handles: true,
        hover: true,
        wheel: true,
        menu: true,
    };
}

/// Which transform gizmo the views show for a context's selection
/// ([`Transforms::gizmo`](super::Transforms::gizmo)).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum GizmoUse {
    /// Neither: nothing to move.
    #[default]
    None,
    /// The host's gizmo at the selection's pivot, its drags handed to
    /// [`Transforms::transform`](super::Transforms::transform); the selected
    /// node's gizmo handles hide.
    Selection,
    /// The selected node's declared handles, in the transform tool's
    /// place (Maya's manipulators).
    NodeHandles,
}

/// Why a live gesture ends ([`LiveTools::cancel`](super::LiveTools::cancel)).
///
/// Escape is taken by the first of: a handle drag under way (it puts back
/// the drag alone; the live tool stays), the host's open popup, popover,
/// menu or Tab browser (it closes), the transform tool's Pick (it ends),
/// and then the live tool, which is told [`Ended::Escape`]. The tools' own
/// [`key`](super::LiveTools::key) never sees Escape.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Ended {
    /// The user pressed Escape over the views.
    Escape,
    /// An undo or redo: what the gesture did is about to change under it.
    Undo,
    /// The location left the context (another object, a level up).
    Left,
}

/// A context's floating panel over the views ([`Panels`](super::Panels)).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Panel {
    /// Its id, unique among the context's panels this frame; a view's
    /// own panel names its view.
    pub id: Id,
    /// Where it is, in screen points.
    pub rect: Rect,
    /// Over what.
    pub scope: PanelScope,
}

/// What a [`Panel`] lies over.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PanelScope {
    /// One view: drawn clipped to it.
    View,
    /// The whole view pane (a tool's card): drawn once, over every view.
    Pane,
}

/// The numeric bar under the active view: its fields in order.
#[derive(Clone, Debug, PartialEq)]
pub struct NumericFields {
    /// The fields.
    pub fields: Vec<BarField>,
    /// What the numbers measure.
    pub unit: Unit,
}

/// One field of the numeric bar.
#[derive(Clone, Debug, PartialEq)]
pub struct BarField {
    /// Shown before the field; `None`: an axis (X, Y, Z by position, in
    /// the axis colors).
    pub name: Option<String>,
    /// What it holds.
    pub value: BarValue,
    /// Greyed, with this reason on hover: it takes no value.
    pub unavailable: Option<String>,
}

/// What a numeric bar field holds.
#[derive(Clone, Debug, PartialEq)]
pub enum BarValue {
    /// A number typed in.
    Number(f64),
    /// A whole number, unitless.
    Count(u32),
    /// One of `labels`, `at` chosen.
    Choice {
        /// The choices.
        labels: Vec<String>,
        /// The chosen one.
        at: usize,
    },
}

impl BarValue {
    /// The value [`NumericBar::apply`](super::NumericBar::apply)
    /// gets as current.
    pub fn value(&self) -> f64 {
        match *self {
            Self::Number(v) => v,
            Self::Count(n) => f64::from(n),
            Self::Choice { at, .. } => at as f64,
        }
    }
}

/// How a context's time is labelled and stepped.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TimeUnits {
    /// Frames at this many a second.
    Frames(f64),
    /// Seconds, labelled h:mm:ss.
    Clock,
}

/// A mark on a context's time ruler.
#[derive(Clone, Debug, PartialEq)]
pub struct TimeMark {
    /// Where, in seconds.
    pub at: f64,
    /// A layer's start (true) or a key.
    pub layer: bool,
    /// Its name, on hover.
    pub label: String,
}

/// A context's own time, which the Timeline shows and drives instead of
/// the scene's while the context is open (CAM: the print's seconds).
#[derive(Clone, Debug, PartialEq)]
pub struct TimeDomain {
    /// Start and end, in seconds.
    pub range: (f64, f64),
    /// How the ruler labels, snaps and steps.
    pub units: TimeUnits,
    /// Sorted by time; built once a result and shared.
    pub marks: Arc<[TimeMark]>,
    /// Now.
    pub current: f64,
    /// The playback speeds offered; none: 1×.
    pub speeds: Vec<f64>,
}
