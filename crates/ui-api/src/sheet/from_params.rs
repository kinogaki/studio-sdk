//! A node's or format's parameter [`params::Schema`] as a sheet, keyed by
//! parameter name: what the host shows for a node's parameters. A field
//! holds its parameter's value as it is, so an edit is the parameter's new
//! value.
use super::{Field, FileDialog, Item, Kind, Number, NumberStyle, Schema, Section};
use kinogaki_studio_api::params::{self, SchemaItem, Unit, Value, ValueKind};

/// The sheet of a parameter schema: its sections as top-level sections,
/// each parameter a field keyed by its name, with its help as the tooltip
/// and its keywords for search. A data parameter has no field: its node's
/// schema adds rows of its own.
impl From<&params::Schema> for Schema<String> {
    fn from(schema: &params::Schema) -> Self {
        let data = |p: &params::Parameter| matches!(p.kind, ValueKind::Data(_));
        let items = (schema.items.iter())
            .filter(|item| !matches!(item, SchemaItem::Parameter(p) if data(p)))
            .map(|item| match item {
                SchemaItem::Section(s) => Item::Section(Section {
                    key: s.key.clone().into(),
                    label: s.label.clone().into(),
                    open: s.open,
                    sub: false,
                }),
                SchemaItem::Parameter(p) => Item::Field(field(p)),
                SchemaItem::Button(b) => {
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
            })
            .collect();
        Schema { items }
    }
}

/// Parameter `p`'s field: its kind's, its help as the tooltip (or its
/// problem, shown as an error), its keywords for search.
fn field(p: &params::Parameter) -> Field<String> {
    let value = match &p.value {
        // Texts show as one read-only line of their names.
        Value::Texts(names) => Value::Text(names.join(", ")),
        other => other.clone(),
    };
    let mut field = Field::new(
        p.name.clone(),
        p.label.clone(),
        Kind::from(&p.kind),
        Some(value),
    );
    field.enabled = p.enabled;
    field.tooltip = (!p.help.is_empty()).then(|| p.help.clone().into());
    field.keywords = p.keywords.clone().into();
    if let Some(problem) = &p.problem {
        field.error = true;
        field.tooltip = Some(problem.clone().into());
    }
    field
}

/// How a parameter number shows: its unit's defaults, its limits and look.
impl From<&params::Number> for Number {
    fn from(number: &params::Number) -> Self {
        let base = match number.unit {
            Unit::Plain => Number::new(),
            Unit::Length => Number::length(),
            Unit::Angle => Number::angle(),
            Unit::Percent => Number::percent(),
            Unit::Millimeters | Unit::Suffix(_) => Number::new().unit(number.unit.clone()),
        };
        let mut out = base.range(number.min..=number.max);
        if let Some((min, max)) = number.soft {
            out = out.soft(min..=max);
        }
        if number.slider {
            out.style = NumberStyle::Slider;
        }
        if let Some(places) = number.decimals {
            out.decimals = places.into();
        }
        if let Some(speed) = number.speed {
            out.speed = speed;
        }
        out
    }
}

/// The field kind a value kind shows as, with its options: a choice's
/// entries, an attribute row's names, an integer's menu over its values
/// from 0.
impl From<&ValueKind> for Kind {
    fn from(kind: &ValueKind) -> Self {
        match kind {
            ValueKind::Number(n) => Kind::Number(n.into()),
            ValueKind::Integer { menu, .. } if !menu.is_empty() => Kind::Dropdown {
                items: menu.iter().map(|n| n.clone().into()).collect(),
                available: vec![true; menu.len()],
                info: None,
            },
            ValueKind::Integer { min, max, .. } => {
                Kind::Number(Number::integer(*min as f64..=*max as f64))
            }
            ValueKind::Toggle => Kind::Toggle,
            ValueKind::Choice {
                choices,
                unavailable,
            } => Kind::Choice {
                choices: choices.clone(),
                unavailable: unavailable.clone(),
            },
            ValueKind::Text => Kind::Text,
            ValueKind::Code { .. } => Kind::Code {
                diagnostics: Vec::new(),
                fill: false,
                lines: None,
            },
            ValueKind::File => Kind::Path(FileDialog::open()),
            // The host fills the pickers and menus from the scene and the
            // node's input.
            ValueKind::Reference(filter) => Kind::Reference(filter.clone()),
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
            ValueKind::Vector { number, .. } => Kind::Vector {
                number: number.into(),
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
                number: number.into(),
                linked: false,
            },
            // Never a field (see the schema's sheet); read-only text if
            // one is made of it alone.
            ValueKind::Texts | ValueKind::Data(_) => Kind::Info,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet::ramp;
    use kinogaki_studio_api::params::{Choice, Parameter, RefFilter};

    #[test]
    fn parameters_become_fields_keyed_by_name() {
        let schema = params::Schema::default()
            .section("shape", "Shape", true)
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
        let sheet = Schema::<String>::from(&schema);
        // A data parameter has no row.
        assert!(sheet.field(&"sketch".to_owned()).is_none());
        assert!(
            matches!(&sheet.items[0], Item::Section(s) if s.key == "shape" && s.open && !s.sub)
        );
        let distance = sheet.field(&"distance".to_owned()).unwrap();
        assert_eq!(distance.value, Some(Value::Number(1.5)));
        assert_eq!(distance.tooltip.as_deref(), Some("How far"));
        let Kind::Number(n) = &distance.kind else {
            panic!("{:?}", distance.kind)
        };
        assert_eq!(
            (n.unit.clone(), n.range.clone(), n.soft.clone()),
            (Unit::Length, 0.0..=10.0, Some(0.0..=2.0))
        );
        let count = sheet.field(&"count".to_owned()).unwrap();
        assert_eq!(count.value, Some(Value::Integer(4)));
        let Kind::Number(count) = &count.kind else {
            panic!()
        };
        assert_eq!(
            (count.style, count.range.clone()),
            (NumberStyle::Stepper, 1.0..=64.0)
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
            ))
            .parameter(Parameter::new(
                "outputs",
                "Outputs",
                ValueKind::Texts,
                vec!["beauty".to_owned(), "depth".to_owned()],
            ));
        for item in &schema.items {
            let SchemaItem::Parameter(p) = item else {
                continue;
            };
            assert!(p.kind.accepts(&p.value), "{}", p.name);
        }
        let sheet = Schema::<String>::from(&schema);
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
        let outputs = field("outputs");
        assert!(matches!(outputs.kind, Kind::Info));
        assert_eq!(outputs.value, Some(Value::Text("beauty, depth".into())));
    }

    /// What a node type's `schema` fills in shows: an attribute menu, a
    /// wire menu over the integers, greyed choices, a problem as an error,
    /// and a button.
    #[test]
    fn shaped_rows_show_their_menus_problems_and_buttons() {
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
                ValueKind::Choice {
                    choices: abc,
                    unavailable: vec!["b".into()],
                },
                Value::Choice("a".into()),
            ))
            .parameter(
                Parameter::new("group", "Group", ValueKind::Group, "0-")
                    .problem("expected a number"),
            )
            .button(params::Button::new("fit", "Fit").enabled(false));
        let sheet = Schema::<String>::from(&schema);
        let field = |name: &str| sheet.field(&name.to_owned()).unwrap();
        assert!(
            matches!(&field("attr").kind, Kind::Name { names, list: true, .. } if names.len() == 2)
        );
        assert!(matches!(&field("index").kind, Kind::Dropdown { items, .. } if items.len() == 2));
        assert_eq!(sheet.index(&"index".to_owned()), Some(1));
        let Kind::Choice {
            choices,
            unavailable,
        } = &field("mode").kind
        else {
            panic!()
        };
        assert!(
            !crate::sheet::segments(choices, unavailable),
            "a menu, one greyed"
        );
        assert!(field("group").error);
        assert!(matches!(&field("fit").kind, Kind::Button { .. }) && !field("fit").enabled);
    }
}
