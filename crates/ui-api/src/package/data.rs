//! The plain values a context's parts hand the host: levels, toolbar
//! items, the status bar's pieces, the numeric bar, what the views draw
//! of the context's own, its time, its network's tiles, its creations and
//! its outliner rows. The host draws all of them with its own components,
//! so every context's chrome looks and behaves alike.
use crate::host::{NodeRef, ObjectRef};
use kinogaki_studio_api::params::Unit;
use kinogaki_studio_api::viewport::{ElementKind, SurfaceSource};
use kinogaki_studio_api::{Mat4, ObjectKindId};
use kinogaki_viewport_api::SceneLines;
use std::fmt;
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

/// A button of the viewport's left toolbar.
#[derive(Clone, Debug, PartialEq)]
pub struct ToolbarItem {
    /// Its stable name ([`ViewportLayer::run_tool`](super::ViewportLayer::run_tool)
    /// gets it back).
    pub id: String,
    /// Its icon, by name.
    pub icon: String,
    /// The tooltip.
    pub label: String,
    /// Drawn lit.
    pub on: bool,
    /// Greyed, with this reason on hover; pressing it does nothing.
    pub unavailable: Option<String>,
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

/// The pieces of the status bar a context owns.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Status {
    /// Left: (text, emphasized), the place first.
    pub left: Vec<(String, bool)>,
    /// The context's problems, for the error line.
    pub problems: Vec<String>,
    /// How long its last cook took, in milliseconds.
    pub took: Option<f64>,
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

/// A shaded object of a context's own (CAM's toolpath beads): a packet
/// placed in the world.
#[derive(Clone)]
pub struct ContextObject {
    /// What it draws (uploaded once per allocation).
    pub surface: Arc<dyn SurfaceSource>,
    /// Where it is: world from the surface's space.
    pub model: Mat4,
    /// One 0xRRGGBB color a face; `None`: the clay.
    pub colors: Option<Arc<[u32]>>,
    /// Drawn see-through, in the look the rest of the scene takes inside
    /// an object (a live tool's operands, bodies behind a sketch).
    pub ghost: bool,
}

impl fmt::Debug for ContextObject {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContextObject")
            .field("faces", &self.surface.face_count())
            .field("model", &self.model)
            .field("colors", &self.colors.as_ref().map(|c| c.len()))
            .field("ghost", &self.ghost)
            .finish()
    }
}

/// What the views draw of a context's own while it is open.
#[derive(Clone, Default)]
pub struct ContextView {
    /// World-space line layers drawn in every view (CAM's plate, CAD's
    /// curvature combs). The host uploads a value once per allocation, so
    /// a context keeps its `Arc` while nothing changed.
    pub lines: Vec<Arc<SceneLines>>,
    /// Shaded objects.
    pub objects: Vec<ContextObject>,
    /// Whether they take the place of the scene's objects.
    pub replaces_scene: bool,
}

impl fmt::Debug for ContextView {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ContextView")
            .field("lines", &self.lines.len())
            .field("objects", &self.objects)
            .field("replaces_scene", &self.replaces_scene)
            .finish()
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

/// A kind of object an object level can create, contributed by the
/// context that owns the kind.
#[derive(Clone, Debug, PartialEq)]
pub struct Creation {
    /// The kind (its id).
    pub kind: ObjectKindId,
    /// What menus call it.
    pub label: String,
    /// Lower-case synonyms for search.
    pub aliases: Vec<String>,
    /// The new object opens, ready for its first node.
    pub opens: bool,
    /// Only the scene root holds it: why, elsewhere.
    pub root_only: Option<String>,
    /// What a new object of this kind can start with, each offered as its
    /// own entry ("Sphere": a new Mesh object holding a Sphere).
    pub starts: Vec<Start>,
}

/// A start: a new object of a creation's kind holding one node,
/// made in one step ([`Creations::start`](super::Creations::start)).
#[derive(Clone, Debug, PartialEq)]
pub struct Start {
    /// Its stored name (the node type's id, "kinogaki.mesh.sphere").
    pub id: String,
    /// What menus call it.
    pub label: String,
    /// The family it is listed in.
    pub family: String,
    /// Lower-case synonyms for search.
    pub aliases: Vec<String>,
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
