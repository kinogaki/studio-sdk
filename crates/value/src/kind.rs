//! [`ValueKind`]: what a value holds, its limits and what a host offers
//! for it (a choice's entries, an attribute row's names). Presentation (a
//! menu or segments, a remembered tool setting, a read-only row) is the UI
//! layer's, never a kind's.
use crate::id::DataTypeId;
use crate::reference::RefFilter;
use crate::value::Value;
use std::sync::Arc;

/// What a value holds and the options its kind takes.
#[derive(Clone, Debug, PartialEq)]
pub enum ValueKind {
    /// [`Value::Number`]: a real number with a unit and limits.
    Number(Number),
    /// [`Value::Integer`] in `min..=max`.
    Integer {
        /// The smallest value.
        min: i64,
        /// The largest value.
        max: i64,
        /// What each value from `min` up shows as, a menu over them (the
        /// nodes behind a node's wires, when it picks one); empty: none.
        menu: Vec<String>,
    },
    /// [`Value::Bool`].
    Toggle,
    /// [`Value::Choice`]: one of these, by key.
    Choice {
        /// The entries, in the order shown.
        choices: Vec<Choice>,
        /// The keys of entries that can't apply now: shown greyed.
        unavailable: Vec<Arc<str>>,
    },
    /// [`Value::Text`], one line, committed when the edit ends.
    Text,
    /// [`Value::Text`]: code, many lines in a code editor.
    Code {
        /// The language its text is in, as the host's code editor names it;
        /// `None`: plain text.
        language: Option<String>,
    },
    /// [`Value::Text`]: a file's path, as typed (empty: none). An image a
    /// render reads is sent as its texture, never as the path.
    File,
    /// [`Value::Text`]: a free path to any node or object of the scene.
    Path,
    /// [`Value::Text`]: a path to what the filter says fits; the host
    /// offers what fits and resolves the path from the node holding it.
    Reference(RefFilter),
    /// [`Value::Text`]: attribute names (a name or a pattern), the
    /// attributes of an input offered as a menu.
    Attribute {
        /// The names offered; empty: none known.
        menu: Vec<String>,
        /// Whether it holds several names, space-separated, so a menu pick
        /// adds one rather than replacing the text.
        list: bool,
    },
    /// [`Value::Text`]: a group, an expression over an input's components
    /// in the node's own syntax, which a pick in the viewport can write.
    Group,
    /// [`Value::Color`]: linear RGB, channels finite (a working space's
    /// colors outside a narrower space's gamut have negative channels).
    Color,
    /// [`Value::Numbers`]: `components` numbers (2 to 4) lettered X, Y,
    /// Z, W, each as `number`.
    Vector {
        /// How many components, 2 to 4.
        components: u8,
        /// How each component shows and its limits.
        number: Number,
    },
    /// [`Value::Numbers`] (4): R, G, B and A, each as `number`.
    Channels(Number),
    /// [`Value::Ramp`]: a float or a color ramp.
    Ramp {
        /// A color ramp rather than a float ramp.
        color: bool,
        /// The most keys taken (an engine's capacity); `None`: any number.
        /// A ramp with more is refused, never cut short.
        most: Option<usize>,
    },
    /// [`Value::ToneCurves`]: one curve a channel.
    ToneCurves {
        /// The channels, in curve order.
        channels: Vec<CurveChannel>,
    },
    /// [`Value::Texts`]: names a node keeps (the outputs it writes),
    /// listed, not edited.
    Texts,
    /// [`Value::Data`] of this registered data type: a record a node's
    /// tools author, never shown as a row and never animated.
    Data(DataTypeId),
}

/// A named choice: its key in files and values, its label in a UI.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Choice {
    /// Its key (`bead_blasted`): what [`Value::Choice`] holds and files
    /// save.
    pub key: Arc<str>,
    /// What a UI shows (`Bead Blasted`).
    pub label: Arc<str>,
}

impl Choice {
    /// The choice `key` labelled `label`.
    pub fn new(key: &str, label: &str) -> Choice {
        Choice {
            key: Arc::from(key),
            label: Arc::from(label),
        }
    }
}

/// One channel of [`ValueKind::ToneCurves`]: its name and its curve's tint.
#[derive(Clone, Debug, PartialEq)]
pub struct CurveChannel {
    /// What its tab says ("Red").
    pub label: String,
    /// The curve's tint, linear RGB; `None` draws it in the text color.
    pub tint: Option<[f64; 3]>,
}

impl ValueKind {
    /// A choice of `choices`, none unavailable.
    pub fn choice(choices: Vec<Choice>) -> Self {
        Self::Choice {
            choices,
            unavailable: Vec::new(),
        }
    }

