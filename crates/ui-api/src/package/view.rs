//! What a viewport layer's behaviour hooks exchange with the host: a key
//! press ([`KeyPress`]) and what a live gesture holds ([`Holds`]).
use crate::Event;

/// A key or typed text over the viewport, as live tools'
/// [`key`](super::LiveTools::key) see it: before any binding runs, while
/// nothing over the panes has the keyboard, the document's commands (Save,
/// Undo) left to the host. A repeat or a release comes as its press came.
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
}

/// What a live gesture keeps from the rest of the viewport.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Holds {
    /// No other view takes the pointer (a drag in one view).
    pub views: bool,
    /// Space doesn't toggle the four views.
    pub space: bool,
}
