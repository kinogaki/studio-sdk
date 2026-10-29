//! [`RecordingUi`]'s [`Ui`]: each widget laid out, recorded and answered
//! from the script.
use super::painter::RecordingPainter;
use super::ui::{Call, RecordingUi};
use crate::card::{CardResponse, CardSpec, PopoverSpec};
use crate::color::Color;
use crate::geom::{Pos, Rect, Vec2, vec2};
use crate::id::Id;
use crate::input::{Input, KeyFilter};
use crate::paint::Painter;
use crate::response::{Response, Sense};
use crate::sheet::{Edits, FieldKey, Number, Schema, SheetStyle};
use crate::text::{Font, TextStyle};
use crate::theme::Theme;
use crate::ui::{Body, RowMenu, Ui};
use crate::value_fields::ValueFields;
use crate::widgets::{ButtonSpec, ChoiceLook, MenuItem, ScrollSpec, TextEditSpec};

impl Ui for RecordingUi {
    fn input(&self) -> &Input {
        &self.input
    }

    fn theme(&self) -> &Theme {
        &self.theme
    }

    fn id(&self) -> Id {
        self.id
    }

    fn painter(&self) -> Box<dyn Painter + '_> {
        self.painter.clipped(self.rect)
    }

    fn painter_at(&self, rect: Rect) -> Box<dyn Painter + '_> {
        self.painter.clipped(self.rect.intersect(rect))
    }

    fn available(&self) -> Rect {
        Rect::from_min_max(self.cursor, self.rect.max)
    }

    fn min_rect(&self) -> Rect {
        self.used
    }

    fn interact(&mut self, rect: Rect, id: Id, sense: Sense) -> Response {
        self.record(Call::Interact(id, rect));
        self.respond(id, rect, sense)
    }

    fn allocate(&mut self, size: Vec2, sense: Sense) -> Response {
        self.record(Call::Allocate(size));
        let id = self.id.with(("allocate", self.shared.borrow().calls.len()));
        self.place(id, size, sense)
    }

    fn tooltip(&mut self, _response: &Response, text: &str) {
        self.record(Call::Tooltip(text.to_owned()));
    }

    fn request_focus(&mut self, id: Id) {
        self.shared.borrow_mut().focus = Some(id);
    }

    fn hold_keys(&mut self, id: Id, filter: KeyFilter) {
        let mut shared = self.shared.borrow_mut();
        shared.focus = Some(id);
        shared.held = Some((id, filter));
    }

    fn request_repaint(&mut self) {
        self.shared.borrow_mut().repaint = true;
    }

    fn label(&mut self, text: &str, style: TextStyle) -> Response {
        self.record(Call::Label(text.to_owned()));
        let font = style.font.unwrap_or(Font::ui(self.theme.text_size));
        let color = style.color.unwrap_or(self.theme.text);
        let size = RecordingPainter::measure(text, font);
        let id = self.id.with(("label", self.shared.borrow().calls.len()));
        let response = self.place(id, size, Sense::HOVER);
        self.painter.clipped(self.rect).text(
            response.rect.min,
            crate::geom::Align2::LEFT_TOP,
            text,
            font,
            color,
        );
        response
    }

    fn button(&mut self, spec: &ButtonSpec<'_>) -> Response {
        let name = if spec.text.is_empty() {
            spec.icon.unwrap_or_default()
        } else {
            spec.text
        };
        self.record(Call::Button(name.to_owned()));
        let response = self.text_widget(name, Sense::CLICK);
        self.scripted_click(response, name, spec.enabled)
    }

    fn selectable(&mut self, _selected: bool, text: &str) -> Response {
        self.record(Call::Selectable(text.to_owned()));
        let response = self.text_widget(text, Sense::CLICK);
        self.scripted_click(response, text, true)
    }

    fn toggle(&mut self, on: &mut bool, text: &str) -> Response {
        let response = self.text_widget(text, Sense::CLICK);
        let mut response = self.scripted_click(response, text, true);
        if response.clicked {
            *on = !*on;
            response.changed = true;
        }
        self.record(Call::Toggle(text.to_owned(), *on));
        response
    }

    fn drag_value(&mut self, value: &mut f64, number: &Number) -> Response {
        let size = vec2(
            self.theme.metrics.slider_width,
            self.theme.metrics.interact_size.y,
        );
        let id = self
            .id
            .with(("drag-value", self.shared.borrow().calls.len()));
        let mut response = self.place(id, size, Sense::CLICK_AND_DRAG);
        if let Some(v) = self
            .shared
            .borrow_mut()
            .number
            .take()
            .filter(|_| self.enabled)
        {
            *value = number.clamp(v);
            response.changed = true;
        }
        self.record(Call::DragValue(*value));
        response
    }

    fn slider(&mut self, value: &mut f64, number: &Number) -> Response {
        let size = vec2(
            self.theme.metrics.slider_width,
            self.theme.metrics.interact_size.y,
        );
        let id = self.id.with(("slider", self.shared.borrow().calls.len()));
        let mut response = self.place(id, size, Sense::CLICK_AND_DRAG);
        if let Some(v) = self
            .shared
            .borrow_mut()
            .number
            .take()
            .filter(|_| self.enabled)
        {
            *value = number.clamp(v);
            response.changed = true;
        }
        self.record(Call::Slider(*value));
        response
    }

    fn text_edit(&mut self, text: &mut String, spec: &TextEditSpec<'_>) -> Response {
        let size = self.field_size(spec.width);
        let id = spec
            .id
            .unwrap_or_else(|| self.id.with(("text", self.shared.borrow().calls.len())));
        let mut response = self.place(id, size, Sense::CLICK);
        let typed = (spec.enabled && self.enabled)
            .then(|| self.shared.borrow_mut().typing.take())
            .flatten();
        if let Some(typed) = typed {
            *text = typed;
            response.changed = true;
            response.lost_focus = true;
        }
        self.record(Call::TextEdit(text.clone()));
        response
    }

    fn choice(&mut self, id: Id, at: &mut usize, items: &[&str], _look: ChoiceLook) -> Response {
        let size = self.field_size(None);
        let mut response = self.place(id, size, Sense::CLICK);
        let chosen = self.shared.borrow_mut().choose.take();
        if let Some(i) = chosen.filter(|&i| self.enabled && i < items.len()) {
            *at = i;
            response.changed = true;
        }
        self.record(Call::Choice(
            items.iter().map(|&s| s.to_owned()).collect(),
            *at,
        ));
        response
    }

    fn swatch(&mut self, color: Color, size: Vec2, _selected: bool) -> Response {
        self.record(Call::Swatch(color));
        let id = self.id.with(("swatch", self.shared.borrow().calls.len()));
        let response = self.place(id, size, Sense::CLICK);
        self.painter
            .clipped(self.rect)
            .rect_filled(response.rect, 0.0, color);
        response
    }

    fn separator(&mut self) {
        self.record(Call::Separator);
        let size = if self.horizontal {
            vec2(1.0, 0.0)
        } else {
            vec2(0.0, 1.0)
        };
        let id = self
            .id
            .with(("separator", self.shared.borrow().calls.len()));
        self.place(id, size, Sense::HOVER);
    }

    fn space(&mut self, amount: f32) {
        self.record(Call::Space(amount));
        if self.horizontal {
            self.cursor.x += amount;
        } else {
            self.cursor.y += amount;
        }
    }

    fn horizontal(&mut self, body: Body<'_>) -> Response {
        self.record(Call::Horizontal);
        self.nest(true, body)
    }

    fn vertical(&mut self, body: Body<'_>) -> Response {
        self.record(Call::Vertical);
        self.nest(false, body)
    }

    fn child(&mut self, rect: Rect, body: Body<'_>) -> Response {
        self.record(Call::Child(rect));
        let id = self.id.with(("child", self.shared.borrow().calls.len()));
        let mut inner = self.nested(rect, false, id);
        body(&mut inner);
        self.respond(id, rect, Sense::HOVER)
    }

    fn scroll_vertical(&mut self, spec: &ScrollSpec, body: Body<'_>) -> Response {
        self.record(Call::Scroll(spec.id));
        self.nest(false, body)
    }

    fn enabled(&mut self, enabled: bool, body: Body<'_>) -> Response {
        self.record(Call::Enabled(enabled));
        let was = self.enabled;
        self.enabled = was && enabled;
        let response = self.nest(false, body);
        self.enabled = was;
        response
    }

    fn set_max_width(&mut self, width: f32) {
        self.max_width = width;
    }

    fn set_item_spacing(&mut self, spacing: Vec2) {
        self.spacing = spacing;
    }

    fn menu_button(&mut self, text: &str, body: Body<'_>) -> Response {
        self.record(Call::MenuButton(text.to_owned()));
        let response = self.text_widget(text, Sense::CLICK);
        if self.shared.borrow().menus_open {
            let mut menu = self.nested(self.rect, false, response.id.with("menu"));
            body(&mut menu);
        }
        response
    }

    fn context_menu(&mut self, response: &Response, body: Body<'_>) {
        self.record(Call::ContextMenu);
        if self.shared.borrow().menus_open {
            let mut menu = self.nested(self.rect, false, response.id.with("context-menu"));
            body(&mut menu);
        }
    }

    fn menu_item(&mut self, item: &MenuItem<'_>) -> Response {
        self.record(Call::MenuItem(item.text.to_owned()));
        let response = self.text_widget(item.text, Sense::CLICK);
        self.scripted_click(response, item.text, item.enabled)
    }

    fn close_menu(&mut self) {
        self.record(Call::CloseMenu);
    }

    fn sheet(
        &mut self,
        style: &SheetStyle<'_>,
        schema: &mut Schema<FieldKey>,
        _menu: Option<RowMenu<'_, FieldKey>>,
    ) -> Edits<FieldKey> {
        self.record(Call::Sheet(
            schema.fields().map(|f| f.label.to_string()).collect(),
        ));
        let rows = schema.items.len() as f32;
        let row = if style.compact {
            self.theme.metrics.card_row
        } else {
            self.theme.metrics.property_row
        };
        let width = self.field_size(None).x;
        self.place(style.id, vec2(width, rows * row), Sense::HOVER);
        let mut edits = Edits::default();
        let mut shared = self.shared.borrow_mut();
        for field in schema.fields_mut().filter(|f| f.enabled && self.enabled) {
            if let Some(at) = shared.edits.iter().position(|(l, _)| *l == field.label) {
                field.value = Some(shared.edits.remove(at).1);
                edits.changed.push(field.key);
                edits.started = true;
                edits.finished = true;
                edits.active.get_or_insert(field.key);
            }
            if let Some(at) = shared.actions.iter().position(|(l, _)| *l == field.label) {
                edits.actions.push((field.key, shared.actions.remove(at).1));
            }
        }
        edits
    }

    fn card(&mut self, spec: &CardSpec<'_>, body: Body<'_>) -> CardResponse {
        self.record(Call::Card(spec.title.to_owned()));
        let mut inner = self.nested(spec.rect, false, spec.id);
        body(&mut inner);
        let mut shared = self.shared.borrow_mut();
        CardResponse {
            rect: spec.rect,
            done: shared.take_click("Done"),
            cancel: shared.take_click("Cancel"),
            toggle_more: spec.more.is_some()
                && shared.take_click(if spec.more == Some(true) {
                    "Less"
                } else {
                    "More"
                }),
            more_rect: None,
        }
    }

    fn popover(&mut self, spec: &PopoverSpec, body: Body<'_>) -> Response {
        self.record(Call::Popover(spec.id));
        let bounds = spec.constrain.unwrap_or(self.rect);
        let rect = Rect::from_min_max(spec.at, bounds.max);
        let mut inner = self.nested(rect, false, spec.id);
        body(&mut inner);
        let used = if inner.used == Rect::NOTHING {
            Rect::from_min_size(spec.at, Vec2::ZERO)
        } else {
            inner.used
        };
        self.respond(spec.id, used, Sense::HOVER)
    }

    fn value_fields(
        &mut self,
        fields: &ValueFields,
        places: &[Pos],
        hold_focus: bool,
    ) -> Vec<Rect> {
        let shown: Vec<&crate::value_fields::ValueField> =
            fields.fields.iter().take(places.len()).collect();
        self.record(Call::ValueFields(
            shown.iter().map(|f| f.shown().to_owned()).collect(),
        ));
        if hold_focus {
            self.hold_keys(self.id.with("value-fields"), KeyFilter::ALL);
        }
        let font = Font::code(11.0);
        shown
            .iter()
            .zip(places)
            .map(|(f, &at)| {
                let text = match &f.label {
                    Some(l) => format!("{l} {}", f.shown()),
                    None => f.shown().to_owned(),
                };
                Rect::from_center_size(at, RecordingPainter::measure(&text, font) + vec2(10.0, 4.0))
            })
            .collect()
    }
}
