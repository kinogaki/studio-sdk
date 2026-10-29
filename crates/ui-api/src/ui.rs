//! [`Ui`]: the host's immediate-mode interface, as a plugin sees it, and
//! [`UiExt`], its generic conveniences.
use crate::card::{CardResponse, CardSpec, PopoverSpec};
use crate::color::Color;
use crate::geom::{Pos, Rect, Vec2};
use crate::id::Id;
use crate::input::{Input, KeyFilter};
use crate::paint::Painter;
use crate::response::{InnerResponse, Response, Sense};
use crate::sheet::{self, Edits, FieldKey, Keyed, Schema, SheetStyle};
use crate::text::TextStyle;
use crate::theme::Theme;
use crate::value_fields::ValueFields;
use crate::widgets::{ButtonSpec, ChoiceLook, MenuItem, ScrollSpec, TextEditSpec};
use std::hash::Hash;

/// A body the host lays out: called with the `Ui` of the region it made.
pub type Body<'a> = &'a mut dyn FnMut(&mut dyn Ui);

/// What fills the menu a secondary click on a sheet row's label opens:
/// called with the menu's `Ui` and the row's field key.
pub type RowMenu<'a, K> = &'a mut dyn FnMut(&mut dyn Ui, &K);

/// A region of the host's window a plugin draws widgets into, top to
/// bottom (or left to right inside [`Ui::horizontal`]), immediate mode:
/// the plugin calls it every frame, and what it called is what shows.
///
/// The host implements it over its own UI toolkit; plugins never see that
/// toolkit. Every call happens on the UI thread, inside the host's frame,
/// and must not block. Widgets report a [`Response`]; values a widget
/// edits are written into what the plugin passed (`&mut`).
///
/// Layout methods take the body as `&mut dyn FnMut(&mut dyn Ui)` so the
/// trait stays object-safe: the host calls it **exactly once**, with the
/// nested region, before returning (menus only while open: see
/// [`Ui::menu_button`]). [`UiExt`] wraps them for closures that return a
/// value.
///
/// Implementations: Studio's, and
/// [`RecordingUi`](crate::testing::RecordingUi), which records for tests.
pub trait Ui {
    /// This frame's input.
    fn input(&self) -> &Input;

    /// The host's theme.
    fn theme(&self) -> &Theme;

    /// This region's id: salt child ids with it ([`Id::with`]).
    fn id(&self) -> Id;

    /// A painter over this region's layer, clipped to the region.
    fn painter(&self) -> Box<dyn Painter + '_>;

    /// A painter over this region's layer, clipped to `rect` within the
    /// region.
    fn painter_at(&self, rect: Rect) -> Box<dyn Painter + '_>;

    /// The room left for widgets.
    fn available(&self) -> Rect;

    /// What the widgets so far take up.
    fn min_rect(&self) -> Rect;

    /// Listen for `sense` over `rect` as `id`, without laying anything out
    /// (a handle, a custom-drawn control).
    fn interact(&mut self, rect: Rect, id: Id, sense: Sense) -> Response;

    /// Take `size` from the layout and listen for `sense` over it; paint
    /// the result with [`Ui::painter`].
    fn allocate(&mut self, size: Vec2, sense: Sense) -> Response;

    /// Show `text` beside the pointer while it rests on `response`'s
    /// widget, enabled or not: a disabled widget's tooltip says why it is.
    fn tooltip(&mut self, response: &Response, text: &str);

    /// Give the keyboard focus to widget `id`.
    fn request_focus(&mut self, id: Id);

    /// Give widget `id` the keyboard focus and keep the navigation keys
    /// `filter` names (Tab, arrows, Escape) for it instead of moving the
    /// focus.
    fn hold_keys(&mut self, id: Id, filter: KeyFilter);

    /// Draw another frame soon, though nothing happened (an animation).
    fn request_repaint(&mut self);

    /// Text, in `style`.
    fn label(&mut self, text: &str, style: TextStyle) -> Response;

