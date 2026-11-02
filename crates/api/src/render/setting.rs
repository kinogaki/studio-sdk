//! Render settings as parameters: an engine's setting (or a declared
//! prim's value) seen as a [`Parameter`] of its kind, its value as a
//! parameter [`Value`] and back. A host keys settings over time as it keys
//! node parameters, through one definition of what each kind holds.
//!
//! | Setting | Parameter | Value |
//! |---|---|---|
//! | Bool | Toggle | `Bool` |
//! | Int | Integer (`min..=max`) | `Number` |
//! | Float | Number (limits, soft span) | `Number` |
//! | Vector2 | Vector of 2 | `Numbers` (2) |
//! | Color | Color | `Numbers` (3) |
//! | Choice | Choice (labels) | `Index`, by the descriptor's order |
//! | ColorRamp | Ramp (color) | `Numbers`: [`ramp::encode`] at [`MAX_RAMP_STOPS`] keys |
//! | Image | File | `Text`: the file's path |
use crate::params::{Number, Parameter, ParameterKind, Unit, Value, ramp};
use kinogaki_render_api::settings::MAX_RAMP_STOPS;
use kinogaki_render_api::{SettingDescriptor, SettingKind, SettingValue};
use std::sync::Arc;

/// Setting `d` as a parameter at its default, animatable as `d` says.
pub fn setting_parameter(d: &SettingDescriptor) -> Option<Parameter> {
    let kind = match &d.kind {
        SettingKind::Bool { .. } => ParameterKind::Toggle,
        &SettingKind::Int { min, max, .. } => ParameterKind::Integer { min, max },
        SettingKind::Float {
            min,
            max,
            soft,
            decimals,
            unit,
            slider,
            step,
            ..
        } => {
            let mut number = Number::new(self::unit(unit))
                .range(*min, *max)
                .soft(soft.0, soft.1)
                .decimals(*decimals)
                .speed(*step);
            if *slider {
                number = number.slider();
            }
            ParameterKind::Number(number)
        }
        SettingKind::Vector2 {
            min,
            max,
            unit,
            step,
            ..
        } => ParameterKind::Vector {
            components: 2,
            number: Number::new(self::unit(unit)).range(*min, *max).speed(*step),
        },
        SettingKind::Color { .. } => ParameterKind::Color,
        SettingKind::Choice { choices, .. } => {
            ParameterKind::Choice(choices.iter().map(|c| c.label.to_string()).collect())
        }
        SettingKind::ColorRamp { .. } => ParameterKind::Ramp { color: true },
        SettingKind::Image => ParameterKind::File,
    };
    let value = setting_to_value(&d.default_value(), d)?;
    let p = Parameter::new(&*d.name, &*d.label, kind, value).animatable(d.animatable);
    Some(match &d.tooltip {
        Some(tip) => p.help(&**tip),
        None => p,
    })
}

/// What a setting's number measures, as a parameter's unit (a suffix is
/// a plain number: a parameter's suffix is static text).
fn unit(unit: &kinogaki_render_api::Unit) -> Unit {
    match unit {
        kinogaki_render_api::Unit::Length => Unit::Length,
        kinogaki_render_api::Unit::Angle => Unit::Angle,
        kinogaki_render_api::Unit::None | kinogaki_render_api::Unit::Suffix(_) => Unit::Plain,
    }
}

/// Setting value `v` of setting `d` as a parameter's value; `None` for a
/// sent texture, or a value of another kind than `d`'s (a choice it
/// lacks).
pub fn setting_to_value(v: &SettingValue, d: &SettingDescriptor) -> Option<Value> {
    Some(match (v, &d.kind) {
        (SettingValue::Bool(on), SettingKind::Bool { .. }) => Value::Bool(*on),
        (SettingValue::Int(n), SettingKind::Int { .. }) => Value::Number(*n as f64),
        (SettingValue::Float(x), SettingKind::Float { .. }) => Value::Number(*x),
        (SettingValue::Vector2(v), SettingKind::Vector2 { .. }) => Value::Numbers(v.to_vec()),
        (SettingValue::Color(c), SettingKind::Color { .. }) => Value::Numbers(c.to_vec()),
        (SettingValue::Choice(name), SettingKind::Choice { choices, .. }) => {
            Value::Index(choices.iter().position(|c| c.name == *name)?)
        }
        (SettingValue::ColorRamp(keys), SettingKind::ColorRamp { .. }) => {
            let ramp = ramp::Ramp {
                keys: keys.to_vec(),
                color: true,
            };
            Value::Numbers(ramp::encode(&ramp, MAX_RAMP_STOPS))
        }
        (SettingValue::File(path), SettingKind::Image) => Value::Text(path.to_string()),
        _ => return None,
    })
}

