//! What a viewport layer's behaviour hooks exchange with the host: a key
//! press ([`KeyPress`]), what a live gesture holds ([`Holds`]), and the
//! rows of the viewport's popovers ([`Popover`], [`PopoverRow`],
//! [`RowEdit`]). The host draws every popover with its own components, so
//! each context's popovers look and behave alike.
use crate::Event;
use crate::sheet::{Edits, Schema};

/// A key or typed text over the viewport, as a layer's
/// [`key`](super::ViewportLayer::key) sees it: before any binding runs,
/// after the host's own modal state (a dialog, a menu, the document's
/// commands such as Save and Undo) had it.
#[derive(Clone, Debug, PartialEq)]
pub struct KeyPress {
    /// The event: a key ([`Event::Key`], pressed, released or repeated) or
    /// typed text ([`Event::Text`]).
    pub event: Event,
    /// The item of the context's own vocabulary the key is bound to, by
    /// its name; `None`: unbound, or bound to a host command.
    pub bound: Option<String>,
    /// The key opens the Tab browser.
    pub browser: bool,
    /// The key chooses a transform tool (Q, W, E, R, T).
    pub tool_key: bool,
    /// A text field has the keyboard (the layer's own value field, or one
    /// of the host's): only the layer's value field takes the event then,
    /// never a live tool's own keys.
    pub typing: bool,
}

/// What a live gesture keeps from the rest of the viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Holds {
    /// No other view takes the pointer (a drag in one view).
    pub views: bool,
    /// Space doesn't toggle the four views.
    pub space: bool,
}

/// The viewport's popovers whose rows a context gives. Shading, Viewport
/// and Camera always show their buttons, with the host's own rows after
/// the context's; Gear, Components and Snap show where the context's
/// library lists them
/// ([`ViewportSpec::popovers`](super::ViewportSpec::popovers)).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Popover {
    /// The toolbar's gear: the current tool's settings (shown while they
    /// have rows).
    Gear,
    /// Shading: how the views draw.
    Shading,
    /// Components: what the views show of the open object.
    Components,
    /// Viewport: the views' own options.
    Viewport,
    /// A view's camera menu.
    Camera,
    /// Snap: the context's snap targets.
    Snap,
}

/// One row of a popover. Keys are the context's own names for its rows;
/// [`RowEdit`] hands them back.
#[derive(Clone, Debug, PartialEq)]
pub enum PopoverRow {
    /// A muted group heading.
    Heading(String),
    /// An icon tile and its label toggling a flag.
    Toggle {
        /// The row's key.
        key: String,
        /// Its icon, by name.
        icon: String,
        /// Its label.
        label: String,
        /// Lit.
        on: bool,
        /// Shown on and disabled with this reason (the active level forces
        /// it: "Shown at the CV level"); the user's value returns when the
        /// reason goes.
        locked: Option<String>,
    },
    /// Exclusive icon tiles on one line.
    Tiles {
        /// The row's key.
        key: String,
        /// Each tile's icon and label.
        tiles: Vec<(String, String)>,
        /// The lit tile.
        chosen: Option<usize>,
    },
    /// Exclusive text choices on one line, after an optional muted label.
    Choice {
        /// The row's key.
        key: String,
        /// The muted label before them.
        label: Option<String>,
        /// Each choice's text.
        options: Vec<String>,
        /// The chosen one.
        chosen: Option<usize>,
    },
    /// Typed fields (lengths, a tool's settings) as one compact sheet.
    Fields {
        /// The row's key.
        key: String,
        /// The fields.
        schema: Schema<u32>,
    },
    /// The rows after it start a new column beside the ones before.
    Column,
    /// A menu row (the Camera menu's Look At), greyed with its reason.
    Button {
        /// The row's key.
        key: String,
        /// Its label.
        label: String,
        /// Greyed, with this reason on hover.
        unavailable: Option<String>,
    },
}

/// What the user did to one of a popover's rows.
#[derive(Clone, Debug, PartialEq)]
pub enum RowEdit {
    /// Toggle `key` turned on or off.
    Toggle(String, bool),
    /// Tile `index` of row `key` chosen.
    Tile(String, usize),
    /// Choice `index` of row `key` chosen.
    Choice(String, usize),
    /// Fields of row `key` edited: its schema with the edited values, and
    /// the edits.
    Fields {
        /// The row's key.
        key: String,
        /// Its fields, holding the edited values.
        schema: Schema<u32>,
        /// What changed.
        edits: Edits<u32>,
    },
    /// Button `key` pressed.
    Button(String),
}
