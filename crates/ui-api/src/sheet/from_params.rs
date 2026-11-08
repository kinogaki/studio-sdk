//! A parameter [`params::Schema`] as a sheet, keyed by parameter name: what
//! the host shows of a node's parameters, a format's options, an engine's
//! settings or a prim's values. A field holds its parameter's value as it
//! is, so an edit is the parameter's new value.
use super::{Band, Field, FileDialog, Item, Kind, Number, NumberLook, Sheet};
use kinogaki_studio_api::params::{self, SchemaRow, ValueKind};

/// The sheet of a parameter schema, its rows in the schema's order
/// ([`params::Schema::placed`]): each section a top-level band over the rows
/// it shows (none over none), open as it says of the schema's values; each
/// parameter a field keyed by its name, with its help as the tooltip and
/// its keywords for search, shown and enabled as its conditions say of the
/// schema's values; each button a button field. A data parameter has no
/// field: its node's schema adds rows of its own.
impl From<&params::Schema> for Sheet<String> {
    fn from(schema: &params::Schema) -> Self {
        let values = schema.values();
        let mut items = Vec::new();
        let mut banded: Option<&str> = None;
        for (section, row) in schema.placed() {
            let item = match row {
                SchemaRow::Parameter(p) if matches!(p.kind, ValueKind::Data(_)) => continue,
                SchemaRow::Parameter(p) => match p.clone().resolved(&values) {
                    Some(p) => Item::Field(field(&p)),
                    None => continue,
                },
                SchemaRow::Button(b) => {
                    let mut field = Field::new(
                        b.name.clone(),
                        "",
                        Kind::Button {
                            text: b.label.clone().into(),
                            icon: None,
                        },
                        None,
                    );
                    field.enabled = b.enabled;
                    field.tooltip = (!b.help.is_empty()).then(|| b.help.clone().into());
                    Item::Field(field)
                }
            };
            if let Some(s) = section.filter(|s| banded != Some(s.name.as_str())) {
                banded = Some(&s.name);
                items.push(Item::Band(Band {
                    key: s.name.clone().into(),
                    label: s.label.clone().into(),
                    open: s.is_open(&values),
                    sub: false,
                }));
            }
            items.push(item);
        }
        Sheet { items }
    }
}

/// Parameter `p`'s field: its kind's, its look (a whole number's a
/// stepper), its help as the tooltip, its keywords for search.
fn field(p: &params::Parameter) -> Field<String> {
    let look = match p.kind {
        ValueKind::Integer { .. } => NumberLook::stepper(),
        _ => p.look,
    };
    let mut field = Field::new(
        p.name.clone(),
        p.label.clone(),
        Kind::from(&p.kind),
        Some(p.value.clone()),
    )
    .look(look);
    field.enabled = p.enabled;
    field.tooltip = (!p.help.is_empty()).then(|| p.help.clone().into());
    field.keywords = p.keywords.clone().into();
    field
}

