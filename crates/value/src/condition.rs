//! [`Condition`]: when something applies, as values holding values: a
//! port that is required, a handle that shows, a row that is enabled or
//! shown.
use crate::value::{Parameters, Value};

/// While every named value holds its value; with none, always.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Condition {
    /// The values tested, by name, and what each must hold.
    pub all: Vec<(String, Value)>,
}

impl Condition {
    /// While `name` holds `value`.
    pub fn when(name: impl Into<String>, value: impl Into<Value>) -> Self {
        Self::default().and(name, value)
    }

    /// It, and `name` holding `value` too.
    pub fn and(mut self, name: impl Into<String>, value: impl Into<Value>) -> Self {
        self.all.push((name.into(), value.into()));
        self
    }

    /// Whether `values` meet it (a name missing from them does not).
    pub fn met(&self, values: &Parameters) -> bool {
        (self.all.iter()).all(|(name, value)| values.get(name) == Some(value))
    }

    /// The names it reads.
    pub fn names(&self) -> impl Iterator<Item = &str> {
        self.all.iter().map(|(name, _)| name.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_condition_holds_while_every_value_does() {
        let mut values = Parameters::new();
        values.insert("shape".into(), Value::Choice("rect".into()));
        assert!(Condition::default().met(&values));
        let rect = Condition::when("shape", Value::Choice("rect".into()));
        assert!(rect.met(&values));
        let both = rect.clone().and("on", true);
        assert!(!both.met(&values), "a missing value doesn't meet it");
        values.insert("on".into(), Value::Bool(true));
        assert!(both.met(&values));
        assert_eq!(both.names().collect::<Vec<_>>(), ["shape", "on"]);
    }
}