/// Parameter value `v` as setting `d`'s value, when `d` takes it
/// ([`SettingDescriptor::accepts`]): an integer's number rounded, a
/// choice's index named, a ramp's numbers decoded. A color's channels
/// below 0 read as 0 (a color parameter has no limits, a setting's color
/// is not negative).
pub fn setting_from_value(v: &Value, d: &SettingDescriptor) -> Option<SettingValue> {
    let value = match (&d.kind, v) {
        (SettingKind::Bool { .. }, Value::Bool(on)) => SettingValue::Bool(*on),
        (SettingKind::Int { .. }, Value::Number(n)) if n.is_finite() => {
            SettingValue::Int(n.round() as i64)
        }
        (SettingKind::Float { .. }, Value::Number(x)) => SettingValue::Float(*x),
        (SettingKind::Vector2 { .. }, Value::Numbers(n)) => {
            SettingValue::Vector2(<[f64; 2]>::try_from(n.as_slice()).ok()?)
        }
        (SettingKind::Color { .. }, Value::Numbers(n)) => {
            let c = <[f64; 3]>::try_from(n.as_slice()).ok()?;
            SettingValue::Color(c.map(|c| c.max(0.0)))
        }
        (SettingKind::Choice { choices, .. }, Value::Index(i)) => {
            SettingValue::Choice(choices.get(*i)?.name.clone())
        }
        (SettingKind::ColorRamp { .. }, Value::Numbers(n)) => {
            SettingValue::ColorRamp(Arc::from(ramp::decode(n, true).keys))
        }
        (SettingKind::Image, Value::Text(path)) => SettingValue::File(Arc::from(&**path)),
        _ => return None,
    };
    d.accepts(&value).then_some(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use kinogaki_render_api::ramp::{Interpolation, Key};
    use kinogaki_render_api::{Choice, SettingKind};

    fn descriptor(kind: SettingKind) -> SettingDescriptor {
        SettingDescriptor::new("s", "S", "main", kind)
    }

    /// One descriptor of each kind, and a value of it other than its
    /// default.
    fn kinds() -> Vec<(SettingDescriptor, SettingValue)> {
        let ramp: Arc<[Key]> = Arc::from([
            Key::color(0.0, [0.1, 0.2, 0.3], Interpolation::Constant),
            Key::color(0.4, [1.0, 0.5, 0.0], Interpolation::Monotone),
            Key::color(1.0, [0.0, 0.0, 2.5], Interpolation::BSpline),
        ]);
        let float = SettingKind::Float {
            default: 1.0,
            min: -2.0,
            max: 8.0,
            soft: (0.0, 4.0),
            decimals: 3,
            unit: kinogaki_render_api::Unit::Suffix(Arc::from(" EV")),
            slider: true,
            step: 0.01,
        };
        let choices = vec![Choice::new("a", "A"), Choice::new("b", "B")];
        vec![
            (
                descriptor(SettingKind::Bool { default: false }),
                SettingValue::Bool(true),
            ),
            (
                descriptor(SettingKind::Int {
                    default: 4,
                    min: 1,
                    max: 64,
                }),
                SettingValue::Int(17),
            ),
            (descriptor(float), SettingValue::Float(2.375)),
            (
                descriptor(SettingKind::Vector2 {
                    default: [0.0; 2],
                    min: -1.0,
                    max: 1.0,
                    unit: kinogaki_render_api::Unit::Length,
                    step: 0.01,
                }),
                SettingValue::Vector2([0.25, -0.5]),
            ),
            (
                descriptor(SettingKind::Color { default: [1.0; 3] }),
                SettingValue::Color([0.2, 3.0, 0.0]),
            ),
            (
                descriptor(SettingKind::Choice {
                    choices,
                    default: Arc::from("a"),
                    menu: false,
                }),
                SettingValue::Choice(Arc::from("b")),
            ),
            (
                descriptor(SettingKind::ColorRamp {
                    default: Arc::from([Key::color(0.0, [1.0; 3], Interpolation::Linear)]),
                }),
                SettingValue::ColorRamp(ramp),
            ),
        ]
    }

    #[test]
    fn every_kind_round_trips_exactly() {
        for (d, v) in kinds() {
            let p = setting_parameter(&d).expect("a parameter");
            assert!(p.animatable, "{:?}", d.kind);
            assert_eq!(p.check(&p.value), Ok(()), "{:?}", d.kind);
            for v in [d.default_value(), v] {
                let value = setting_to_value(&v, &d).expect("a value");
                assert_eq!(p.check(&value), Ok(()), "{v:?}");
                assert_eq!(setting_from_value(&value, &d), Some(v));
            }
        }
    }

    #[test]
    fn kinds_map_to_their_parameters() {
        let p: Vec<ParameterKind> = (kinds().iter())
            .map(|(d, _)| setting_parameter(d).unwrap().kind)
            .collect();
        assert_eq!(p[0], ParameterKind::Toggle);
        assert_eq!(p[1], ParameterKind::Integer { min: 1, max: 64 });
        let ParameterKind::Number(n) = &p[2] else {
            panic!("{:?}", p[2]);
        };
        assert_eq!(
            (n.min, n.max, n.soft, n.slider),
            (-2.0, 8.0, Some((0.0, 4.0)), true)
        );
        assert!(matches!(p[3], ParameterKind::Vector { components: 2, .. }));
        assert_eq!(p[4], ParameterKind::Color);
        assert_eq!(p[5], ParameterKind::Choice(vec!["A".into(), "B".into()]));
        assert_eq!(p[6], ParameterKind::Ramp { color: true });
    }

    #[test]
    fn an_image_is_its_file_and_values_a_setting_lacks_have_none() {
        let image = descriptor(SettingKind::Image);
        let p = setting_parameter(&image).unwrap();
        assert_eq!(p.kind, ParameterKind::File);
        assert!(!p.animatable && !image.animatable);
        assert_eq!(p.value, Value::Text(String::new()));
        let sky = SettingValue::File(Arc::from("sky/noon.exr"));
        let value = setting_to_value(&sky, &image).unwrap();
        assert_eq!(value, Value::Text("sky/noon.exr".into()));
        assert_eq!(setting_from_value(&value, &image), Some(sky));
        // What an engine receives is no parameter's.
        let sent = SettingValue::Texture(None);
        assert!(image.accepts(&sent));
        assert_eq!(setting_to_value(&sent, &image), None);
        let (choice, _) = kinds().remove(5);
        let missing = SettingValue::Choice(Arc::from("c"));
        assert_eq!(setting_to_value(&missing, &choice), None);
        assert_eq!(setting_from_value(&Value::Index(2), &choice), None);
        let (int, _) = kinds().remove(1);
        assert_eq!(setting_from_value(&Value::Number(65.0), &int), None);
        assert_eq!(setting_from_value(&Value::Bool(true), &int), None);
        let (color, _) = kinds().remove(4);
        let below = Value::Numbers(vec![-0.5, 1.0, 2.0]);
        let clamped = SettingValue::Color([0.0, 1.0, 2.0]);
        assert_eq!(setting_from_value(&below, &color), Some(clamped));
    }

    #[test]
    fn an_engine_turns_animation_off() {
        let (d, _) = kinds().remove(1);
        let p = setting_parameter(&d.animatable(false)).unwrap();
        assert!(!p.animatable);
    }
}