    /// An integer in `min..=max`, without a menu.
    pub fn integer(min: i64, max: i64) -> Self {
        Self::Integer {
            min,
            max,
            menu: Vec::new(),
        }
    }

    /// Attribute names, without a menu yet.
    pub fn attribute() -> Self {
        Self::Attribute {
            menu: Vec::new(),
            list: false,
        }
    }

    /// Whether a value of this kind can be keyed over time unless its
    /// owner says otherwise: numbers, integers, vectors, colors, channels,
    /// toggles and choices can; ramps and tone curves (whose keys come and
    /// go, so two of their values needn't blend), text of every kind and
    /// data can't.
    pub fn animatable(&self) -> bool {
        match self {
            Self::Number(_)
            | Self::Integer { .. }
            | Self::Vector { .. }
            | Self::Color
            | Self::Channels(_)
            | Self::Toggle
            | Self::Choice { .. } => true,
            Self::Ramp { .. }
            | Self::ToneCurves { .. }
            | Self::Text
            | Self::Code { .. }
            | Self::File
            | Self::Path
            | Self::Reference(_)
            | Self::Attribute { .. }
            | Self::Group
            | Self::Texts
            | Self::Data(_) => false,
        }
    }

    /// Whether `value` is of the variant this kind holds (of the right
    /// count, a choice it offers, a ramp of its kind, data of its type).
    /// Limits are [`check`](Self::check)'s.
    pub fn accepts(&self, value: &Value) -> bool {
        match (self, value) {
            (Self::Number(_), Value::Number(_)) => true,
            (Self::Integer { .. }, Value::Integer(_)) => true,
            (Self::Toggle, Value::Bool(_)) => true,
            (Self::Choice { choices, .. }, Value::Choice(key)) => {
                choices.iter().any(|c| c.key == *key)
            }
            (
                Self::Text
                | Self::Code { .. }
                | Self::File
                | Self::Path
                | Self::Reference(_)
                | Self::Attribute { .. }
                | Self::Group,
                Value::Text(_),
            ) => true,
            (Self::Color, Value::Color(_)) => true,
            (Self::Vector { components, .. }, Value::Numbers(v)) => v.len() == *components as usize,
            (Self::Channels(_), Value::Numbers(v)) => v.len() == 4,
            (Self::Ramp { color, .. }, Value::Ramp(r)) => r.color == *color,
            (Self::ToneCurves { channels }, Value::ToneCurves(c)) => c.len() == channels.len(),
            (Self::Texts, Value::Texts(_)) => true,
            (Self::Data(ty), Value::Data(data)) => data.ty() == *ty,
            _ => false,
        }
    }

    /// Whether `value` is one this kind takes: of its variant
    /// ([`accepts`](Self::accepts)), every number finite and within the
    /// kind's limits, a ramp's keys ascending in 0–1 and at most `most`, a
    /// tone curve's keys ascending.
    /// The error says why, as a phrase after the value's label ("must be a
    /// number from 0 to 10").
    pub fn check(&self, value: &Value) -> Result<(), String> {
        if !self.accepts(value) {
            return Err("doesn't take that value".to_owned());
        }
        let finite = |v: &[f64]| v.iter().all(|v| v.is_finite());
        match (self, value) {
            (Self::Number(n), Value::Number(v)) => n.check(std::slice::from_ref(v)),
            (Self::Vector { number: n, .. } | Self::Channels(n), Value::Numbers(v)) => n.check(v),
            (Self::Integer { min, max, .. }, Value::Integer(v)) => {
                if (min..=max).contains(&v) {
                    return Ok(());
                }
                Err(match (*min > i64::MIN, *max < i64::MAX) {
                    (true, true) => format!("must be a whole number from {min} to {max}"),
                    (true, false) => format!("must be a whole number of at least {min}"),
                    (false, _) => format!("must be a whole number of at most {max}"),
                })
            }
            (Self::Color, Value::Color(c)) => match finite(c) {
                true => Ok(()),
                false => Err("must be a color of finite channels".to_owned()),
            },
            (Self::Ramp { most, .. }, Value::Ramp(r)) => {
                let keys = &r.keys;
                if keys.is_empty() {
                    return Err("must hold a key".to_owned());
                }
                if let Some(most) = most.filter(|m| keys.len() > *m) {
                    return Err(format!("takes at most {most} keys"));
                }
                let positions = keys.iter().all(|k| (0.0..=1.0).contains(&k.position))
                    && keys.windows(2).all(|p| p[0].position <= p[1].position);
                let values = keys.iter().all(|k| finite(&k.value));
                match positions && values {
                    true => Ok(()),
                    false => Err("must have keys ascending in 0 to 1, values finite".to_owned()),
                }
            }
            (Self::ToneCurves { .. }, Value::ToneCurves(curves)) => {
                match curves.iter().all(|c| c.is_valid()) {
                    true => Ok(()),
                    false => Err("must have curves of two or more finite keys".to_owned()),
                }
            }
            _ => Ok(()),
        }
    }
}

