//! [`Parameter`]: one value a person sets, described as data. The same
//! description serves a node's parameters, a format's options, an engine's
//! settings, the values of a prim type it declares (a light's size) and a
//! material node's inputs, so a host draws any of them as rows without
//! knowing whose they are; a [`Schema`](crate::Schema) holds them, in
//! sections.
use crate::condition::Condition;
use crate::data::Data;
use crate::kind::{Choice, Number, ValueKind};
use crate::look::NumberLook;
use crate::value::{Parameters, Value};
use kinogaki_geometry::Error;

/// One parameter: its name, what it holds, its value (its default, in a
/// description), how its row shows and when it applies.
#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    /// Its stored name (`distance`, `merge_shells`), unique where it is
    /// declared: the key its value is kept and saved by.
    pub name: String,
    /// What its row says.
    pub label: String,
    /// What it holds, with its kind's options (a choice's entries, an
    /// attribute row's names on the input).
    pub kind: ValueKind,
    /// Its current value: its default, in a description.
    pub value: Value,
    /// How its numbers show and move, for a number, vector or channels
    /// kind; the host's defaults for its unit unless set.
    pub look: NumberLook,
    /// Its tooltip: what it means, where its label alone does not say;
    /// empty for none.
    pub help: String,
    /// More words a search finds it by ("orientation" for an up axis);
    /// empty for none.
    pub keywords: String,
    /// Whether it takes input now; a disabled row shows its value greyed and
    /// its help says why.
    pub enabled: bool,
    /// Whether it can be keyed over time. Its kind gives the default
    /// ([`ValueKind::animatable`]); turned off for a parameter that shapes
    /// ports or spares, or whose change should be one deliberate edit (a
    /// sample cap).
    pub animatable: bool,
    /// The [`Section`](crate::Section) of its [`Schema`](crate::Schema) it
    /// sits in, by name; empty for none.
    pub group: String,
    /// When it applies; none: always. A host hides a row that doesn't
    /// apply (it takes no space) and keeps its value for when it applies
    /// again.
    requires: Option<Condition>,
}

impl Parameter {
    /// Parameter `name` labelled `label`, of `kind`, at `value`: enabled,
    /// in no section, always applying, with no help.
    pub fn new(
        name: impl Into<String>,
        label: impl Into<String>,
        kind: ValueKind,
        value: impl Into<Value>,
    ) -> Self {
        let animatable = kind.animatable();
        Self {
            name: name.into(),
            label: label.into(),
            kind,
            value: value.into(),
            look: NumberLook::default(),
            help: String::new(),
            keywords: String::new(),
            enabled: true,
            animatable,
            group: String::new(),
            requires: None,
        }
    }

    /// A number of `number`'s unit and limits, at `default`.
    pub fn number(
        name: impl Into<String>,
        label: impl Into<String>,
        number: Number,
        default: f64,
    ) -> Self {
        Self::new(name, label, ValueKind::Number(number), default)
    }

    /// A slider over `min..=max`, dragged over `soft`, shown with
    /// `decimals` places, at `default`.
    pub fn slider(
        name: impl Into<String>,
        label: impl Into<String>,
        (min, max): (f64, f64),
        soft: (f64, f64),
        decimals: u8,
        default: f64,
    ) -> Self {
        let number = Number::plain().range(min, max).soft(soft.0, soft.1);
        Self::number(name, label, number, default)
            .look(NumberLook::slider().decimals(decimals).speed(0.01))
    }

    /// A length in scene units over `min..=max`, dragged over `soft` when
    /// given, shown with `decimals` places, at `default`.
    pub fn length(
        name: impl Into<String>,
        label: impl Into<String>,
        (min, max): (f64, f64),
        soft: Option<(f64, f64)>,
        decimals: u8,
        default: f64,
    ) -> Self {
        let number = Number::length().range(min, max);
        let number = match soft {
            Some((a, b)) => number.soft(a, b),
            None => number,
        };
        Self::number(name, label, number, default)
            .look(NumberLook::default().decimals(decimals).speed(0.01))
    }

    /// A whole number in `min..=max`, at `default`.
    pub fn integer(
        name: impl Into<String>,
        label: impl Into<String>,
        min: i64,
        max: i64,
        default: i64,
    ) -> Self {
        Self::new(name, label, ValueKind::integer(min, max), default)
    }

    /// Three numbers, each of `number`'s unit and limits, at `default`.
    pub fn vector(
        name: impl Into<String>,
        label: impl Into<String>,
        number: Number,
        default: [f64; 3],
    ) -> Self {
        let kind = ValueKind::Vector {
            components: 3,
            number,
        };
        Self::new(name, label, kind, default.to_vec())
    }

    /// A toggle, at `default`.
    pub fn toggle(name: impl Into<String>, label: impl Into<String>, default: bool) -> Self {
        Self::new(name, label, ValueKind::Toggle, default)
    }

