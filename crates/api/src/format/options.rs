//! What file formats share about their options: reading one typed value
//! out of a format's [`FormatOptions`] (a choice by its key, so a saved
//! setting survives a reordered list), the rows most formats show
//! (a scale, toggles, [`embed`]), the rows a node holds a format's
//! options in ([`row`], [`from_rows`]), and the element a host saves a
//! reader's or a writer's options in elsewhere ([`write()`], [`read()`]).
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
use crate::element::{ElementReader, ElementWriter, Field, FieldRef};
use crate::params::{Number, NumberLook, Parameter, Schema, ValueKind};
use crate::{Error, FormatId, FormatOptions, Parameters, Value};

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

/// The row a node holds format `format`'s option `option` in:
/// `<format id>/<option>` (`acme.format.blocks/scale`). A File node holds
/// its readers' options so, a node writing files its writers', each
/// format's apart (a project stores each format's as an element of its
/// own, keyed by its id).
pub fn row(format: FormatId, option: &str) -> String {
    format!("{format}/{option}")
}

/// `options` (a format's or a writer's, each at its default) as `rows`
/// hold them: each overridden by its [`row`] of `format` when that holds a
/// value of its kind.
pub fn from_rows(format: FormatId, mut options: FormatOptions, rows: &Parameters) -> FormatOptions {
    for (option, value) in options.iter_mut() {
        if let Some(held) = rows.get(&row(format, option))
            && std::mem::discriminant(held) == std::mem::discriminant(value)
        {
            *value = held.clone();
        }
    }
    options
}

/// Writes `options` (a reader's or a writer's) into the element `out`
/// fills, each a field named as it: a number a real, an integer an
/// integer, a toggle a boolean, a choice its key, text as text, a color,
/// numbers and texts arrays (an option holds no ramp, tone curves, object
/// or data).
///
/// # Errors
/// An option of a kind no field holds, or the element's own error.
pub fn write(options: &FormatOptions, out: &mut dyn ElementWriter) -> Result<(), Error> {
    for (name, value) in options {
        let field = match value {
            Value::Number(v) => Field::Float(*v),
            Value::Numbers(v) => Field::F64s(v.clone()),
            Value::Bool(v) => Field::Bool(*v),
            Value::Integer(v) => Field::Int(*v),
            Value::Color(c) => Field::F64s(c.to_vec()),
            Value::Choice(v) => Field::Text(v.to_string()),
            Value::Text(v) => Field::Text(v.clone()),
            _ => {
                return Err(Error::invalid(format_args!(
                    "option {name}: a format's options hold no ramps, curves, objects or data"
                )));
            }
        };
        out.set(name, field)?;
    }
    Ok(())
}

/// The options [`write()`] wrote into `element`, read strictly against
/// `defaults` (the reader's or the writer's, each at its default): every
/// one, each of its default's type.
///
/// # Errors
/// An option missing or of another type, named with the element.
pub fn read(defaults: FormatOptions, element: &dyn ElementReader) -> Result<FormatOptions, Error> {
    let mut options = defaults;
    for (name, value) in &mut options {
        let wrong = || {
            Error::invalid(format_args!(
                "{}: option {name} is not of its type",
                element.location()
            ))
        };
        *value = match (&*value, element.required(name)?) {
            (Value::Number(_), FieldRef::Float(v)) => Value::Number(v),
            (Value::Numbers(_), FieldRef::F64s(v)) => Value::Numbers(v.to_vec()),
            (Value::Bool(_), FieldRef::Bool(v)) => Value::Bool(v),
            (Value::Integer(_), FieldRef::Int(v)) => Value::Integer(v),
            (Value::Color(_), FieldRef::F64s(&[r, g, b])) => Value::Color([r, g, b]),
            (Value::Choice(_), FieldRef::Text(v)) => Value::Choice(v.into()),
            (Value::Text(_), FieldRef::Text(v)) => Value::Text(v.to_owned()),
            _ => return Err(wrong()),
        };
    }
    Ok(options)
}

/// The `scale` row, within `SCALE`.
pub fn scale(schema: Schema, scale: f64) -> Schema {
    schema.parameter(
        Parameter::new(
            "scale",
            "Scale",
            ValueKind::Number(Number::plain().range(*SCALE.start(), *SCALE.end())),
            scale,
        )
        .look(NumberLook::default().speed(0.01).decimals(6))
        .keywords("size multiply factor"),
    )
}

/// The option saying whether textures a file names on disk are embedded
/// ([`embed`]).
pub const EMBED: &str = "embed";

/// The Embed Textures row ([`EMBED`], off by default): on, the textures a
/// file names on disk are read into the project, its Image nodes holding
/// them embedded; off, they stay paths. Textures packed in the file (a
/// usdz's, a GLB's) are embedded whatever it says.
pub fn embed(schema: Schema, embed: bool) -> Schema {
    toggle(
        schema,
        EMBED,
        "Embed Textures",
        "images pictures pack inline keep",
        embed,
    )
}

/// A toggle row.
pub fn toggle(schema: Schema, name: &str, label: &str, keywords: &str, value: bool) -> Schema {
    schema.parameter(Parameter::new(name, label, ValueKind::Toggle, value).keywords(keywords))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::MemoryElement;

    #[test]
    fn options_save_as_fields_and_read_back_strictly() {
        let defaults = FormatOptions::from([
            ("scale".to_owned(), Value::Number(1.0)),
            ("normals".to_owned(), Value::Bool(true)),
            ("up".to_owned(), Value::Choice("y".into())),
            ("prims".to_owned(), Value::Text(String::new())),
            ("color".to_owned(), Value::Color([0.5; 3])),
            ("count".to_owned(), Value::Integer(3)),
        ]);
        let mut options = defaults.clone();
        options.insert("scale".into(), Value::Number(2.5));
        options.insert("prims".into(), Value::Text("/World".into()));
        let mut element = MemoryElement::new("plain");
        element.write(|w| write(&options, w)).unwrap();
        assert_eq!(
            element.read(|r| read(defaults.clone(), r)).unwrap(),
            options
        );
        // Strict: a missing option, or one of another type, fails.
        let mut short = MemoryElement::new("plain");
        short
            .write(|w| w.set("scale", Field::Text("2".into())))
            .unwrap();
        assert!(short.read(|r| read(defaults.clone(), r)).is_err());
    }

    #[test]
    fn rows_carry_their_names_and_values() {
        let schema = scale(
            toggle(Schema::default(), "weld", "Weld", "merge", true),
            2.0,
        );
        let schema = embed(schema, false);
        let names: Vec<_> = schema.parameters().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["weld", "scale", EMBED]);
        assert_eq!(schema.parameters().next().unwrap().value, Value::Bool(true));
    }

    #[test]
    fn a_node_holds_a_formats_options_in_rows_of_its_id() {
        let blocks = FormatId::declared("acme.format.blocks");
        assert_eq!(row(blocks, "scale"), "acme.format.blocks/scale");
        let defaults = FormatOptions::from([
            ("scale".to_owned(), Value::Number(1.0)),
            ("weld".to_owned(), Value::Bool(true)),
        ]);
        let rows = Parameters::from([
            (row(blocks, "scale"), Value::Number(2.0)),
            // Of another kind than its option: the default stays.
            (row(blocks, "weld"), Value::Number(0.0)),
            ("acme.format.other/scale".to_owned(), Value::Number(9.0)),
        ]);
        let read = from_rows(blocks, defaults, &rows);
        assert_eq!(read["scale"], Value::Number(2.0));
        assert_eq!(read["weld"], Value::Bool(true));
    }
}
