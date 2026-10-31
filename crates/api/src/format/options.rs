//! What file formats share about their options: reading one typed value
//! out of a format's [`FormatOptions`], choices saved by a token of their
//! own (`"m"`, `"Z"`, `"Keep"`) rather than an index so a saved setting
//! survives a reordered list, and the rows most import dialogs show
//! (sections, the units a file is taken to be in, a scale, toggles).
//!
//! ```
//! use kinogaki_studio_api::format::options;
//! use kinogaki_studio_api::{FormatOptions, Value};
//! const SIDES: [&str; 2] = ["front", "back"];
//! let mut chosen = FormatOptions::new();
//! chosen.insert("side".into(), options::choice_value(&SIDES, "back"));
//! chosen.insert("weld".into(), Value::Bool(true));
//! assert_eq!(options::choice(&chosen, "side", &SIDES).unwrap(), "back");
//! assert!(options::option(&chosen, "weld", Value::bool).unwrap());
//! assert!(options::option(&chosen, "scale", Value::number).is_err());
//! ```
use crate::element::ElementReader;
use crate::params::{Number, Parameter, ParameterKind, Schema};
use crate::{Error, FormatOptions, Value};

/// The smallest and largest import scale [`scale`]'s row accepts.
pub const SCALE: std::ops::RangeInclusive<f64> = 1e-6..=1e6;

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

/// `value` as a choice among `all` (its index, [`choice_index`]).
pub fn choice_value<T: PartialEq>(all: &[T], value: T) -> Value {
    Value::Index(choice_index(all, value))
}

/// The choice among `all` whose token (`token`) field `name` of `element`
/// holds: how a saved choice is read back.
pub fn read_token<T: Copy>(
    element: &dyn ElementReader,
    name: &str,
    all: &[T],
    token: impl Fn(T) -> &'static str,
) -> Result<T, Error> {
    let text = element.text(name)?;
    all.iter()
        .copied()
        .find(|&v| token(v) == text)
        .ok_or_else(|| {
            Error::invalid(format_args!(
                "{}: unknown {name} {text:?}",
                element.location()
            ))
        })
}

/// A section band of a format's dialog, keyed by the format and the
/// title so a settings page can show several formats' at once.
pub fn section(schema: Schema, format: &str, title: &str) -> Schema {
    schema.section(format!("import-{format}-{title}"), title, true)
}

/// A choice row's labels and the one chosen.
#[derive(Clone, Copy, Debug)]
pub struct Choices<'a> {
    /// What each choice shows as.
    pub labels: &'a [&'a str],
    /// The chosen one's index.
    pub selected: usize,
}

/// The two rows of the units a file is taken to be in: `up` (its axis
/// choices) and `unit` (its length choices).
pub fn units(schema: Schema, up: Choices<'_>, unit: Choices<'_>) -> Schema {
    let labels = |c: Choices<'_>| c.labels.iter().map(|l| (*l).to_owned()).collect();
    schema
        .parameter(
            Parameter::new(
                "up",
                "Up axis in file",
                ParameterKind::Choice(labels(up)),
                Value::Index(up.selected),
            )
            .keywords("axis y z orientation"),
        )
        .parameter(
            Parameter::new(
                "unit",
                "Unit in file",
                ParameterKind::Choice(labels(unit)),
                Value::Index(unit.selected),
            )
            .keywords("length units scale meters"),
        )
}

/// The `scale` row, within [`SCALE`].
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
    use crate::element::{Field, MemoryElement};

    #[test]
    fn a_saved_token_reads_back_as_its_choice_and_an_unknown_one_is_named() {
        const SIDES: [&str; 2] = ["front", "back"];
        let saved = |text: &str| {
            let mut element = MemoryElement::new("import");
            element
                .write(|w| w.set("side", Field::Text(text.into())))
                .unwrap();
            element
        };
        let read = |e: &MemoryElement| e.read(|r| read_token(r, "side", &SIDES, |s| s));
        assert_eq!(read(&saved("back")).unwrap(), "back");
        let error = read(&saved("left")).unwrap_err();
        assert!(
            error.to_string().contains("unknown side \"left\""),
            "{error}"
        );
    }

    #[test]
    fn rows_carry_their_names_and_choices() {
        let up = Choices {
            labels: &["Y", "Z"],
            selected: 1,
        };
        let unit = Choices {
            labels: &["mm", "m"],
            selected: 0,
        };
        let schema = units(Schema::default(), up, unit);
        let schema = scale(toggle(schema, "weld", "Weld", "merge", true), 2.0);
        let names: Vec<_> = schema.parameters().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["up", "unit", "weld", "scale"]);
        assert_eq!(schema.parameters().next().unwrap().value, Value::Index(1));
    }
}