/// How a number shows: its unit, limits and look.
#[derive(Clone, Debug, PartialEq)]
pub struct Number {
    /// What it measures.
    pub unit: Unit,
    /// The smallest value, in stored units: typed and dragged values clamp
    /// to it, and a smaller one is refused ([`ValueKind::check`]).
    pub min: f64,
    /// The largest value, in stored units, likewise.
    pub max: f64,
    /// The span dragging and the slider cover, when narrower than the
    /// limits; typing may leave it.
    pub soft: Option<(f64, f64)>,
    /// Shown as a slider filling the soft span (or the limits).
    pub slider: bool,
    /// Most decimal places shown; `None` leaves it to the host.
    pub decimals: Option<u8>,
    /// How much it changes per point dragged; `None` leaves it to the host.
    pub speed: Option<f64>,
    /// A length whose default is a real size in millimetres (a
    /// primitive's dimensions): the host gives every new node, and a reset
    /// to the default, that size in the document's unit. Its limits are
    /// in stored units, as its values are.
    pub real_size: bool,
}

impl Number {
    /// An unlimited number of `unit`.
    pub fn new(unit: Unit) -> Self {
        Self {
            unit,
            min: f64::NEG_INFINITY,
            max: f64::INFINITY,
            soft: None,
            slider: false,
            decimals: None,
            speed: None,
            real_size: false,
        }
    }

    /// A plain unlimited number.
    pub fn plain() -> Self {
        Self::new(Unit::Plain)
    }

    /// An unlimited length.
    pub fn length() -> Self {
        Self::new(Unit::Length)
    }

    /// An unlimited angle.
    pub fn angle() -> Self {
        Self::new(Unit::Angle)
    }

    /// With the limits `min..=max`.
    pub fn range(mut self, min: f64, max: f64) -> Self {
        (self.min, self.max) = (min, max);
        self
    }

    /// With the soft span `min..=max`.
    pub fn soft(mut self, min: f64, max: f64) -> Self {
        self.soft = Some((min, max));
        self
    }

    /// Shown as a slider.
    pub fn slider(mut self) -> Self {
        self.slider = true;
        self
    }

    /// Changing by `speed` per point dragged.
    pub fn speed(mut self, speed: f64) -> Self {
        self.speed = Some(speed);
        self
    }

    /// A length whose default is a real size in millimetres
    /// ([`Number::real_size`](Number#structfield.real_size)).
    pub fn real_size(mut self) -> Self {
        self.unit = Unit::Length;
        self.real_size = true;
        self
    }

    /// Showing at most `places` decimals.
    pub fn decimals(mut self, places: u8) -> Self {
        self.decimals = Some(places);
        self
    }

    /// Whether every one of `numbers` is finite and within the limits.
    fn check(&self, numbers: &[f64]) -> Result<(), String> {
        if numbers.iter().any(|v| !v.is_finite()) {
            return Err("must be a finite number".to_owned());
        }
        if numbers.iter().all(|v| (self.min..=self.max).contains(v)) {
            return Ok(());
        }
        let (min, max) = (self.min, self.max);
        Err(match (min.is_finite(), max.is_finite()) {
            (true, true) => format!("must be a number from {min} to {max}"),
            (true, false) => format!("must be a number of at least {min}"),
            (false, _) => format!("must be a number of at most {max}"),
        })
    }
}

/// What a number measures. The host shows and parses it in the user's
/// units (a length in the project's millimetres or inches).
#[derive(Clone, Debug, Default, PartialEq, Eq, Hash)]
pub enum Unit {
    /// A plain number.
    #[default]
    Plain,
    /// A length, stored in scene units.
    Length,
    /// A length stored and always shown in millimetres, whatever the
    /// project's unit (a lens's focal length).
    Millimeters,
    /// An angle, stored in degrees.
    Angle,
    /// A percentage, stored as shown (0–100).
    Percent,
    /// A plain number shown and typed with this suffix after it (`" px"`,
    /// `" EV"`, `" K"`).
    Suffix(Arc<str>),
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ramp::{Interpolation, Key, Ramp};
    use crate::tone_curve::ToneCurve;
    use crate::{Data, DataTypeId, Typed};

