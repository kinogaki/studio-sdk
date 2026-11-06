//! [`Schema`]: how parameters are presented, as data. The host turns it
//! into its inspector's rows; edits come back as new [`Value`]s.
use super::{Parameter, Parameters, Section};

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
        self.items
            .push(SchemaItem::Section(Section::new(key, label, open)));
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::{Number, Value, ValueKind};

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
}
