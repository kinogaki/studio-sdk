//! [`RecordingUi`]: a [`Ui`] that records what it is asked to show and
//! plays back scripted user actions.
use super::painter::RecordingPainter;
use crate::color::Color;
use crate::geom::{Pos, Rect, Vec2, vec2};
use crate::id::Id;
use crate::input::{Input, KeyFilter};
use crate::response::{Response, Sense};
use crate::sheet::Action;
use crate::text::Font;
use crate::theme::Theme;
use crate::ui::Body;
#[cfg(doc)]
use crate::ui::Ui;
use kinogaki_studio_api::Value;
use std::cell::RefCell;
use std::rc::Rc;

/// One call a [`RecordingUi`] received, with what identifies it.
#[derive(Clone, Debug, PartialEq)]
pub enum Call {
    /// [`Ui::label`].
    Label(String),
    /// [`Ui::button`]: its text, or its icon's name when it has no text.
    Button(String),
    /// [`Ui::selectable`].
    Selectable(String),
    /// [`Ui::toggle`]: its text and the state after the call.
    Toggle(String, bool),
    /// [`Ui::drag_value`]: the value after the call.
    DragValue(f64),
    /// [`Ui::slider`]: the value after the call.
    Slider(f64),
    /// [`Ui::text_edit`]: the text after the call.
    TextEdit(String),
    /// [`Ui::choice`]: the entries and the index after the call.
    Choice(Vec<String>, usize),
    /// [`Ui::swatch`].
    Swatch(Color),
    /// [`Ui::separator`].
    Separator,
    /// [`Ui::space`].
    Space(f32),
    /// [`Ui::horizontal`].
    Horizontal,
    /// [`Ui::vertical`].
    Vertical,
    /// [`Ui::child`].
    Child(Rect),
    /// [`Ui::scroll_vertical`].
    Scroll(Id),
    /// [`Ui::enabled`].
    Enabled(bool),
    /// [`Ui::menu_button`].
    MenuButton(String),
    /// [`Ui::context_menu`].
    ContextMenu,
    /// [`Ui::menu_item`].
    MenuItem(String),
    /// [`Ui::close_menu`](crate::Ui::close_menu).
    CloseMenu,
    /// [`Ui::sheet`]: its fields' labels, in order.
    Sheet(Vec<String>),
    /// [`Ui::card`]: its title.
    Card(String),
    /// [`Ui::popover`].
    Popover(Id),
    /// [`Ui::value_fields`]: what each drawn field shows.
    ValueFields(Vec<String>),
    /// [`Ui::tooltip`].
    Tooltip(String),
    /// [`Ui::interact`].
    Interact(Id, Rect),
    /// [`Ui::allocate`].
    Allocate(Vec2),
}

/// What every region of one recording shares: the record, the script and
/// the host-side state.
#[derive(Debug, Default)]
pub(super) struct Shared {
    pub(super) calls: Vec<Call>,
    /// Texts of widgets to click, each once.
    pub(super) clicks: Vec<String>,
    /// Sheet edits to make, by field label, each once.
    pub(super) edits: Vec<(String, Value)>,
    /// Sheet actions to take, by field label, each once.
    pub(super) actions: Vec<(String, Action)>,
    /// Text the next text field gets.
    pub(super) typing: Option<String>,
    /// The value the next number field or slider gets.
    pub(super) number: Option<f64>,
    /// The index the next choice gets.
    pub(super) choose: Option<usize>,
    /// Whether menus are open (their bodies run).
    pub(super) menus_open: bool,
    pub(super) focus: Option<Id>,
    pub(super) held: Option<(Id, KeyFilter)>,
    pub(super) repaint: bool,
}

impl Shared {
    /// Whether `text` was scripted to be clicked; the click is used up.
    pub(super) fn take_click(&mut self, text: &str) -> bool {
        let at = self.clicks.iter().position(|c| c == text);
        at.map(|i| self.clicks.remove(i)).is_some()
    }
}

