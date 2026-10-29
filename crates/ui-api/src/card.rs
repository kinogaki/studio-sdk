//! Floating panels the host draws around a plugin's content: a tool's
//! [card](CardSpec) and a [popover](PopoverSpec).
use crate::geom::{Pos, Rect};
use crate::id::Id;

/// A floating parameter card: a rounded panel with a title row (an
/// optional More/Less toggle, Done and ×) over a scrolling body, usually a
/// compact [sheet](crate::Ui::sheet). Enter and Escape stay with the
/// plugin, which owns what the card adjusts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CardSpec<'a> {
    /// Keeps the card's widget state apart from other cards'.
    pub id: Id,
    /// Where it goes and its size; it is kept inside `bounds`.
    pub rect: Rect,
    /// What it is kept inside (the viewport, say).
    pub bounds: Rect,
    /// The title row's text.
    pub title: &'a str,
    /// `Some(expanded)` shows a More/Less toggle in that state.
    pub more: Option<bool>,
}

/// What a card reports.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct CardResponse {
    /// The whole card, for routing the pointer (clicks on it are not the
    /// viewport's).
    pub rect: Rect,
    /// Done was clicked.
    pub done: bool,
    /// The × was clicked.
    pub cancel: bool,
    /// The More/Less toggle was clicked.
    pub toggle_more: bool,
    /// Where the More/Less toggle is, when shown.
    pub more_rect: Option<Rect>,
}

/// Which layer a floating panel draws in, from the bottom.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Order {
    /// Under the panes' content.
    Background,
    /// With the panes.
    Middle,
    /// Over the panes: cards and popovers.
    #[default]
    Foreground,
    /// Over everything: tooltips and drag previews.
    Tooltip,
}

/// How a popover's panel is drawn.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum PopoverLook {
    /// The host's popup: raised, with a shadow.
    #[default]
    Popup,
    /// A flat panel on the views: the surface's fill, a one-point border,
    /// control rounding and a gap's margin.
    Panel,
}

/// A floating panel at a point, sized by its content: a picker, a hint, a
/// chooser beside a handle, a list at the pointer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopoverSpec {
    /// Keeps its widget state apart from other panels'.
    pub id: Id,
    /// Where its top left goes.
    pub at: Pos,
    /// What it is kept inside; `None` is the window.
    pub constrain: Option<Rect>,
    /// Its layer.
    pub order: Order,
    /// How its panel is drawn.
    pub look: PopoverLook,
}

impl PopoverSpec {
    /// Popover `id` with its top left at `at`, in the foreground, kept in
    /// the window.
    pub fn new(id: Id, at: Pos) -> Self {
        PopoverSpec {
            id,
            at,
            constrain: None,
            order: Order::Foreground,
            look: PopoverLook::Popup,
        }
    }

    /// Kept inside `rect`.
    pub fn constrain(mut self, rect: Rect) -> Self {
        self.constrain = Some(rect);
        self
    }

    /// In layer `order`.
    pub fn order(mut self, order: Order) -> Self {
        self.order = order;
        self
    }

    /// Drawn as `look`.
    pub fn look(mut self, look: PopoverLook) -> Self {
        self.look = look;
        self
    }
}
