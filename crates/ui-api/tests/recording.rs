//! A small plugin panel driven through the headless fakes: layouts that
//! return values, a sheet keyed by the plugin's own type, menus, a card,
//! overlays and value fields fed by input.
use studio_ui_api::sheet::{Action, Field, Number, Schema};
use studio_ui_api::testing::{Call, RecordingUi};
use studio_ui_api::value_fields::Typing;
use studio_ui_api::{
    Align2, ButtonSpec, CardSpec, Color, Event, Font, Id, Input, KeyFilter, MenuItem, Pos, Rect,
    Shape, SheetStyle, Stroke, TextStyle, Ui, UiExt, ValueFields, pos, vec2,
};

/// The plugin's own field keys.
#[derive(Clone, Copy, Debug, Hash, PartialEq)]
enum Param {
    Width,
    Cap,
    Rebuild,
}

/// The plugin's settings and the sheet it shows them in.
struct Settings {
    width: f64,
    cap: bool,
    rebuilt: bool,
}

impl Settings {
    fn page(&self) -> Schema<Param> {
        let mut s = Schema::new();
        s.section("shape", "Shape", true);
        s.push(Field::number(
            Param::Width,
            "Width",
            self.width,
            Number::length(),
        ));
        s.push(Field::toggle(Param::Cap, "Cap", self.cap));
        s.push(Field::button(Param::Rebuild, "", "Rebuild"));
        s
    }

    fn show(&mut self, ui: &mut dyn Ui) {
        let mut page = self.page();
        let edits = ui.sheet_keyed(&SheetStyle::new(Id::new("settings")), &mut page, None);
        if edits.changed(&Param::Width) {
            self.width = page.number(&Param::Width).unwrap();
        }
        if edits.changed(&Param::Cap) {
            self.cap = page.on(&Param::Cap).unwrap();
        }
        self.rebuilt |= edits.action(&Param::Rebuild) == Some(Action::Press);
    }
}

fn window() -> Rect {
    Rect::from_min_size(Pos::ZERO, vec2(400.0, 300.0))
}

#[test]
fn a_sheet_keyed_by_the_plugins_type_applies_scripted_edits() {
    let mut ui = RecordingUi::new(window());
    let mut settings = Settings {
        width: 1.0,
        cap: false,
        rebuilt: false,
    };
    ui.edit("Width", 2.5);
    ui.edit("Cap", true);
    ui.act("", Action::Press);
    settings.show(&mut ui);
    assert_eq!(
        (settings.width, settings.cap, settings.rebuilt),
        (2.5, true, true)
    );
    assert_eq!(
        ui.calls(),
        [Call::Sheet(vec!["Width".into(), "Cap".into(), "".into()])]
    );
    // Nothing scripted: nothing changes.
    settings.show(&mut ui);
    assert_eq!(settings.width, 2.5);
}

#[test]
fn layouts_return_their_bodies_values_and_widgets_report_clicks() {
    let mut ui = RecordingUi::new(window());
    ui.click("Apply");
    ui.click("delete");
    let row = ui.horizontal_with(|ui| {
        let apply = ui.button(&ButtonSpec::new("Apply")).clicked;
        let delete = ui.button(&ButtonSpec::icon("delete", "Delete")).clicked;
        let disabled = ui.button(&ButtonSpec::new("Apply").enabled(false)).clicked;
        (apply, delete, disabled)
    });
    assert_eq!(row.inner, (true, true, false));
    // The row sits at the top and spans its three buttons side by side.
    assert_eq!(row.response.rect.min, Pos::ZERO);
    assert!(row.response.rect.width() > 3.0 * ui.theme().metrics.interact_size.x);
    let mut width = 1.0;
    ui.set_number(-4.0);
    let changed = ui
        .vertical_with(|ui| {
            ui.label("Width", TextStyle::default());
            ui.drag_value(&mut width, &Number::length().range(0.0..=10.0))
                .changed
        })
        .inner;
    assert!(changed);
    assert_eq!(width, 0.0, "clamped");
    assert_eq!(
        ui.calls(),
        [
            Call::Horizontal,
            Call::Button("Apply".into()),
            Call::Button("delete".into()),
            Call::Button("Apply".into()),
            Call::Vertical,
            Call::Label("Width".into()),
            Call::DragValue(0.0),
        ]
    );
}