/// A headless [`Ui`] for plugin unit tests: it lays widgets out simply
/// (top to bottom, or left to right), records every call as a [`Call`] and
/// its drawing in a [`RecordingPainter`], and plays back a script of user
/// actions: [`click`](RecordingUi::click) a button,
/// [`edit`](RecordingUi::edit) a sheet field, [`type_text`](RecordingUi::type_text)
/// into a field.
///
/// Every layout body runs exactly once; menu bodies run only after
/// [`open_menus`](RecordingUi::open_menus). Text is measured as the
/// recording painter measures it.
pub struct RecordingUi {
    pub(super) input: Input,
    pub(super) theme: Theme,
    pub(super) id: Id,
    /// The region.
    pub(super) rect: Rect,
    /// Where the next widget goes.
    pub(super) cursor: Pos,
    pub(super) horizontal: bool,
    pub(super) enabled: bool,
    pub(super) spacing: Vec2,
    pub(super) max_width: f32,
    /// What the widgets so far take up.
    pub(super) used: Rect,
    pub(super) shared: Rc<RefCell<Shared>>,
    pub(super) painter: RecordingPainter,
}

impl RecordingUi {
    /// A region of `rect` with the reference [`Theme`] and no input.
    pub fn new(rect: Rect) -> Self {
        let theme = Theme::default();
        let spacing = Vec2::splat(theme.metrics.gap);
        RecordingUi {
            input: Input::default(),
            theme,
            id: Id::new("recording"),
            rect,
            cursor: rect.min,
            horizontal: false,
            enabled: true,
            spacing,
            max_width: rect.width(),
            used: Rect::NOTHING,
            shared: Rc::default(),
            painter: RecordingPainter::new(rect),
        }
    }

    /// With `input` as this frame's.
    pub fn with_input(mut self, input: Input) -> Self {
        self.input = input;
        self
    }

    /// With `theme`.
    pub fn with_theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    /// The calls so far, in order (nested regions' included).
    pub fn calls(&self) -> Vec<Call> {
        self.shared.borrow().calls.clone()
    }

    /// What was drawn so far.
    pub fn painter_record(&self) -> &RecordingPainter {
        &self.painter
    }

    /// The next widget showing `text` (a button's text or icon name, a
    /// selectable, a toggle, a menu entry, a card's "Done" or "Cancel")
    /// reports a click.
    pub fn click(&mut self, text: &str) {
        self.shared.borrow_mut().clicks.push(text.to_owned());
    }

    /// The next sheet with a field labelled `label` sets it to `value` and
    /// reports the change as one finished gesture.
    pub fn edit(&mut self, label: &str, value: impl Into<Value>) {
        self.shared
            .borrow_mut()
            .edits
            .push((label.to_owned(), value.into()));
    }

    /// The next sheet with a field labelled `label` reports `action` on it.
    pub fn act(&mut self, label: &str, action: Action) {
        self.shared
            .borrow_mut()
            .actions
            .push((label.to_owned(), action));
    }

    /// The next text field gets `text` in place of its own.
    pub fn type_text(&mut self, text: &str) {
        self.shared.borrow_mut().typing = Some(text.to_owned());
    }

    /// The next number field or slider is set to `value` (then clamped).
    pub fn set_number(&mut self, value: f64) {
        self.shared.borrow_mut().number = Some(value);
    }

    /// The next choice picks entry `index`.
    pub fn choose(&mut self, index: usize) {
        self.shared.borrow_mut().choose = Some(index);
    }

    /// Menus are open from now on: their bodies run.
    pub fn open_menus(&mut self) {
        self.shared.borrow_mut().menus_open = true;
    }

    /// The widget given the keyboard focus last.
    pub fn focus(&self) -> Option<Id> {
        self.shared.borrow().focus
    }

    /// The widget holding keys, and which.
    pub fn held_keys(&self) -> Option<(Id, KeyFilter)> {
        self.shared.borrow().held
    }

