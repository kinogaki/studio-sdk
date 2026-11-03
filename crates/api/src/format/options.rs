//! What file formats share about their options: reading one typed value
//! out of a format's [`FormatOptions`], choices saved by a token of their
//! own (`"m"`, `"Z"`, `"Keep"`) rather than an index so a saved setting
//! survives a reordered list, and the rows most import dialogs show
//! (sections, a scale, toggles).
//!
//! ```
//! use kinogaki_studio_api::format::options;
//! use kinogaki_studio_api::{FormatOptions, Value};
//! const SIDES: [&str; 2] = ["front", "back"];
//! let mut chosen = FormatOptions::new();
//! chosen.insert("side".into(), Value::Index(options::choice_index(&SIDES, "back")));
//! chosen.insert("weld".into(), Value::Bool(true));
//! assert_eq!(options::choice(&chosen, "side", &SIDES).unwrap(), "back");
//! assert!(options::option(&chosen, "weld", Value::bool).unwrap());
//! assert!(options::option(&chosen, "scale", Value::number).is_err());
//! ```
use crate::params::{Number, Parameter, ParameterKind, Schema};
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

/// Choice option `name` as one of `all`.
pub fn choice<T: Copy>(options: &FormatOptions, name: &str, all: &[T]) -> Result<T, Error> {
    option(options, name, |v| {
        v.index().and_then(|i| all.get(i).copied())
    })
}

/// Where `value` is among `all` (the first when it is none of them).
pub fn choice_index<T: PartialEq>(all: &[T], value: T) -> usize {
    all.iter().position(|v| *v == value).unwrap_or(0)
}

/// A section band of a format's dialog, keyed by the format and the
/// title so a settings page can show several formats' at once.
pub fn section(schema: Schema, format: &str, title: &str) -> Schema {
    schema.section(format!("import-{format}-{title}"), title, true)
}

/// The `scale` row, within `SCALE`.
pub fn scale(schema: Schema, scale: f64) -> Schema {
    schema.parameter(
        Parameter::new(
            "scale",
            "Scale",
            ParameterKind::Number(
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
    schema.parameter(Parameter::new(name, label, ParameterKind::Toggle, value).keywords(keywords))
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
