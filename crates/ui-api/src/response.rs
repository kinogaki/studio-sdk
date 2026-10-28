//! [`Response`]: what a widget or an interactive area reports, and
//! [`Sense`]: what it listens for.
use crate::geom::{Pos, Rect, Vec2};
use crate::id::Id;

/// What an area listens for. Hover is always sensed.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Sense {
    /// Clicks (and double clicks).
    pub click: bool,
    /// Drags.
    pub drag: bool,
    /// Keyboard focus, without clicks or drags (Tab reaches it).
    pub focusable: bool,
}

impl Sense {
    /// Hover only.
    pub const HOVER: Sense = Sense {
        click: false,
        drag: false,
        focusable: false,
    };
    /// Clicks.
    pub const CLICK: Sense = Sense {
        click: true,
        drag: false,
        focusable: false,
    };
    /// Drags.
    pub const DRAG: Sense = Sense {
        click: false,
        drag: true,
        focusable: false,
    };
    /// Clicks and drags.
    pub const CLICK_AND_DRAG: Sense = Sense {
        click: true,
        drag: true,
        focusable: false,
    };
    /// Keyboard focus only.
    pub const FOCUSABLE: Sense = Sense {
        click: false,
        drag: false,
        focusable: true,
    };
}

/// What happened to a widget or area this frame, as plain data. Fields
/// the area did not sense stay false.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Response {
    /// The widget's id.
    pub id: Id,
    /// Where it was drawn.
    pub rect: Rect,
    /// The pointer is over it (and nothing above it).
    pub hovered: bool,
    /// The primary button clicked it.
    pub clicked: bool,
    /// The secondary button clicked it.
    pub secondary_clicked: bool,
    /// The primary button clicked it twice.
    pub double_clicked: bool,
    /// A drag on it began this frame.
    pub drag_started: bool,
    /// It is being dragged.
    pub dragged: bool,
    /// A drag on it ended this frame.
    pub drag_stopped: bool,
    /// How far the drag moved this frame.
    pub drag_delta: Vec2,
    /// Where the pointer acts on it, while hovered or dragged.
    pub interact_pos: Option<Pos>,
    /// Its value changed (a field, a toggle, a slider).
    pub changed: bool,
    /// It has the keyboard focus.
    pub has_focus: bool,
    /// It lost the keyboard focus this frame (a text field's edit ended).
    pub lost_focus: bool,
}

impl Response {
    /// Nothing happened to widget `id` at `rect`.
    pub fn new(id: Id, rect: Rect) -> Self {
        Response {
            id,
            rect,
            ..Response::default()
        }
    }

    /// Both together: `rect` spans both, the flags of either.
    pub fn union(self, other: Response) -> Response {
        Response {
            id: self.id,
            rect: self.rect.union(other.rect),
            hovered: self.hovered || other.hovered,
            clicked: self.clicked || other.clicked,
            secondary_clicked: self.secondary_clicked || other.secondary_clicked,
            double_clicked: self.double_clicked || other.double_clicked,
            drag_started: self.drag_started || other.drag_started,
            dragged: self.dragged || other.dragged,
            drag_stopped: self.drag_stopped || other.drag_stopped,
            drag_delta: self.drag_delta + other.drag_delta,
            interact_pos: self.interact_pos.or(other.interact_pos),
            changed: self.changed || other.changed,
            has_focus: self.has_focus || other.has_focus,
            lost_focus: self.lost_focus || other.lost_focus,
        }
    }
}

/// What a closure inside a layout returned, with the layout's response.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct InnerResponse<R, T = Response> {
    /// What the closure returned.
    pub inner: R,
    /// What the layout (or card, or popover) reported.
    pub response: T,
}
