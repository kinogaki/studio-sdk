//! Parameters: what a node's or a format's settings hold ([`Value`],
//! [`Parameters`]), how the host presents them ([`Schema`]), and the
//! [`Diagnostic`]s cooks and readers report; [`ramp`] holds the ramps'
//! maths and encoding.
//!
//! These are the API's own types. Studio draws a [`Schema`] with its
//! inspector and stores [`Value`]s in projects; a package never sees the
//! host's UI types.
mod diagnostic;
mod schema;

pub use diagnostic::{Diagnostic, Severity, Span};
pub use kinogaki_render_api::ramp;
pub use schema::{Number, Parameter, ParameterKind, Schema, SchemaItem, Section, Unit};

use std::collections::BTreeMap;

/// A parameter's value. Which variant a parameter holds follows from its
/// [`ParameterKind`] (see [`ParameterKind::accepts`]).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A number, in stored units (lengths in scene units, angles in
    /// degrees, percentages as shown); also an integer parameter's value.
    Number(f64),
    /// Several numbers: a vector's components, a color's channels, an
    /// encoded ramp.
    Numbers(Vec<f64>),
    /// A toggle's state.
    Bool(bool),
    /// A choice's index into its entries.
    Index(usize),
    /// Text: a name, an expression, code, a file path.
    Text(String),
}

impl Value {
    /// The number, when it is one.
    pub fn number(&self) -> Option<f64> {
        match self {
            Self::Number(v) => Some(*v),
            _ => None,
        }
    }

    /// The numbers, when they are several.
    pub fn numbers(&self) -> Option<&[f64]> {
        match self {
            Self::Numbers(v) => Some(v),
            _ => None,
        }
    }

    /// The state, when it is a toggle's.
    pub fn bool(&self) -> Option<bool> {
        match self {
            Self::Bool(v) => Some(*v),
            _ => None,
        }
    }

    /// The index, when it is a choice's.
    pub fn index(&self) -> Option<usize> {
        match self {
            Self::Index(v) => Some(*v),
            _ => None,
        }
    }

    /// The text, when it is text.
    pub fn text(&self) -> Option<&str> {
        match self {
            Self::Text(v) => Some(v),
            _ => None,
        }
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Self::Number(v)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<&str> for Value {
    fn from(v: &str) -> Self {
        Self::Text(v.to_owned())
    }
}

impl From<String> for Value {
    fn from(v: String) -> Self {
        Self::Text(v)
    }
}

impl From<Vec<f64>> for Value {
    fn from(v: Vec<f64>) -> Self {
        Self::Numbers(v)
    }
}

/// Values by parameter name (`"distance"`, `"merge_shells"`): a node's
/// parameters when it cooks, a format's options when it reads or writes.
/// Every declared parameter is present. A node may hold more than its
/// description declares: what its interactive tools author (a stroke's
/// points) and the parameters its code declares.
pub type Parameters = BTreeMap<String, Value>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_read_back_only_as_their_variant() {
        assert_eq!(Value::from(2.5).number(), Some(2.5));
        assert_eq!(Value::from(true).bool(), Some(true));
        assert_eq!(Value::Index(3).index(), Some(3));
        assert_eq!(Value::from("a").text(), Some("a"));
        assert_eq!(Value::from(vec![1.0, 2.0]).numbers(), Some(&[1.0, 2.0][..]));
        assert_eq!(Value::from(2.5).text(), None);
        assert_eq!(Value::from("a").number(), None);
    }
}