/// The field kind a value kind shows as, with its options: a choice's
/// entries, an attribute row's names, an integer's menu over its values
/// from 0 (else a number its field's stepper look edits whole).
impl From<&ValueKind> for Kind {
    fn from(kind: &ValueKind) -> Self {
        match kind {
            ValueKind::Number(n) => Kind::Number(n.clone()),
            ValueKind::Integer { menu, .. } if !menu.is_empty() => Kind::Dropdown {
                items: menu.iter().map(|n| n.clone().into()).collect(),
                available: vec![true; menu.len()],
                info: None,
            },
            ValueKind::Integer { min, max, .. } => {
                Kind::Number(Number::plain().range(*min as f64, *max as f64))
            }
            ValueKind::Toggle => Kind::Toggle,
            // The host adds its configuration's spaces to a color space's.
            ValueKind::Choice { choices } | ValueKind::ColorSpace { choices } => Kind::Choice {
                choices: choices.clone(),
            },
            ValueKind::Text => Kind::Text,
            ValueKind::Code => Kind::Code {
                diagnostics: Vec::new(),
                fill: false,
                lines: None,
            },
            ValueKind::File => Kind::path(FileDialog::open()),
            ValueKind::SaveFile => Kind::path(FileDialog::save()),
            // The host fills the pickers and menus from the scene and the
            // node's input.
            ValueKind::Reference(filter) | ValueKind::Object(filter) => {
                Kind::Reference(filter.clone())
            }
            ValueKind::Path => Kind::TreePath {
                choices: Vec::new(),
                relative: false,
                tooltip: "Choose a node or object".into(),
            },
            ValueKind::Attribute { menu, list } => Kind::Name {
                names: menu.clone(),
                list: *list,
                tooltip: "The input's attributes".into(),
            },
            ValueKind::Group => Kind::Expression { pick: None },
            ValueKind::Color => Kind::Color,
            ValueKind::DisplayColor => Kind::DisplayColor,
            ValueKind::Vector { number, .. } => Kind::Vector {
                number: number.clone(),
                lock: None,
            },
            ValueKind::Ramp { color, .. } => Kind::Ramp {
                color: *color,
                interpolation: None,
            },
            ValueKind::ToneCurves { channels } => Kind::ToneCurves {
                channels: channels.clone(),
            },
            // Apart until the host says the node keeps them linked (its
            // view, not the value).
            ValueKind::Channels(number) => Kind::Channels {
                number: number.clone(),
                linked: false,
            },
            // Data is never a field (see the schema's sheet): read-only text
            // if one is made of it alone, as is a kind this sheet has no
            // field for.
            _ => Kind::Info,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet::ramp;
    use crate::sheet::{NumberStyle, Unit};
    use kinogaki_studio_api::params::Value;
    use kinogaki_studio_api::params::{Choice, Parameter, RefFilter};

    #[test]
    fn parameters_become_fields_keyed_by_name() {
        let mut schema = params::Schema::default()
            .section(params::Section::new("shape", "Shape", true))
            .parameter(
                Parameter::new(
                    "distance",
                    "Distance",
                    ValueKind::Number(params::Number::length().range(0.0, 10.0).soft(0.0, 2.0)),
                    1.5,
                )
                .help("How far"),
            )
            .parameter(Parameter::new(
                "count",
                "Count",
                ValueKind::integer(1, 64),
                4_i64,
            ))
            .parameter(
                Parameter::new(
                    "mode",
                    "Mode",
                    ValueKind::choice(vec![Choice::new("a", "A"), Choice::new("b", "B")]),
                    Value::Choice("b".into()),
                )
                .enabled(false),
            )
            .parameter(Parameter::new(
                "falloff",
                "Falloff",
                ValueKind::Ramp {
                    color: false,
                    most: None,
                },
                ramp::Ramp::default_of(false),
            ))
            .parameter(Parameter::new(
                "source",
                "Source",
                ValueKind::Reference(RefFilter::material()),
                Value::Text("/Materials/Red".into()),
            ))
            .parameter(Parameter::data(
                "sketch",
                "Sketch",
                kinogaki_studio_api::Typed::<f64>::new(kinogaki_studio_api::DataTypeId::declared(
                    "acme.cad.sketch",
                ))
                .wrap(0.0),
            ));
        for p in schema.parameters_mut() {
            p.group = "shape".into();
        }
        let sheet = Sheet::<String>::from(&schema);
        // A data parameter has no row.
        assert!(sheet.field(&"sketch".to_owned()).is_none());
        assert!(matches!(&sheet.items[0], Item::Band(s) if s.key == "shape" && s.open && !s.sub));
        let distance = sheet.field(&"distance".to_owned()).unwrap();
        assert_eq!(distance.value, Some(Value::Number(1.5)));
        assert_eq!(distance.tooltip.as_deref(), Some("How far"));
        let Kind::Number(n) = &distance.kind else {
            panic!("{:?}", distance.kind)
        };
        assert_eq!(
            (n.unit.clone(), n.min, n.max, n.soft),
            (Unit::Length, 0.0, 10.0, Some((0.0, 2.0)))
        );
        let count = sheet.field(&"count".to_owned()).unwrap();
        assert_eq!(count.value, Some(Value::Integer(4)));
        let Kind::Number(n) = &count.kind else {
            panic!()
        };
        assert_eq!(
            (count.look.style, n.min, n.max),
            (NumberStyle::Stepper, 1.0, 64.0)
        );
        let mode = sheet.field(&"mode".to_owned()).unwrap();
        assert!(!mode.enabled && mode.tooltip.is_none());
        assert_eq!(sheet.choice(&"mode".to_owned()), Some("b"));
        assert_eq!(
            sheet.field(&"falloff".to_owned()).unwrap().kind,
            Kind::Ramp {
                color: false,
                interpolation: None
            }
        );
        assert_eq!(
            sheet.field(&"source".to_owned()).unwrap().kind,
            Kind::Reference(RefFilter::material())
        );
    }

    /// The sheet follows the schema's one order: rows in no section first,
    /// then each section's band over the rows it shows, opening as its
    /// condition says; a section showing no row has no band.
    #[test]
    fn rows_show_in_the_schemas_order_with_bands_over_shown_rows_alone() {
        use kinogaki_studio_api::params::Section;
        use kinogaki_studio_api::value::Condition;
        let toggle = |name: &str, group: &str| Parameter::toggle(name, name, false).group(group);
        let schema = params::Schema::default()
            .section(Section::new("look", "Look", false).open_when(Condition::when("on", true)))
            .section(Section::new("hidden", "Hidden", true))
            .parameter(toggle("on", "look").help("Shows the look"))
            .parameter(toggle("ghost", "hidden").shown_when(Condition::when("on", true)))
            .parameter(Parameter::toggle("first", "First", true))
            .button(params::Button::new("go", "Go").group("look"));
        let sheet = Sheet::<String>::from(&schema);
        let listed: Vec<String> = (sheet.items.iter())
            .map(|item| match item {
                Item::Band(b) => format!("[{}{}]", b.label, if b.open { "" } else { " closed" }),
                Item::Field(f) => f.key.clone(),
                Item::Heading(h) => format!("{h:?}"),
            })
            .collect();
        assert_eq!(listed, ["first", "[Look closed]", "on", "go"]);
        let mut on = schema.clone();
        on.parameters_mut().find(|p| p.name == "on").unwrap().value = Value::Bool(true);
        let sheet = Sheet::<String>::from(&on);
        let bands: Vec<(&str, bool)> = (sheet.items.iter())
            .filter_map(|item| match item {
                Item::Band(b) => Some((&*b.label, b.open)),
                _ => None,
            })
            .collect();
        assert_eq!(bands, [("Look", true), ("Hidden", true)]);
    }

    #[test]
    fn curves_channels_and_texts_become_their_fields() {
        use kinogaki_studio_api::params::{CurveChannel as Channel, tone_curve};
        let curves = vec![tone_curve::ToneCurve::identity(); 2];
        let channels = vec![
            Channel {
                label: "Master".into(),
                tint: None,
            },
            Channel {
                label: "Red".into(),
                tint: Some([1.0, 0.0, 0.0]),
            },
        ];
        let schema = params::Schema::default()
            .parameter(Parameter::new(
                "curves",
                "Curves",
                ValueKind::ToneCurves { channels },
                Value::ToneCurves(curves.clone()),
            ))
            .parameter(Parameter::new(
                "gain",
                "Gain",
                ValueKind::Channels(params::Number::plain().range(0.0, 4.0)),
                vec![1.0; 4],
            ));
        for p in schema.parameters() {
            assert!(p.kind.accepts(&p.value), "{}", p.name);
        }
        let sheet = Sheet::<String>::from(&schema);
        let field = |name: &str| sheet.field(&name.to_owned()).unwrap();
        let curves_field = field("curves");
        assert_eq!(curves_field.value, Some(Value::ToneCurves(curves)));
        let Kind::ToneCurves { channels } = &curves_field.kind else {
            panic!("{:?}", curves_field.kind)
        };
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[1].tint, Some([1.0, 0.0, 0.0]));
        assert!(matches!(
            field("gain").kind,
            Kind::Channels { linked: false, .. }
        ));
    }

    /// What a node type's `schema` fills in shows: an attribute menu, a
    /// wire menu over the integers, a choice and a button.
    #[test]
    fn shaped_rows_show_their_menus_and_buttons() {
        let abc = vec![Choice::new("a", "A"), Choice::new("b", "B")];
        let schema = params::Schema::default()
            .parameter(Parameter::new(
                "attr",
                "Attribute",
                ValueKind::Attribute {
                    menu: vec!["P".into(), "N".into()],
                    list: true,
                },
                "P",
            ))
            .parameter(Parameter::new(
                "index",
                "Index",
                ValueKind::Integer {
                    min: 0,
                    max: 1,
                    menu: vec!["Box 1".into(), "Tube 1".into()],
                },
                1_i64,
            ))
            .parameter(Parameter::new(
                "mode",
                "Mode",
                ValueKind::choice(abc),
                Value::Choice("a".into()),
            ))
            .parameter(Parameter::new("group", "Group", ValueKind::Group, "0-"))
            .button(params::Button::new("fit", "Fit").enabled(false));
        let sheet = Sheet::<String>::from(&schema);
        let field = |name: &str| sheet.field(&name.to_owned()).unwrap();
        assert!(
            matches!(&field("attr").kind, Kind::Name { names, list: true, .. } if names.len() == 2)
        );
        assert!(matches!(&field("index").kind, Kind::Dropdown { items, .. } if items.len() == 2));
        assert_eq!(sheet.index(&"index".to_owned()), Some(1));
        let Kind::Choice { choices } = &field("mode").kind else {
            panic!()
        };
        assert!(crate::sheet::segments(choices), "two short ones: segments");
        assert!(matches!(&field("fit").kind, Kind::Button { .. }) && !field("fit").enabled);
    }

    /// A row shows and applies as its conditions say of the rows' values.
    #[test]
    fn conditions_hide_and_disable_rows() {
        use kinogaki_studio_api::value::Condition;
        let rows = |flat: bool| {
            params::Schema::default()
                .parameter(Parameter::toggle("flat", "Flat", flat))
                .parameter(
                    Parameter::toggle("glow", "Glow", false)
                        .shown_when(Condition::default().except("flat", true)),
                )
                .parameter(
                    Parameter::toggle("tint", "Tint", false)
                        .requires(Condition::when("flat", false), "Flat shows no tint"),
                )
        };
        let sheet = Sheet::<String>::from(&rows(true));
        assert!(sheet.field(&"glow".to_owned()).is_none());
        assert!(!sheet.field(&"tint".to_owned()).unwrap().enabled);
        let sheet = Sheet::<String>::from(&rows(false));
        assert!(sheet.field(&"glow".to_owned()).is_some());
        assert!(sheet.field(&"tint".to_owned()).unwrap().enabled);
    }
}