    /// A button; [`Response::clicked`] when pressed.
    fn button(&mut self, spec: &ButtonSpec<'_>) -> Response;

    /// A row that is lit when `selected`; clicking it is the plugin's to
    /// act on.
    fn selectable(&mut self, selected: bool, text: &str) -> Response;

    /// A labelled toggle switch writing `on`.
    fn toggle(&mut self, on: &mut bool, text: &str) -> Response;

    /// A number field writing `value`, shown, dragged, typed and clamped
    /// as `number` says (the host formats its unit).
    fn drag_value(&mut self, value: &mut f64, number: &sheet::Number) -> Response;

    /// A slider writing `value` across `number`'s span.
    fn slider(&mut self, value: &mut f64, number: &sheet::Number) -> Response;

    /// A text field writing `text`; [`Response::changed`] while typing,
    /// [`Response::lost_focus`] when the edit ends.
    fn text_edit(&mut self, text: &mut String, spec: &TextEditSpec<'_>) -> Response;

    /// A choice of one of `items` writing the index `at`, as `look` says;
    /// `id` keeps a dropdown's state.
    fn choice(&mut self, id: Id, at: &mut usize, items: &[&str], look: ChoiceLook) -> Response;

    /// A color swatch of `size`, outlined when `selected`.
    fn swatch(&mut self, color: Color, size: Vec2, selected: bool) -> Response;

    /// A thin line across the layout.
    fn separator(&mut self);

    /// `amount` points of empty space in the layout's direction.
    fn space(&mut self, amount: f32);

    /// `body` laid out left to right.
    fn horizontal(&mut self, body: Body<'_>) -> Response;

    /// `body` laid out top to bottom.
    fn vertical(&mut self, body: Body<'_>) -> Response;

    /// `body` laid out top to bottom inside `rect`, outside the current
    /// layout (a custom-placed panel).
    fn child(&mut self, rect: Rect, body: Body<'_>) -> Response;

    /// `body` in a vertical scroll area, as `spec` says.
    fn scroll_vertical(&mut self, spec: &ScrollSpec, body: Body<'_>) -> Response;

    /// `body` with its widgets enabled or, when `enabled` is false, greyed
    /// and taking no input.
    fn enabled(&mut self, enabled: bool, body: Body<'_>) -> Response;

    /// Widgets no wider than `width` from here on.
    fn set_max_width(&mut self, width: f32);

    /// The space between widgets from here on.
    fn set_item_spacing(&mut self, spacing: Vec2);

    /// A button with `text` that opens a menu; `body` fills the menu's
    /// entries while it is open (called only then).
    fn menu_button(&mut self, text: &str, body: Body<'_>) -> Response;

    /// A menu opened by a secondary click on `response`'s widget; `body`
    /// fills it while it is open (called only then).
    fn context_menu(&mut self, response: &Response, body: Body<'_>);

    /// An entry in an open menu; [`Response::clicked`] when chosen (the
    /// menu then closes).
    fn menu_item(&mut self, item: &MenuItem<'_>) -> Response;

    /// Closes the menu this region is in, from inside its body: when
    /// something other than a [`Ui::menu_item`] acted (a text field's edit
    /// committed), or when there is nothing to offer.
    fn close_menu(&mut self);

    /// A property sheet: the host draws `schema` across the region and
    /// writes edited values into it. `menu`, when given, fills the menu a
    /// secondary click on a row's label opens, for that field. Use
    /// [`UiExt::sheet_keyed`] to key fields by your own type.
    fn sheet(
        &mut self,
        style: &SheetStyle<'_>,
        schema: &mut Schema<FieldKey>,
        menu: Option<RowMenu<'_, FieldKey>>,
    ) -> Edits<FieldKey>;

    /// A floating card around `body` (a tool's parameters).
    fn card(&mut self, spec: &CardSpec<'_>, body: Body<'_>) -> CardResponse;

    /// A popover around `body`.
    fn popover(&mut self, spec: &PopoverSpec, body: Body<'_>) -> Response;

    /// The heads-up value fields centred on `places` (screen points; fewer
    /// places than fields draw fewer), holding the keyboard while shown
    /// when `hold_focus` (see [`value_fields`](crate::value_fields)).
    /// Returns each drawn field's rectangle.
    fn value_fields(&mut self, fields: &ValueFields, places: &[Pos], hold_focus: bool)
    -> Vec<Rect>;
}

/// Generic conveniences over any [`Ui`] (`dyn Ui` included): layouts whose
/// body returns a value, and sheets keyed by the plugin's own type.
pub trait UiExt: Ui {
    /// [`Ui::horizontal`] with a body that returns a value.
    fn horizontal_with<R>(&mut self, body: impl FnOnce(&mut dyn Ui) -> R) -> InnerResponse<R> {
        let mut run = Once::new(body);
        let response = self.horizontal(&mut |ui| run.call(ui));
        InnerResponse {
            inner: run.take(),
            response,
        }
    }

    /// [`Ui::vertical`] with a body that returns a value.
    fn vertical_with<R>(&mut self, body: impl FnOnce(&mut dyn Ui) -> R) -> InnerResponse<R> {
        let mut run = Once::new(body);
        let response = self.vertical(&mut |ui| run.call(ui));
        InnerResponse {
            inner: run.take(),
            response,
        }
    }

    /// [`Ui::scroll_vertical`] with a body that returns a value.
    fn scroll_with<R>(
        &mut self,
        spec: &ScrollSpec,
        body: impl FnOnce(&mut dyn Ui) -> R,
    ) -> InnerResponse<R> {
        let mut run = Once::new(body);
        let response = self.scroll_vertical(spec, &mut |ui| run.call(ui));
        InnerResponse {
            inner: run.take(),
            response,
        }
    }

    /// [`Ui::card`] with a body that returns a value.
    fn card_with<R>(
        &mut self,
        spec: &CardSpec<'_>,
        body: impl FnOnce(&mut dyn Ui) -> R,
    ) -> InnerResponse<R, CardResponse> {
        let mut run = Once::new(body);
        let response = self.card(spec, &mut |ui| run.call(ui));
        InnerResponse {
            inner: run.take(),
            response,
        }
    }

    /// [`Ui::popover`] with a body that returns a value.
    fn popover_with<R>(
        &mut self,
        spec: &PopoverSpec,
        body: impl FnOnce(&mut dyn Ui) -> R,
    ) -> InnerResponse<R> {
        let mut run = Once::new(body);
        let response = self.popover(spec, &mut |ui| run.call(ui));
        InnerResponse {
            inner: run.take(),
            response,
        }
    }

    /// [`Ui::sheet`] over a schema keyed by the plugin's own `K`: the keys
    /// are swapped for [`FieldKey`]s (hashes of the keys, so a row keeps
    /// its widget state when rows above it come and go) for the host, and
    /// back for the edits.
    fn sheet_keyed<K: Clone + PartialEq + Hash>(
        &mut self,
        style: &SheetStyle<'_>,
        schema: &mut Schema<K>,
        menu: Option<RowMenu<'_, K>>,
    ) -> Edits<K> {
        let (keyed, mut fields) = Keyed::new(std::mem::take(schema));
        let edits = match menu {
            Some(menu) => {
                let mut by_field = |ui: &mut dyn Ui, field: &FieldKey| {
                    if let Some(key) = keyed.key(*field) {
                        menu(ui, key);
                    }
                };
                self.sheet(style, &mut fields, Some(&mut by_field))
            }
            None => self.sheet(style, &mut fields, None),
        };
        *schema = keyed.restore(fields);
        keyed.edits(edits)
    }
}

impl<T: Ui + ?Sized> UiExt for T {}

/// A run-once body behind the `FnMut` the trait takes, and what it
/// returned.
struct Once<F, R> {
    body: Option<F>,
    out: Option<R>,
}

impl<F: FnOnce(&mut dyn Ui) -> R, R> Once<F, R> {
    fn new(body: F) -> Self {
        Once {
            body: Some(body),
            out: None,
        }
    }

    /// Runs the body, the first time only.
    fn call(&mut self, ui: &mut dyn Ui) {
        if let Some(body) = self.body.take() {
            self.out = Some(body(ui));
        }
    }

    /// What the body returned.
    fn take(self) -> R {
        self.out
            .expect("the host calls a layout's body exactly once")
    }
}