#[test]
fn menus_run_only_while_open() {
    let mut ui = RecordingUi::new(window());
    let mut chosen = None;
    ui.menu_button("Edit", &mut |ui| {
        if ui.menu_item(&MenuItem::new("Copy")).clicked {
            chosen = Some("Copy");
        }
    });
    assert_eq!(ui.calls(), [Call::MenuButton("Edit".into())]);
    ui.open_menus();
    ui.click("Copy");
    ui.menu_button("Edit", &mut |ui| {
        if ui.menu_item(&MenuItem::new("Copy").shortcut("⌘C")).clicked {
            chosen = Some("Copy");
        }
    });
    assert_eq!(chosen, Some("Copy"));
}

#[test]
fn a_card_reports_done_and_its_body() {
    let mut ui = RecordingUi::new(window());
    ui.click("Done");
    let spec = CardSpec {
        id: Id::new("bevel-card"),
        rect: Rect::from_min_size(pos(10.0, 10.0), vec2(200.0, 120.0)),
        bounds: window(),
        title: "Bevel",
        more: None,
    };
    let card = ui.card_with(&spec, |ui| {
        ui.label("Distance", TextStyle::default());
        7
    });
    assert_eq!(card.inner, 7);
    assert!(card.response.done && !card.response.cancel);
    assert_eq!(card.response.rect, spec.rect);
    assert_eq!(ui.calls()[0], Call::Card("Bevel".into()));
}

#[test]
fn overlays_are_recorded_as_shapes() {
    let ui = RecordingUi::new(window());
    let theme = ui.theme().clone();
    {
        let painter = ui.painter();
        painter.segment(
            pos(0.0, 0.0),
            pos(10.0, 10.0),
            Stroke::new(1.5, theme.axes[0]),
        );
        let at = painter.text(
            pos(50.0, 50.0),
            Align2::CENTER_CENTER,
            "12",
            Font::code(11.0),
            theme.text,
        );
        assert_eq!(at.center(), pos(50.0, 50.0));
        painter.circles(&[studio_ui_api::Circle {
            center: pos(5.0, 5.0),
            radius: 3.0,
            fill: Color::WHITE,
            stroke: Stroke::NONE,
        }]);
    }
    let shapes = ui.painter_record().shapes();
    assert_eq!(shapes.len(), 3);
    assert!(matches!(&shapes[1], Shape::Text(t) if t.text == "12"));
}

#[test]
fn value_fields_take_typing_from_input_and_hold_the_keys() {
    let input = Input {
        events: vec![Event::Text("4".into()), Event::Text("2".into())],
        ..Input::default()
    };
    let mut ui = RecordingUi::new(window()).with_input(input);
    let mut fields = ValueFields::new(2);
    fields.fields[0].live = "10".into();
    fields.fields[1].live = "45°".into();
    assert_eq!(fields.type_events(&ui.input().events), Typing::Changed);
    let rects = ui.value_fields(&fields, &[pos(100.0, 100.0), pos(100.0, 130.0)], true);
    assert_eq!(rects.len(), 2);
    assert_eq!(rects[0].center(), pos(100.0, 100.0));
    assert_eq!(
        ui.calls(),
        [Call::ValueFields(vec!["42".into(), "45°".into()])]
    );
    assert_eq!(ui.held_keys().map(|(_, f)| f), Some(KeyFilter::ALL));
}

#[test]
fn text_and_choices_take_scripted_input() {
    let mut ui = RecordingUi::new(window());
    let mut name = String::from("group1");
    ui.type_text("rim");
    let r = ui.text_edit(&mut name, &studio_ui_api::TextEditSpec::new().hint("Name"));
    assert!(r.changed && r.lost_focus);
    assert_eq!(name, "rim");
    let mut at = 0;
    ui.choose(2);
    let r = ui.choice(
        Id::new("mode"),
        &mut at,
        &["A", "B", "C"],
        studio_ui_api::ChoiceLook::Dropdown,
    );
    assert!(r.changed);
    assert_eq!(at, 2);
    let mut on = false;
    ui.click("Cap");
    assert!(ui.toggle(&mut on, "Cap").changed && on);
}
