//! [`Value`]: what a parameter, a render setting, a declared prim's value
//! or a material port holds. Which variant a value is follows from its
//! [`ValueKind`](crate::ValueKind) ([`ValueKind::accepts`](crate::ValueKind::accepts)).
use crate::data::Data;
use crate::ramp::Ramp;
use crate::tone_curve::ToneCurve;
use std::collections::BTreeMap;
use std::sync::Arc;

/// A value of a [`ValueKind`](crate::ValueKind). Numbers are `f64` and
/// integers `i64` wherever they are held, authored or sent: an engine
/// narrows what it keeps on its own side.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Value {
    /// A real number, in stored units (lengths in scene units, angles in
    /// degrees, percentages as shown).
    Number(f64),
    /// A whole number, exact.
    Integer(i64),
    /// Several numbers: a vector's components, RGBA channels.
    Numbers(Vec<f64>),
    /// A color: linear RGB, in the working space as authored and in
    /// ACEScg as an engine receives it (the host converts at that one
    /// boundary).
    Color([f64; 3]),
    /// A toggle's state.
    Bool(bool),
    /// A choice, by its [`Choice::key`](crate::Choice::key): what files
    /// save, so relabelling or reordering choices never changes a value.
    Choice(Arc<str>),
    /// Text: a name, an expression, code, a path (to a file, a node, an
    /// object).
    Text(String),
    /// A scene object, by the host's id (none: no object): it names the
    /// object through renames and moves, where a path would have to be
    /// rewritten.
    Object(Option<crate::ObjectRef>),
    /// A float or color ramp.
    Ramp(Ramp),
    /// Tone curves, one a channel.
    ToneCurves(Vec<ToneCurve>),
    /// A value of a registered data type: a structured record a node's
    /// tools author (a sketch). Equal by type and allocation ([`Data`]'s
    /// rule).
    Data(Data),
}

impl Value {
    /// The number, when it is one.
    pub fn number(&self) -> Option<f64> {
        match self {
            Self::Number(v) => Some(*v),
            _ => None,
        }
    }

    /// The object named, when it is an object value naming one.
    pub fn object(&self) -> Option<crate::ObjectRef> {
        match self {
            Self::Object(o) => *o,
            _ => None,
        }
    }

    /// The whole number, when it is one.
    pub fn integer(&self) -> Option<i64> {
        match self {
            Self::Integer(v) => Some(*v),
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

    /// The color, when it is one.
    pub fn color(&self) -> Option<[f64; 3]> {
        match self {
            Self::Color(c) => Some(*c),
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

    /// The choice's key, when it is a choice.
    pub fn choice(&self) -> Option<&str> {
        match self {
            Self::Choice(v) => Some(v),
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

    /// The ramp, when it is one.
    pub fn ramp(&self) -> Option<&Ramp> {
        match self {
            Self::Ramp(v) => Some(v),
            _ => None,
        }
    }

    /// The tone curves, when they are some.
    pub fn tone_curves(&self) -> Option<&[ToneCurve]> {
        match self {
            Self::ToneCurves(v) => Some(v),
            _ => None,
        }
    }

    /// The data, when it is a data type's value.
    pub fn data(&self) -> Option<&Data> {
        match self {
            Self::Data(v) => Some(v),
            _ => None,
        }
    }
}

impl From<f64> for Value {
    fn from(v: f64) -> Self {
        Self::Number(v)
    }
}

impl From<i64> for Value {
    fn from(v: i64) -> Self {
        Self::Integer(v)
    }
}

impl From<bool> for Value {
    fn from(v: bool) -> Self {
        Self::Bool(v)
    }
}

impl From<[f64; 3]> for Value {
    fn from(v: [f64; 3]) -> Self {
        Self::Color(v)
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

impl From<Ramp> for Value {
    fn from(v: Ramp) -> Self {
        Self::Ramp(v)
    }
}

impl From<Data> for Value {
    fn from(v: Data) -> Self {
        Self::Data(v)
    }
}

/// Values by name (`"distance"`, `"samples"`): a node's parameters when it
/// cooks, a format's options when it reads or writes, an engine's settings
/// and a declared prim's values. Every declared name is present; a node
/// may hold more than its description declares (what its interactive
/// tools author, the parameters its code declares).
pub type Parameters = BTreeMap<String, Value>;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{DataTypeId, Typed};

    #[test]
    fn values_read_back_only_as_their_variant() {
        assert_eq!(Value::from(2.5).number(), Some(2.5));
        assert_eq!(Value::from(7_i64).integer(), Some(7));
        assert_eq!(Value::from(7_i64).number(), None);
        assert_eq!(Value::from(true).bool(), Some(true));
        assert_eq!(Value::Choice("smooth".into()).choice(), Some("smooth"));
        assert_eq!(Value::from([0.5; 3]).color(), Some([0.5; 3]));
        assert_eq!(Value::from("a").text(), Some("a"));
        assert_eq!(Value::from(vec![1.0, 2.0]).numbers(), Some(&[1.0, 2.0][..]));
        assert_eq!(Value::from(2.5).text(), None);
        assert_eq!(Value::from("a").choice(), None);
        let ramp = Value::from(Ramp::default_of(false));
        assert_eq!(ramp.ramp().map(|r| r.keys.len()), Some(2));
        let data = Typed::<f64>::new(DataTypeId::declared("acme.points.count"));
        let value = Value::from(data.wrap(1.0));
        assert_eq!(value.data().and_then(|d| data.get(d)), Some(&1.0));
        assert_eq!(value.clone(), value);
        assert_ne!(Value::from(data.wrap(1.0)), value);
    }

    /// Integers past 2^53 stay exact.
    #[test]
    fn integers_are_exact() {
        let big = (1_i64 << 53) + 1;
        assert_ne!(Value::Integer(big), Value::Integer(big - 1));
        assert_eq!(Value::Integer(i64::MAX).integer(), Some(i64::MAX));
    }
}