    /// Whether a repaint was asked for.
    pub fn repaint_requested(&self) -> bool {
        self.shared.borrow().repaint
    }

    pub(super) fn record(&self, call: Call) {
        self.shared.borrow_mut().calls.push(call);
    }

    /// A nested region at `rect` sharing this recording.
    pub(super) fn nested(&self, rect: Rect, horizontal: bool, id: Id) -> RecordingUi {
        RecordingUi {
            input: self.input.clone(),
            theme: self.theme.clone(),
            id,
            rect,
            cursor: rect.min,
            horizontal,
            enabled: self.enabled,
            spacing: self.spacing,
            max_width: self.max_width.min(rect.width()),
            used: Rect::NOTHING,
            shared: self.shared.clone(),
            painter: self.painter.clone(),
        }
    }

    /// Runs `body` in a nested region from the cursor and takes the room
    /// it used.
    pub(super) fn nest(&mut self, horizontal: bool, body: Body<'_>) -> Response {
        let rect = Rect::from_min_max(self.cursor, self.rect.max);
        let id = self.id.with(self.shared.borrow().calls.len());
        let mut inner = self.nested(rect, horizontal, id);
        body(&mut inner);
        let size = if inner.used == Rect::NOTHING {
            Vec2::ZERO
        } else {
            inner.used.max - rect.min
        };
        self.place(id, size, Sense::HOVER)
    }

    /// Takes `size` from the layout for widget `id`.
    pub(super) fn place(&mut self, id: Id, size: Vec2, sense: Sense) -> Response {
        let rect = Rect::from_min_size(self.cursor, size);
        if self.horizontal {
            self.cursor.x += size.x + self.spacing.x;
        } else {
            self.cursor.y += size.y + self.spacing.y;
        }
        self.used = self.used.union(rect);
        self.respond(id, rect, sense)
    }

    /// The response of widget `id` at `rect`: hovered and clicked as the
    /// input says.
    pub(super) fn respond(&self, id: Id, rect: Rect, sense: Sense) -> Response {
        let pointer = &self.input.pointer;
        let hovered = pointer.hover.is_some_and(|p| rect.contains(p));
        let mut response = Response::new(id, rect);
        response.hovered = hovered;
        response.contains_pointer = hovered;
        response.interact_pos = pointer.interact.filter(|_| hovered);
        if self.enabled && hovered && sense.click {
            response.clicked = pointer.primary_released();
            response.secondary_clicked = pointer.secondary_released();
        }
        if self.enabled && sense.drag && pointer.press_origin.is_some_and(|p| rect.contains(p)) {
            response.dragged = pointer.primary_down();
            response.drag_started = pointer.primary_pressed();
            response.drag_stopped = pointer.primary_released();
            response.drag_delta = if response.dragged {
                pointer.delta
            } else {
                Vec2::ZERO
            };
        }
        response
    }

    /// A widget showing `text` with padding.
    pub(super) fn text_widget(&mut self, text: &str, sense: Sense) -> Response {
        let font = Font::ui(self.theme.text_size);
        let size = RecordingPainter::measure(text, font) + self.theme.metrics.button_padding * 2.0;
        let id = self.id.with((text, self.shared.borrow().calls.len()));
        let size = size.max(self.theme.metrics.interact_size);
        self.place(id, size, sense)
    }

    /// A clicked response when `text` was scripted (and the widget is
    /// enabled).
    pub(super) fn scripted_click(
        &self,
        mut response: Response,
        text: &str,
        enabled: bool,
    ) -> Response {
        if enabled && self.enabled && self.shared.borrow_mut().take_click(text) {
            response.clicked = true;
        }
        response
    }

    /// A field: the interface's control height across the available width.
    pub(super) fn field_size(&self, width: Option<f32>) -> Vec2 {
        let width = width.unwrap_or(self.max_width.min(self.rect.max.x - self.cursor.x));
        vec2(width.max(0.0), self.theme.metrics.interact_size.y)
    }
}