    /// A line of text, at `default`.
    pub fn text(name: impl Into<String>, label: impl Into<String>, default: &str) -> Self {
        Self::new(name, label, ValueKind::Text, default)
    }

    /// One of `choices` (`(key, label)`, in order), at key `default`.
    pub fn choice(
        name: impl Into<String>,
        label: impl Into<String>,
        choices: &[(&str, &str)],
        default: &str,
    ) -> Self {
        let choices = choices.iter().map(|(key, label)| Choice::new(key, label));
        let kind = ValueKind::choice(choices.collect());
        Self::new(name, label, kind, Value::Choice(default.into()))
    }

    /// Attribute names of the input, one or (`list`) a list, at `default`;
    /// the node type fills the menu.
    pub fn attribute(
        name: impl Into<String>,
        label: impl Into<String>,
        list: bool,
        default: &str,
    ) -> Self {
        let kind = ValueKind::Attribute {
            menu: Vec::new(),
            list,
        };
        Self::new(name, label, kind, default)
    }

    /// A parameter holding values of `empty`'s data type, starting at
    /// `empty` (the type's empty value): [`ValueKind::Data`].
    pub fn data(name: impl Into<String>, label: impl Into<String>, empty: Data) -> Self {
        Self::new(name, label, ValueKind::Data(empty.ty()), empty)
    }

    /// In section `group` (see [`group`](Self::group)).
    pub fn group(mut self, group: impl Into<String>) -> Self {
        self.group = group.into();
        self
    }

    /// Animatable or not (see [`animatable`](Self::animatable)).
    pub fn animatable(mut self, on: bool) -> Self {
        self.animatable = on;
        self
    }

    /// Its numbers shown and moved as `look` says.
    pub fn look(mut self, look: NumberLook) -> Self {
        self.look = look;
        self
    }

    /// With `help` as its tooltip.
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = help.into();
        self
    }

    /// Found by search under `keywords` too.
    pub fn keywords(mut self, keywords: impl Into<String>) -> Self {
        self.keywords = keywords.into();
        self
    }

    /// Disabled (or enabled again).
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    /// Applying, and so shown, while `when` holds of the values beside it.
    pub fn requires(mut self, when: Condition) -> Self {
        self.requires = Some(when);
        self
    }

    /// Whether it applies while the parameters beside it hold `values`.
    pub fn applies(&self, values: &Parameters) -> bool {
        self.requires.as_ref().is_none_or(|c| c.met(values))
    }

    /// It as its row shows while the parameters beside it hold `values`,
    /// its condition spent; none while it doesn't apply (hidden).
    pub fn resolved(mut self, values: &Parameters) -> Option<Self> {
        self.applies(values).then(|| {
            self.requires = None;
            self
        })
    }

    /// Whether `value` is one it takes: of its kind and within its limits.
    pub fn accepts(&self, value: &Value) -> bool {
        self.check(value).is_ok()
    }

    /// Whether `value` is one it takes, as a host checks every value (when
    /// a node is made, edited or loaded): its kind's
    /// [`check`](ValueKind::check). The error says why, naming the
    /// parameter by its label.
    pub fn check(&self, value: &Value) -> Result<(), Error> {
        Ok(self.kind.check(value, &self.label)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::ValueKind;

    /// A data parameter takes values of its data type alone, starts at the
    /// empty value it was declared with and is never animated.
    #[test]
    fn a_data_parameter_takes_values_of_its_type_alone() {
        let sketch = crate::Typed::<Vec<f64>>::new(crate::DataTypeId::declared("acme.cad.sketch"));
        let other = crate::Typed::<Vec<f64>>::new(crate::DataTypeId::declared("acme.cad.other"));
        let p = Parameter::data("sketch", "Sketch", sketch.wrap(Vec::new()));
        assert_eq!(p.kind, ValueKind::Data(sketch.id()));
        assert!(!p.animatable && !p.kind.animatable());
        assert!(p.check(&p.value).is_ok());
        assert!(p.check(&Value::Data(sketch.wrap(vec![1.0]))).is_ok());
        assert!(p.check(&Value::Data(other.wrap(Vec::new()))).is_err());
        assert!(p.check(&Value::Number(1.0)).is_err());
        assert!(!ValueKind::Text.accepts(&p.value));
    }

    #[test]
    fn a_parameter_check_names_it_by_its_label() {
        let length = Parameter::new(
            "distance",
            "Distance",
            ValueKind::Number(Number::length().range(0.0, 10.0)),
            1.0,
        );
        assert!(length.check(&Value::Number(10.0)).is_ok());
        assert_eq!(
            length.check(&Value::Number(-1.0)).unwrap_err().message(),
            "Distance must be a number from 0 to 10"
        );
        let count = Parameter::new("count", "Count", ValueKind::integer(1, i64::MAX), 2_i64);
        assert!(count.check(&Value::Integer(1 << 60)).is_ok());
        assert_eq!(
            count.check(&Value::Number(2.0)).unwrap_err().message(),
            "Count doesn't take that value"
        );
    }
}
