//! [`Schema`]: how parameters are presented, as data. The host turns it
//! into its inspector's rows; edits come back as new [`Value`]s.
use super::{Parameters, Value, ValueKind};
use crate::Error;
use crate::data::Data;

/// What a parameter dialog or inspector shows, top to bottom: sections and
/// parameters.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Schema {
    /// Sections and parameters, in order. A parameter belongs to the
    /// section before it; parameters before the first section have none.
    pub items: Vec<SchemaItem>,
}

impl Schema {
    /// With a section band `label` (remembered open or closed by `key`)
    /// next.
    pub fn section(mut self, key: impl Into<String>, label: impl Into<String>, open: bool) -> Self {
        self.items.push(SchemaItem::Section(Section {
            key: key.into(),
            label: label.into(),
            open,
        }));
        self
    }

    /// With `parameter` next.
    pub fn parameter(mut self, parameter: Parameter) -> Self {
        self.items.push(SchemaItem::Parameter(parameter));
        self
    }

    /// With `button` next.
    pub fn button(mut self, button: Button) -> Self {
        self.items.push(SchemaItem::Button(button));
        self
    }

    /// The parameters, in order.
    pub fn parameters(&self) -> impl Iterator<Item = &Parameter> {
        self.items.iter().filter_map(|item| match item {
            SchemaItem::Parameter(p) => Some(p),
            SchemaItem::Section(_) | SchemaItem::Button(_) => None,
        })
    }

    /// Every parameter's value by name.
    pub fn values(&self) -> Parameters {
        self.parameters()
            .map(|p| (p.name.clone(), p.value.clone()))
            .collect()
    }
}

/// One row of a [`Schema`].
// Rows are few and short-lived (a sheet's, while it shows): a parameter
// kept inline is simpler to match than a boxed one.
#[allow(clippy::large_enum_variant)]
#[derive(Clone, Debug, PartialEq)]
pub enum SchemaItem {
    /// A collapsible section band; the parameters after it, up to the next
    /// band, are in it.
    Section(Section),
    /// A parameter.
    Parameter(Parameter),
    /// A button: it holds no value; pressing it asks the node's type
    /// ([`NodeType::pressed`](crate::NodeType::pressed)).
    Button(Button),
}

/// A button row of a [`Schema`].
#[derive(Clone, Debug, PartialEq)]
pub struct Button {
    /// What the node's type is told was pressed, unique in its schema.
    pub name: String,
    /// What the button says.
    pub label: String,
    /// Its tooltip; empty for none.
    pub help: String,
    /// Whether it can be pressed now; a disabled button's help says why.
    pub enabled: bool,
}

impl Button {
    /// Button `name` saying `label`: enabled, with no help.
    pub fn new(name: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            label: label.into(),
            help: String::new(),
            enabled: true,
        }
    }

    /// With `help` as its tooltip.
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = help.into();
        self
    }

    /// Disabled (or enabled again).
    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

/// A collapsible section band.
#[derive(Clone, Debug, PartialEq)]
pub struct Section {
    /// Remembers whether the band is open, across schemas.
    pub key: String,
    /// The band's text.
    pub label: String,
    /// Whether it starts open.
    pub open: bool,
}

/// One parameter: its name, what it holds, and its current value.
#[derive(Clone, Debug, PartialEq)]
pub struct Parameter {
    /// Its stored name (`distance`, `merge_shells`), unique in its schema.
    pub name: String,
    /// What its row says.
    pub label: String,
    /// What it holds, with its kind's options (a choice's entries, an
    /// attribute row's names on the input, filled by
    /// [`NodeType::schema`](crate::NodeType::schema)).
    pub kind: ValueKind,
    /// Its current value (the default, in a description).
    pub value: Value,
    /// Its tooltip; empty for none.
    pub help: String,
    /// More words a search finds it by ("orientation" for an up axis);
    /// empty for none.
    pub keywords: String,
    /// Whether it takes input; a disabled row shows its value greyed and
    /// its help says why.
    pub enabled: bool,
    /// Why its value is faulty (a group expression that doesn't parse):
    /// the row shows as an error with this as its tooltip. `None`: it isn't.
    pub problem: Option<String>,
    /// Whether it can be keyed over time. Its kind gives the default
    /// ([`ValueKind::animatable`]); a type turns it off for a parameter
    /// that shapes its ports or spares, or whose change should be one
    /// deliberate edit.
    pub animatable: bool,
}

impl Parameter {
    /// Parameter `name` labelled `label`, of `kind`, at `value`: enabled,
    /// with no help.
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
            help: String::new(),
            keywords: String::new(),
            enabled: true,
            problem: None,
            animatable,
        }
    }

    /// Data parameter `name` labelled `label` holding values of `empty`'s
    /// data type, starting at `empty` (the type's
    /// [`empty`](crate::DataType::empty)): [`ValueKind::Data`].
    pub fn data(name: impl Into<String>, label: impl Into<String>, empty: Data) -> Self {
        Self::new(name, label, ValueKind::Data(empty.ty()), empty)
    }

    /// Render setting (or declared prim value) `d` as a parameter at its
    /// default: its kind, label and tooltip, animatable as `d` says.
    pub fn setting(d: &kinogaki_render_api::SettingDescriptor) -> Self {
        let p = Self::new(&*d.name, &*d.label, d.kind.clone(), d.default.clone());
        let p = p.animatable(d.animatable);
        match &d.tooltip {
            Some(tip) => p.help(&**tip),
            None => p,
        }
    }

    /// Animatable or not (see [`animatable`](Self::animatable)).
    pub fn animatable(mut self, on: bool) -> Self {
        self.animatable = on;
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

    /// Faulty, `why` (see [`problem`](Self::problem)).
    pub fn problem(mut self, why: impl Into<String>) -> Self {
        self.problem = Some(why.into());
        self
    }

    /// Whether `value` is one this parameter takes, as core checks every
    /// node's parameters (when a node is made, edited or loaded): its
    /// kind's [`check`](ValueKind::check). The error says why, naming the
    /// parameter by its label.
    pub fn check(&self, value: &Value) -> Result<(), Error> {
        (self.kind.check(value)).map_err(|why| Error::invalid(format_args!("{} {why}", self.label)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Number;

    #[test]
    fn a_schema_lists_its_parameters_and_values() {
        let schema = Schema::default()
            .section("shape", "Shape", true)
            .parameter(Parameter::new(
                "distance",
                "Distance",
                ValueKind::Number(Number::length().range(0.0, f64::INFINITY)),
                1.5,
            ))
            .parameter(
                Parameter::new("cap", "Cap", ValueKind::Toggle, true).help("Close the ends"),
            );
        let names: Vec<&str> = schema.parameters().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["distance", "cap"]);
        assert_eq!(schema.values()["distance"], Value::Number(1.5));
        assert!(schema.parameters().all(|p| p.kind.accepts(&p.value)));
    }

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

    #[test]
    fn a_setting_is_a_parameter_of_its_kind() {
        let d = kinogaki_render_api::SettingDescriptor::new(
            "samples",
            "Samples",
            "main",
            ValueKind::integer(1, 64),
            8_i64,
        )
        .tooltip("Rays a pixel")
        .animatable(false);
        let p = Parameter::setting(&d);
        assert_eq!((p.kind, p.value), (d.kind, d.default));
        assert_eq!((p.help.as_str(), p.animatable), ("Rays a pixel", false));
    }
}