    #[test]
    fn numbers_take_finite_values_within_their_limits() {
        let length = ValueKind::Number(Number::length().range(0.0, 10.0));
        assert!(length.check(&Value::Number(10.0)).is_ok());
        let refused = |k: &ValueKind, v: Value| k.check(&v).unwrap_err();
        assert_eq!(
            refused(&length, Value::Number(-1.0)),
            "must be a number from 0 to 10"
        );
        assert_eq!(
            refused(&length, Value::Number(f64::NAN)),
            "must be a finite number"
        );
        assert_eq!(
            refused(&length, Value::Bool(true)),
            "doesn't take that value"
        );
        let size = ValueKind::Vector {
            components: 3,
            number: Number::length().range(0.0, f64::INFINITY),
        };
        assert!(size.check(&Value::Numbers(vec![1.0, 0.0, 2.0])).is_ok());
        assert!(size.check(&Value::Numbers(vec![1.0, -0.5, 2.0])).is_err());
        assert!(size.check(&Value::Numbers(vec![1.0; 2])).is_err());
    }

    #[test]
    fn integers_are_exact_and_within_their_limits() {
        let count = ValueKind::integer(1, i64::MAX);
        assert!(count.check(&Value::Integer(i64::MAX)).is_ok());
        assert_eq!(
            count.check(&Value::Integer(0)).unwrap_err(),
            "must be a whole number of at least 1"
        );
        assert!(!count.accepts(&Value::Number(2.0)), "a float is no integer");
    }

    #[test]
    fn choices_take_their_keys_alone() {
        let kind = ValueKind::choice(vec![Choice::new("a", "A"), Choice::new("b", "B")]);
        assert!(kind.accepts(&Value::Choice("b".into())));
        assert!(
            !kind.accepts(&Value::Choice("B".into())),
            "a label is no key"
        );
        assert!(!kind.accepts(&Value::Text("a".into())));
    }

    #[test]
    fn colors_ramps_and_curves_check_their_structure() {
        assert!(
            ValueKind::Color
                .check(&Value::Color([4.0, -0.5, 1.0]))
                .is_ok()
        );
        assert!(
            ValueKind::Color
                .check(&Value::Color([f64::NAN, 0.0, 0.0]))
                .is_err()
        );
        let ramp = ValueKind::Ramp {
            color: true,
            most: Some(2),
        };
        let mut r = Ramp::default_of(true);
        assert!(ramp.check(&Value::Ramp(r.clone())).is_ok());
        assert!(!ramp.accepts(&Value::Ramp(Ramp::default_of(false))));
        r.insert(0.5);
        assert_eq!(
            ramp.check(&Value::Ramp(r.clone())).unwrap_err(),
            "takes at most 2 keys"
        );
        let any = ValueKind::Ramp {
            color: true,
            most: None,
        };
        assert!(any.check(&Value::Ramp(r.clone())).is_ok());
        r.keys[1] = Key::color(2.0, [0.0; 3], Interpolation::Linear);
        assert!(any.check(&Value::Ramp(r)).is_err());
        let curves = ValueKind::ToneCurves {
            channels: vec![CurveChannel {
                label: "Master".into(),
                tint: None,
            }],
        };
        assert!(
            curves
                .check(&Value::ToneCurves(vec![ToneCurve::identity()]))
                .is_ok()
        );
        let one_key = ToneCurve {
            keys: vec![[0.0, 0.0]],
            ..ToneCurve::identity()
        };
        assert!(curves.check(&Value::ToneCurves(vec![one_key])).is_err());
        assert!(!curves.accepts(&Value::ToneCurves(Vec::new())));
    }

    #[test]
    fn kinds_give_the_animatable_default() {
        assert!(ValueKind::Number(Number::plain()).animatable());
        assert!(ValueKind::choice(vec![Choice::new("a", "A")]).animatable());
        assert!(!ValueKind::Code { language: None }.animatable());
        assert!(!ValueKind::Texts.animatable());
        let sketch = Typed::<Vec<f64>>::new(DataTypeId::declared("acme.cad.sketch"));
        let kind = ValueKind::Data(sketch.id());
        assert!(!kind.animatable());
        let value: Data = sketch.wrap(Vec::new());
        assert!(kind.check(&Value::Data(value)).is_ok());
        let other = Typed::<Vec<f64>>::new(DataTypeId::declared("acme.cad.other"));
        assert!(!kind.accepts(&Value::Data(other.wrap(Vec::new()))));
    }
}
