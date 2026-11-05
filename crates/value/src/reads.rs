//! [`ParametersExt`]: a value of [`Parameters`] read by name as its variant,
//! or an error naming it.
use crate::data::Data;
use crate::value::{Parameters, Value};
use kinogaki_geometry::{Error, Vec3};

/// The typed readers of [`Parameters`]: a value by name as its variant, or
/// an error naming it. A node's cook, an Edit step, a compiler and a
/// format read their values with them (a cook's context reads through
/// them too).
pub trait ParametersExt {
    /// Value `name`, when it is there.
    fn value(&self, name: &str) -> Option<&Value>;

    /// Value `name` read by `read`, or an error saying it isn't `what`.
    ///
    /// # Errors
    /// When `name` is missing or `read` refuses it.
    fn read<'v, T>(
        &'v self,
        name: &str,
        what: &str,
        read: impl Fn(&'v Value) -> Option<T>,
    ) -> Result<T, Error> {
        self.value(name)
            .and_then(read)
            .ok_or_else(|| Error::invalid(format!("parameter {name:?} is not {what}")))
    }
    /// Value `name` as a number.
    ///
    /// # Errors
    /// When it isn't one.
    fn number(&self, name: &str) -> Result<f64, Error> {
        self.read(name, "a number", Value::number)
    }
    /// Value `name` as a whole number.
    ///
    /// # Errors
    /// When it isn't one.
    fn integer(&self, name: &str) -> Result<i64, Error> {
        self.read(name, "a whole number", Value::integer)
    }
    /// Value `name` as a toggle's state.
    ///
    /// # Errors
    /// When it isn't one.
    fn toggle(&self, name: &str) -> Result<bool, Error> {
        self.read(name, "a toggle", Value::bool)
    }
    /// Value `name` as a choice's key.
    ///
    /// # Errors
    /// When it isn't one.
    fn choice(&self, name: &str) -> Result<&str, Error> {
        self.read(name, "a choice", Value::choice)
    }
    /// Value `name` as text.
    ///
    /// # Errors
    /// When it isn't text.
    fn text(&self, name: &str) -> Result<&str, Error> {
        self.read(name, "text", Value::text)
    }
    /// Value `name` as three numbers (a vector).
    ///
    /// # Errors
    /// When it isn't three numbers.
    fn vector(&self, name: &str) -> Result<Vec3, Error> {
        self.read(name, "three numbers", |v| match v.numbers()? {
            &[x, y, z] => Some(Vec3::new(x, y, z)),
            _ => None,
        })
    }
    /// Value `name` as data.
    ///
    /// # Errors
    /// When it isn't data.
    fn data(&self, name: &str) -> Result<&Data, Error> {
        self.read(name, "data", Value::data)
    }
}

impl ParametersExt for Parameters {
    fn value(&self, name: &str) -> Option<&Value> {
        self.get(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_value_reads_as_its_variant_or_names_itself() {
        let mut p = Parameters::new();
        p.insert("distance".into(), Value::Number(1.5));
        p.insert("count".into(), Value::Integer(3));
        p.insert("on".into(), Value::Bool(true));
        p.insert("mode".into(), Value::Choice("smooth".into()));
        p.insert("name".into(), Value::Text("a".into()));
        p.insert("at".into(), Value::Numbers(vec![1.0, 2.0, 3.0]));
        assert_eq!(p.number("distance").unwrap(), 1.5);
        assert_eq!(p.integer("count").unwrap(), 3);
        assert!(p.toggle("on").unwrap());
        assert_eq!(p.choice("mode").unwrap(), "smooth");
        assert_eq!(p.text("name").unwrap(), "a");
        assert_eq!(p.vector("at").unwrap(), Vec3::new(1.0, 2.0, 3.0));
        let wrong = p.number("count").unwrap_err();
        assert!(
            wrong.to_string().contains("\"count\" is not a number"),
            "{wrong}"
        );
        assert!(p.text("missing").is_err() && p.data("name").is_err());
    }
}
