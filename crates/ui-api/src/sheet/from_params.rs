//! A node's or format's parameter [`params::Schema`] as a sheet, keyed by
//! parameter name: what the host shows for a node's parameters.
use super::{CurveChannel, Field, FileDialog, Item, Kind, Number, NumberStyle, Schema, Section};
use kinogaki_studio_api::params::{self, ParameterKind, SchemaItem, Unit};

/// The sheet of a parameter schema: its sections as top-level sections,
/// each parameter a field keyed by its name, with its help as the tooltip
/// and its keywords for search.
impl From<&params::Schema> for Schema<String> {
    fn from(schema: &params::Schema) -> Self {
        let items = schema
            .items
            .iter()
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

/// Parameter `p`'s field: its kind as [`kind`] shapes it, its help as
/// the tooltip (or its problem, shown as an error), its keywords for
/// search.
fn field(p: &params::Parameter) -> Field<String> {
    let value = match (&p.value, &p.kind) {
        // Texts show as one read-only line of their names.
        (params::Value::Texts(names), _) => params::Value::Text(names.join(", ")),
        // A wire chosen from a menu shows as the menu's index.
        (params::Value::Number(at), ParameterKind::Integer { .. }) if !p.menu.is_empty() => {
            params::Value::Index((at.max(0.0) as usize).min(p.menu.len() - 1))
        }
        (other, _) => other.clone(),
    };
    let mut field = Field::new(p.name.clone(), p.label.clone(), kind(p), Some(value));
    field.enabled = p.enabled;
    field.tooltip = (!p.help.is_empty()).then(|| p.help.clone().into());
    field.keywords = p.keywords.clone().into();
    if let Some(problem) = &p.problem {
        field.error = true;
        field.tooltip = Some(problem.clone().into());
    }
    field.read_only = p.read_only;
    field
}

/// Parameter `p`'s field kind: its kind's, with what its schema filled
/// in (an attribute row's menu, a wire menu, unavailable choices).
fn kind(p: &params::Parameter) -> Kind {
    match &p.kind {
        ParameterKind::Choice(items) => choice(items, &p.unavailable),
        ParameterKind::Integer { .. } if !p.menu.is_empty() => Kind::Dropdown {
            items: p.menu.iter().map(|n| n.clone().into()).collect(),
            available: vec![true; p.menu.len()],
            info: None,
        },
        ParameterKind::Attribute => Kind::Name {
            names: p.menu.clone(),
            list: p.list,
            tooltip: "The input's attributes".into(),
        },
        other => Kind::from(other),
    }
}

/// A choice of `items`, those at `unavailable` greyed: segments or a menu
/// ([`params::segments`]).
fn choice(items: &[String], unavailable: &[usize]) -> Kind {
    let names = items.iter().map(|i| i.clone().into()).collect();
    if params::segments(items, unavailable) {
        return Kind::Choice(names);
    }
    Kind::Dropdown {
        items: names,
        available: (0..items.len())
            .map(|i| !unavailable.contains(&i))
            .collect(),
        info: None,
    }
}

/// How a parameter number shows: its unit's defaults, its limits and look.
impl From<&params::Number> for Number {
    fn from(number: &params::Number) -> Self {
        let base = match number.unit {
            Unit::Plain => Number::new(),
            Unit::Length => Number::length(),
            Unit::Millimeters => Number::new().unit(Unit::Millimeters),
            Unit::Angle => Number::angle(),
            Unit::Percent => Number::percent(),
            Unit::Suffix(_) => Number::new().unit(number.unit),
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

/// The field kind a parameter kind shows as.
impl From<&ParameterKind> for Kind {
    fn from(kind: &ParameterKind) -> Self {
        match kind {
            ParameterKind::Number(n) => Kind::Number(n.into()),
            ParameterKind::Integer { min, max } => {
                Kind::Number(Number::integer(*min as f64..=*max as f64))
            }
            ParameterKind::Toggle => Kind::Toggle,
            ParameterKind::Choice(items) => choice(items, &[]),
            ParameterKind::Text => Kind::Text,
            ParameterKind::Code => Kind::Code {
                diagnostics: Vec::new(),
                fill: false,
                lines: None,
            },
            ParameterKind::File => Kind::Path(FileDialog::open()),
            // The host fills the pickers and menus from the scene and the
            // node's input.
            ParameterKind::Reference(filter) => Kind::Reference(filter.clone()),
            ParameterKind::Path => Kind::TreePath {
                choices: Vec::new(),
                relative: false,
                tooltip: "Choose a node or object".into(),
            },
            ParameterKind::Attribute => Kind::Name {
                names: Vec::new(),
                list: false,
                tooltip: "The input's attributes".into(),
            },
            ParameterKind::Group => Kind::Expression { pick: None },
            ParameterKind::Color => Kind::Color,
            ParameterKind::Vector { number, .. } => Kind::Vector {
                number: number.into(),
                lock: None,
            },
            ParameterKind::Ramp { color } => Kind::Ramp {
                color: *color,
                interpolation: None,
            },
            ParameterKind::ToneCurves { channels, capacity } => Kind::ToneCurves {
                channels: channels
                    .iter()
                    .map(|c| CurveChannel {
                        label: c.label.clone().into(),
                        tint: c.tint,
                    })
                    .collect(),
                capacity: *capacity,
            },
            ParameterKind::Palette { capacity } => Kind::Palette {
                capacity: *capacity,
            },
            // Apart until the host says the node keeps them linked (its
            // view, not the value).
            ParameterKind::Channels(number) => Kind::Channels {
                number: number.into(),
                linked: false,
            },
            ParameterKind::Texts => Kind::Info,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sheet::ramp;
    use kinogaki_studio_api::params::{Parameter, RefFilter, Value};

    #[test]
    fn parameters_become_fields_keyed_by_name() {
        let schema = params::Schema::default()
            .section("shape", "Shape", true)
            .parameter(
                Parameter::new(
                    "distance",
                    "Distance",
                    ParameterKind::Number(params::Number::length().range(0.0, 10.0).soft(0.0, 2.0)),
                    1.5,
                )
                .help("How far"),
            )
            .parameter(Parameter::new(
                "count",
                "Count",
                ParameterKind::Integer { min: 1, max: 64 },
                4.0,
            ))
            .parameter(
                Parameter::new(
                    "mode",
                    "Mode",
                    ParameterKind::Choice(vec!["A".into(), "B".into()]),
                    Value::Index(1),
                )
                .enabled(false),
            )
            .parameter(Parameter::new(
                "falloff",
                "Falloff",
                ParameterKind::Ramp { color: false },
                ramp::encode(&ramp::Ramp::default_of(false), 4),
            ))
            .parameter(Parameter::new(
                "source",
                "Source",
                ParameterKind::Reference(RefFilter::material(false)),
                Value::Text("/Materials/Red".into()),
            ));
        let sheet = Schema::<String>::from(&schema);
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
            (n.unit, n.range.clone(), n.soft.clone()),
            (Unit::Length, 0.0..=10.0, Some(0.0..=2.0))
        );
        let Kind::Number(count) = &sheet.field(&"count".to_owned()).unwrap().kind else {
            panic!()
        };
        assert_eq!(
            (count.style, count.range.clone()),
            (NumberStyle::Stepper, 1.0..=64.0)
        );
        let mode = sheet.field(&"mode".to_owned()).unwrap();
        assert!(!mode.enabled && mode.tooltip.is_none());
        assert_eq!(sheet.index(&"mode".to_owned()), Some(1));
        assert_eq!(
            sheet.field(&"falloff".to_owned()).unwrap().kind,
            Kind::Ramp {
                color: false,
                interpolation: None
            }
        );
        assert_eq!(
            sheet.field(&"source".to_owned()).unwrap().kind,
            Kind::Reference(RefFilter::material(false))
        );
    }

    #[test]
    fn curves_palettes_channels_and_texts_become_their_fields() {
        use kinogaki_studio_api::params::{CurveChannel as Channel, tone_curve};
        let curves = tone_curve::encode(&vec![tone_curve::ToneCurve::identity(); 2], 8);
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
                ParameterKind::ToneCurves {
                    channels,
                    capacity: 8,
                },
                curves.clone(),
            ))
            .parameter(Parameter::new(
                "palette",
                "Palette",
                ParameterKind::Palette { capacity: 16 },
                vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
            ))
            .parameter(Parameter::new(
                "gain",
                "Gain",
                ParameterKind::Channels(params::Number::plain().range(0.0, 4.0)),
                vec![1.0; 4],
            ))
            .parameter(Parameter::new(
                "outputs",
                "Outputs",
                ParameterKind::Texts,
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
        assert_eq!(curves_field.value, Some(Value::Numbers(curves)));
        let Kind::ToneCurves { channels, capacity } = &curves_field.kind else {
            panic!("{:?}", curves_field.kind)
        };
        assert_eq!((channels.len(), *capacity), (2, 8));
        assert_eq!(channels[1].tint, Some([1.0, 0.0, 0.0]));
        assert!(matches!(
            field("palette").kind,
            Kind::Palette { capacity: 16 }
        ));
        assert!(matches!(
            field("gain").kind,
            Kind::Channels { linked: false, .. }
        ));
        let outputs = field("outputs");
        assert!(matches!(outputs.kind, Kind::Info));
        assert_eq!(outputs.value, Some(Value::Text("beauty, depth".into())));
        // A palette past its capacity, or three channels, isn't accepted.
        assert!(!ParameterKind::Palette { capacity: 1 }.accepts(&Value::Numbers(vec![0.0; 6])));
        assert!(
            !ParameterKind::Channels(params::Number::plain())
                .accepts(&Value::Numbers(vec![0.0; 3]))
        );
    }

    /// What a node type's `schema` fills in shows: an attribute menu, a
    /// wire menu over the indices, greyed choices as a menu, a problem as
    /// an error, and a button.
    #[test]
    fn shaped_rows_show_their_menus_problems_and_buttons() {
        let schema = params::Schema::default()
            .parameter(
                Parameter::new("attr", "Attribute", ParameterKind::Attribute, "P")
                    .menu(vec!["P".into(), "N".into()], true),
            )
            .parameter(
                Parameter::new(
                    "index",
                    "Index",
                    ParameterKind::Integer { min: 0, max: 9 },
                    5.0,
                )
                .menu(vec!["Box 1".into(), "Tube 1".into()], false),
            )
            .parameter(
                Parameter::new(
                    "mode",
                    "Mode",
                    ParameterKind::Choice(vec!["A".into(), "B".into()]),
                    params::Value::Index(0),
                )
                .unavailable(vec![1]),
            )
            .parameter(
                Parameter::new("group", "Group", ParameterKind::Group, "0-")
                    .problem("expected a number"),
            )
            .button(params::Button::new("fit", "Fit").enabled(false));
        let sheet = Schema::<String>::from(&schema);
        let field = |name: &str| sheet.field(&name.to_owned()).unwrap();
        assert!(
            matches!(&field("attr").kind, Kind::Name { names, list: true, .. } if names.len() == 2)
        );
        assert!(matches!(&field("index").kind, Kind::Dropdown { items, .. } if items.len() == 2));
        assert_eq!(field("index").value, Some(Value::Index(1)));
        assert!(
            matches!(&field("mode").kind, Kind::Dropdown { available, .. } if available == &[true, false])
        );
        assert!(field("group").error);
        assert!(matches!(&field("fit").kind, Kind::Button { .. }) && !field("fit").enabled);
    }

    /// Read-only parameters are read-only fields, enabled: the host draws
    /// them at full contrast, taking no input.
    #[test]
    fn read_only_rows_show_without_editing() {
        let schema = params::Schema::default()
            .parameter(
                Parameter::new(
                    "palette",
                    "Palette",
                    ParameterKind::Palette { capacity: 4 },
                    vec![0.1, 0.2, 0.3],
                )
                .read_only(true),
            )
            .parameter(
                Parameter::new(
                    "amount",
                    "Amount",
                    ParameterKind::Number(params::Number::plain()),
                    1.0,
                )
                .read_only(true),
            );
        let sheet = Schema::<String>::from(&schema);
        for key in ["palette", "amount"] {
            let field = sheet.field(&key.to_owned()).unwrap();
            assert!(field.enabled && field.read_only, "{key}");
        }
    }
}
