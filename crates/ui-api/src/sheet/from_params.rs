//! A node's or format's parameter [`params::Schema`] as a sheet, keyed by
//! parameter name: what the host shows for a node's parameters.
use super::{Field, Item, Kind, Number, NumberStyle, Schema, Section};
use kinogaki_studio_api::params::{self, ParameterKind, SchemaItem, Unit};

/// The interpolation codes' names of a parameter ramp (its encoding's 0,
/// 1 and 2).
pub const RAMP_INTERPOLATIONS: [&str; 3] = ["Constant", "Linear", "Smooth"];

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
                SchemaItem::Parameter(p) => {
                    let mut field = Field::new(
                        p.name.clone(),
                        p.label.clone(),
                        Kind::from(&p.kind),
                        Some(p.value.clone()),
                    );
                    field.enabled = p.enabled;
                    field.tooltip = (!p.help.is_empty()).then(|| p.help.clone().into());
                    field.keywords = p.keywords.clone().into();
                    Item::Field(field)
                }
            })
            .collect();
        Schema { items }
    }
}

/// How a parameter number shows: its unit's defaults, its limits and look.
impl From<&params::Number> for Number {
    fn from(number: &params::Number) -> Self {
        let base = match number.unit {
            Unit::Plain => Number::new(),
            Unit::Length => Number::length(),
            Unit::Angle => Number::angle(),
            Unit::Percent => Number::percent(),
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
            ParameterKind::Choice(items) => {
                Kind::Choice(items.iter().map(|i| i.clone().into()).collect())
            }
            ParameterKind::Text => Kind::Text,
            ParameterKind::Code => Kind::Code {
                diagnostics: Vec::new(),
                fill: false,
                lines: None,
            },
            ParameterKind::File => Kind::Path,
            ParameterKind::Color => Kind::Color,
            ParameterKind::Vector { number, .. } => Kind::Vector {
                number: number.into(),
                lock: None,
            },
            ParameterKind::Ramp { min, max } => Kind::Ramp {
                min: *min,
                max: *max,
                curve: Vec::new(),
                interpolations: RAMP_INTERPOLATIONS.iter().map(|&n| n.into()).collect(),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kinogaki_studio_api::params::{Parameter, Value};

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
                ParameterKind::Ramp { min: 0.0, max: 1.0 },
                vec![2.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0],
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
        let Kind::Ramp { interpolations, .. } = &sheet.field(&"falloff".to_owned()).unwrap().kind
        else {
            panic!()
        };
        assert_eq!(interpolations.len(), 3);
    }
}
