//! What file formats share about their options: reading one typed value
//! out of a format's [`FormatOptions`] (a choice by its key, so a saved
//! setting survives a reordered list), and the rows most import dialogs
//! show (a scale, toggles).
//!
//! ```
//! use kinogaki_studio_api::format::options;
//! use kinogaki_studio_api::{FormatOptions, Value};
//! let mut chosen = FormatOptions::new();
//! chosen.insert("side".into(), Value::Choice("back".into()));
//! chosen.insert("weld".into(), Value::Bool(true));
//! assert_eq!(options::option(&chosen, "side", Value::choice).unwrap(), "back");
//! assert!(options::option(&chosen, "weld", Value::bool).unwrap());
//! assert!(options::option(&chosen, "scale", Value::number).is_err());
//! ```
use crate::params::{Number, Parameter, Schema, ValueKind};
use crate::{Error, FormatOptions, Value};

/// The smallest and largest import scale [`scale`]'s row accepts.
pub(crate) const SCALE: std::ops::RangeInclusive<f64> = 1e-6..=1e6;

/// Option `name` of `options`, read by `read`; an error naming it when it
/// is missing or of another type.
pub fn option<'a, T>(
    options: &'a FormatOptions,
    name: &str,
    read: impl Fn(&'a Value) -> Option<T>,
) -> Result<T, Error> {
    options
        .get(name)
        .and_then(read)
        .ok_or_else(|| Error::invalid(format_args!("option {name} is missing or of another type")))
}

/// The `scale` row, within `SCALE`.
pub fn scale(schema: Schema, scale: f64) -> Schema {
    schema.parameter(
        Parameter::new(
            "scale",
            "Scale",
            ValueKind::Number(
                Number::plain()
                    .range(*SCALE.start(), *SCALE.end())
                    .speed(0.01)
                    .decimals(6),
            ),
            scale,
        )
        .keywords("size multiply factor"),
    )
}

/// A toggle row.
pub fn toggle(schema: Schema, name: &str, label: &str, keywords: &str, value: bool) -> Schema {
    schema.parameter(Parameter::new(name, label, ValueKind::Toggle, value).keywords(keywords))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rows_carry_their_names_and_values() {
        let schema = scale(
            toggle(Schema::default(), "weld", "Weld", "merge", true),
            2.0,
        );
        let names: Vec<_> = schema.parameters().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["weld", "scale"]);
        assert_eq!(schema.parameters().next().unwrap().value, Value::Bool(true));
    }
}
