//! The plain values a context's parts and the host exchange: levels, the
//! stacked list's candidates, what live tools take and why they end, the
//! gizmo shown, panels, the numeric bar and a context's time. The host
//! draws all of them with its own components,
//! so every context's chrome looks and behaves alike.
use crate::{Id, Rect};
use kinogaki_studio_api::Parameters;
use kinogaki_studio_api::cook::Handle;
use kinogaki_studio_api::edit::select::{Expand, Mode};
use std::sync::Arc;

/// One selection level of a context (Face, Edge, CV…), what the levels
/// row, the number keys and the marking menus show.
#[derive(Clone, Debug, PartialEq)]
pub struct LevelSpec {
    /// Its stable name for settings keys ("Face"), unique in its context.
    pub id: String,
    /// What the row's tooltip calls it.
    pub label: String,
    /// Its icon, by its name in Studio's icon set.
    pub icon: String,
    /// What it selects, and how a pick grows there (Mesh's Pieces: the
    /// Primitive domain, grown to connected pieces).
    pub mode: Mode,
    /// Whether it may be active together with others (Shift+number).
    pub combines: bool,
    /// What a double click grows its pick to (Mesh's edge loops at its Edge
    /// level); [`Expand::None`]: as a single click does.
    pub double: Expand,
}

/// A button after a context's levels that makes several of them active
/// at once ([`Levels::choose`](super::Levels::choose)), on the number key
/// after the last level's. It is on while the active levels are its
/// `levels`; turned off, the primary level is active alone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LevelPreset {
    /// Its stable name for settings keys.
    pub id: String,
    /// Its tooltip.
    pub label: String,
    /// Its icon, by name.
    pub icon: String,
    /// The levels it makes active, by index, in order.
    pub levels: Vec<usize>,
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

/// A live tool's handles and the rows they read and write
/// ([`LiveTools::handles`](super::LiveTools::handles)).
#[derive(Clone, Debug, PartialEq)]
pub struct LiveHandles {
    /// The handles, placed in the open object's space.
    pub handles: Vec<Handle>,
    /// The rows now.
    pub values: Arc<Parameters>,
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
