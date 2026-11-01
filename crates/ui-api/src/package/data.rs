//! The plain values a context's parts hand the host: levels, the stacked
//! list's candidates, what live tools take, the numeric bar, its time and
//! its outliner rows. The host draws all of them with its own components,
//! so every context's chrome looks and behaves alike.
use crate::host::{NodeRef, ObjectRef};
use kinogaki_studio_api::params::Unit;
use kinogaki_studio_api::viewport::ElementKind;
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
    /// What it addresses.
    pub level: ElementKind,
    /// Whether it may be active together with others (Shift+number).
    pub combines: bool,
}

/// A button after a context's levels that turns several of them on at
/// once (Mesh's Smart: faces, edges and points), on the number key after
/// the last level's. Which levels it turns on is the context's own
/// [`active`](super::Levels::active) while it is on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelPreset {
    /// Its stable name for settings keys ("Smart").
    pub id: String,
    /// Its tooltip ("Smart selection").
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

/// What a context's live tools take this frame, before camera navigation.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tools {
    /// A tool owns the pointer: no picking, no gizmo.
    pub busy: bool,
    /// The wheel belongs to the tool, not the camera.
    pub wheel: bool,
    /// No right-click menu now.
    pub menu: bool,
}

/// The numeric bar under the active view: its fields in order.
#[derive(Clone, Debug, PartialEq)]
pub struct NumericBar {
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
    /// The value [`ViewportLayer::apply_numeric`](super::ViewportLayer::apply_numeric)
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

/// One outliner row inside an object of a kind a context owns.
#[derive(Clone, Debug, PartialEq)]
pub struct OutlineRow {
    /// The object it is inside.
    pub object: ObjectRef,
    /// The node it stands for, if any.
    pub node: Option<NodeRef>,
    /// What it shows.
    pub label: String,
    /// Its icon, by name.
    pub icon: String,
    /// Its collapse key, unique among the object's rows.
    pub key: String,
    /// Its depth under the object (0: right under it).
    pub depth: usize,
    /// It has rows under it.
    pub branch: bool,
    /// It is picked (drawn selected).
    pub picked: bool,
}
