//! A value as a field, and back: the one encoding of every value kind, in
//! projects, a kind's codec and the user's settings alike.
//!
//! | Value | Field |
//! |---|---|
//! | Number | [`Float`](Field::Float) |
//! | Integer | [`Int`](Field::Int) |
//! | Numbers | [`F64s`](Field::F64s) |
//! | Color | [`F64s`](Field::F64s): three |
//! | Bool | [`Bool`](Field::Bool) |
//! | Choice | [`Text`](Field::Text): its key |
//! | Text | [`Text`](Field::Text) |
//! | Object | [`I64s`](Field::I64s): none, or the object's id (an id in a project; an item's index where a format's items name one another) |
//! | Ramp | [`F64s`](Field::F64s): a key after another, each its position, its value (one channel, or three) and its interpolation's place in `Interpolation::ALL` |
//! | ToneCurves | [`F64s`](Field::F64s): a curve after another, its interpolation (0 monotone, 1 linear), its key count, then its keys' x, y |
//!
//! An image ([`Value::Image`]) is no field: it is a child element of kind
//! `image` ([`write_image`], [`read_image`]): empty for none; else `file` (a
//! path), `embedded` (the file, through its type's codec, written once a save
//! for equal bytes) or `node` (an Image node's path from the root), and with a
//! file its `color` (an [`ImageColor`]'s key, as an Image node's Color holds
//! it).
//!
//! A value reads back by its declared kind; one no descriptor declares reads
//! back by its field's type alone (an image by its element's kind), and
//! writes back the same. A body holding values by their descriptors' names
//! (a light's, a render output's engine settings) saves them as one
//! element's fields and image elements ([`write_values`], [`read_values`]).
use super::{ElementReader, ElementWriter, Field, FieldRef};
use crate::params::ramp::{Interpolation, Key, Ramp};
use crate::params::tone_curve::{self, ToneCurve};
use crate::params::{Value, ValueKind};
use crate::standard::embedded_file;
use crate::{Error, Parameters};
use kinogaki_geometry::image::{ImageColor, ImageFile, ImageSource};
use kinogaki_render_api::Parameter;
use kinogaki_value::{ImageRef, ObjectRef};
use std::collections::btree_map::Entry;
use std::sync::Arc;

/// `values` as fields of `out`, each under its name.
///
/// # Errors
/// A value no field holds ([`field_of`]).
pub fn write_values(out: &mut dyn ElementWriter, values: &Parameters) -> Result<(), Error> {
    values.iter().try_for_each(|(name, value)| match value {
        Value::Image(image) => write_image(out, name, image.as_ref()),
        value => out.set(name, field_of(name, value)?),
    })
}

/// What an image element is.
const IMAGE: &str = "image";

/// `image` as child `name` of `out` (see the module docs).
///
/// # Errors
/// The element refusing the child or a field.
pub fn write_image(
    out: &mut dyn ElementWriter,
    name: &str,
    image: Option<&ImageRef>,
) -> Result<(), Error> {
    let out = &mut *out.child(name, IMAGE)?;
    match image {
        None => Ok(()),
        Some(ImageRef::Node(path)) => out.set("node", Field::Text(path.to_string())),
        Some(ImageRef::File(source)) => {
            match source.file() {
                ImageFile::Path(path) => out.set("file", Field::Text(path.to_string()))?,
                ImageFile::Embedded(file) => {
                    out.data("embedded", &embedded_file().wrap(file.clone()))?
                }
            }
            out.set("color", Field::Text(source.color().key().to_owned()))
        }
    }
}

/// The image child `name` of `e` holds (see the module docs).
///
/// # Errors
/// No such child, or one holding something else.
pub fn read_image(e: &dyn ElementReader, name: &str) -> Result<Option<ImageRef>, Error> {
    let at = e.required_child(name)?;
    let wrong = |what: &str| Error::invalid(format_args!("{}: {what}", at.location()));
    if at.kind() != IMAGE {
        return Err(wrong("is no image"));
    }
    let color = || match at.text("color")? {
        "" => Err(wrong("its color is no Color")),
        key => Ok(ImageColor::of_key(key)),
    };
    let file = |file| Ok(Some(ImageRef::File(ImageSource::new(file, color()?))));
    match (at.field("file"), at.data("embedded"), at.field("node")) {
        (None, None, None) => Ok(None),
        (Some(FieldRef::Text(path)), None, None) => file(ImageFile::Path(Arc::from(path))),
        (None, Some(data), None) => {
            let data = data?;
            let held = embedded_file()
                .get(&data)
                .ok_or_else(|| wrong("not a file"))?;
            file(ImageFile::Embedded(held.clone()))
        }
        (None, None, Some(FieldRef::Text(path))) => Ok(Some(ImageRef::Node(Arc::from(path)))),
        _ => Err(wrong("it names no one image")),
    }
}

/// The values `e`'s fields hold: each of `settings` by its descriptor (of
/// its kind, within its limits), and every other field as its type says
/// (another engine's value, kept so a save writes it back).
///
/// # Errors
/// A setting missing, of another kind or out of range, naming it.
pub fn read_values<'d>(
    e: &dyn ElementReader,
    settings: impl IntoIterator<Item = &'d Parameter>,
) -> Result<Parameters, Error> {
    let bad =
        |name: &str, why: String| Error::invalid(format_args!("{}: {name} {why}", e.location()));
    let mut values = Parameters::new();
    for d in settings {
        let value = match d.kind {
            ValueKind::Image => Value::Image(read_image(e, &d.name)?),
            _ => {
                let field = e
                    .field(&d.name)
                    .ok_or_else(|| bad(&d.name, "is missing".into()))?;
                value_of(field, Some(&d.kind)).map_err(|why| bad(&d.name, why))?
            }
        };
        if !d.accepts(&value) {
            return Err(bad(&d.name, "is out of range".into()));
        }
        values.insert(d.name.to_string(), value);
    }
    for name in e.fields() {
        if let Entry::Vacant(held) = values.entry(name) {
            let value =
                value_of(e.required(held.key())?, None).map_err(|why| bad(held.key(), why))?;
            held.insert(value);
        }
    }
    for name in e.children() {
        if let Entry::Vacant(held) = values.entry(name) {
            let image = read_image(e, held.key())?;
            held.insert(Value::Image(image));
        }
    }
    Ok(values)
}

/// `value` as a field (named `name`, for what it says).
///
/// # Errors
/// An id past `i64`, a ramp without keys, or data (it saves as an element
/// of its own).
pub fn field_of(name: &str, value: &Value) -> Result<Field, Error> {
    let refused = |what: &str| Error::invalid(format_args!("{name}: {what}"));
    Ok(match value {
        Value::Number(n) => Field::Float(*n),
        Value::Integer(n) => Field::Int(*n),
        Value::Numbers(n) => Field::F64s(n.clone()),
        Value::Color(c) => Field::F64s(c.to_vec()),
        Value::Bool(b) => Field::Bool(*b),
        Value::Choice(key) => Field::Text(key.to_string()),
        Value::Text(t) => Field::Text(t.clone()),
        Value::Object(object) => {
            let id = |o: &ObjectRef| i64::try_from(o.0).map_err(|_| refused("an id out of range"));
            Field::I64s(object.iter().map(id).collect::<Result<_, _>>()?)
        }
        Value::Ramp(r) if r.keys.is_empty() => return Err(refused("a ramp with no key")),
        Value::Ramp(r) => Field::F64s(ramp_numbers(r)),
        Value::ToneCurves(curves) => Field::F64s(curve_numbers(curves)),
        Value::Data(_) => return Err(refused("data saves as a data element, not a field")),
        Value::Image(_) => return Err(refused("an image saves as an image element, not a field")),
        _ => return Err(refused("a value of a kind no field holds")),
    })
}

/// `field` (or one of its time samples) as a value of `kind` when declared,
/// else as its type says.
///
/// # Errors
/// What it is not, as a phrase after the field's name ("is not an int64").
pub fn value_of(field: FieldRef<'_>, kind: Option<&ValueKind>) -> Result<Value, String> {
    use ValueKind as K;
    let wrong = |expected: &str| format!("is not {expected}");
    let number = || match field {
        FieldRef::Float(x) => Ok(x),
        _ => Err(wrong("a float64")),
    };
    let integer = || match field {
        FieldRef::Int(n) => Ok(n),
        _ => Err(wrong("an int64")),
    };
    let numbers = || match field {
        FieldRef::F64s(n) => Ok(n),
        _ => Err(wrong("float64")),
    };
    let boolean = || match field {
        FieldRef::Bool(b) => Ok(b),
        _ => Err(wrong("bool")),
    };
    let text = || match field {
        FieldRef::Text(t) => Ok(t),
        _ => Err(wrong("a string")),
    };
    Ok(match kind {
        Some(K::Number(_)) => Value::Number(number()?),
        Some(K::Integer { .. }) => Value::Integer(integer()?),
        Some(K::Toggle) => Value::Bool(boolean()?),
        Some(K::Choice { .. } | K::ColorSpace { .. }) => Value::Choice(text()?.into()),
        Some(
            K::Text
            | K::Code
            | K::File
            | K::SaveFile
            | K::Path
            | K::Reference(_)
            | K::Attribute { .. }
            | K::Group,
        ) => Value::Text(text()?.to_owned()),
        Some(K::Color | K::DisplayColor) => match *numbers()? {
            [r, g, b] => Value::Color([r, g, b]),
            _ => return Err(wrong("three float64")),
        },
        Some(&K::Ramp { color, .. }) => {
            Value::Ramp(ramp_of(numbers()?, color).ok_or_else(|| wrong("a ramp's keys"))?)
        }
        Some(K::ToneCurves { .. }) => {
            Value::ToneCurves(curves_of(numbers()?).ok_or_else(|| wrong("tone curves"))?)
        }
        Some(K::Vector { .. } | K::Channels(_)) => Value::Numbers(numbers()?.to_vec()),
        Some(K::Object(_)) => match field {
            FieldRef::I64s([]) => Value::Object(None),
            FieldRef::I64s(&[id]) => Value::Object(Some(ObjectRef(
                u64::try_from(id).map_err(|_| wrong("an id"))?,
            ))),
            _ => return Err(wrong("an object's id")),
        },
        Some(K::Data(_)) => return Err(wrong("a data element")),
        Some(K::Image) => return Err(wrong("an image element")),
        Some(_) => return Err(wrong("a kind no field holds")),
        None => match field {
            FieldRef::Float(x) => Value::Number(x),
            FieldRef::F64s(n) => Value::Numbers(n.to_vec()),
            FieldRef::Int(n) => Value::Integer(n),
            FieldRef::Bool(b) => Value::Bool(b),
            FieldRef::Text(t) => Value::Text(t.to_owned()),
            _ => return Err("is no parameter value".to_owned()),
        },
    })
}

/// A ramp as numbers: each key's position, value channels, interpolation.
fn ramp_numbers(r: &Ramp) -> Vec<f64> {
    let channels = r.channels();
    let mut data = Vec::with_capacity(r.keys.len() * (channels + 2));
    for k in &r.keys {
        data.push(k.position);
        data.extend_from_slice(&k.value[..channels]);
        let place = Interpolation::ALL
            .iter()
            .position(|i| *i == k.interpolation);
        data.push(place.unwrap_or(1) as f64);
    }
    data
}

/// The ramp `numbers` hold: `None` unless they are whole keys of a
/// `color` (or float) ramp, each with a known interpolation.
fn ramp_of(numbers: &[f64], color: bool) -> Option<Ramp> {
    let channels = if color { 3 } else { 1 };
    let width = channels + 2;
    if numbers.is_empty() || !numbers.len().is_multiple_of(width) {
        return None;
    }
    let keys = (numbers.chunks_exact(width))
        .map(|row| {
            let place = row[width - 1];
            let interpolation = *(place.fract() == 0.0 && place >= 0.0)
                .then(|| Interpolation::ALL.get(place as usize))
                .flatten()?;
            let mut value = [0.0; 3];
            value[..channels].copy_from_slice(&row[1..1 + channels]);
            Some(Key {
                position: row[0],
                value,
                interpolation,
            })
        })
        .collect::<Option<Vec<Key>>>()?;
    Some(Ramp { color, keys })
}

/// Tone curves as numbers: each its interpolation, its key count, its keys.
fn curve_numbers(curves: &[ToneCurve]) -> Vec<f64> {
    let mut out = Vec::new();
    for c in curves {
        out.push(match c.interpolation {
            tone_curve::Interpolation::Monotone => 0.0,
            tone_curve::Interpolation::Linear => 1.0,
        });
        out.push(c.keys.len() as f64);
        out.extend(c.keys.iter().flatten());
    }
    out
}

/// The tone curves [`curve_numbers`] wrote; `None` for numbers that aren't.
fn curves_of(mut numbers: &[f64]) -> Option<Vec<ToneCurve>> {
    let mut curves = Vec::new();
    while let [interpolation, count, rest @ ..] = numbers {
        let interpolation = match *interpolation {
            0.0 => tone_curve::Interpolation::Monotone,
            1.0 => tone_curve::Interpolation::Linear,
            _ => return None,
        };
        let count = (count.fract() == 0.0 && *count >= 0.0).then_some(*count as usize)?;
        let pairs = rest.get(..count.checked_mul(2)?)?;
        let keys = pairs.as_chunks::<2>().0.to_vec();
        curves.push(ToneCurve {
            keys,
            interpolation,
        });
        numbers = &rest[count * 2..];
    }
    numbers.is_empty().then_some(curves)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::MemoryElement;

    /// `values` written, read back by `settings`.
    fn back(values: &Parameters, settings: &[Parameter]) -> Result<Parameters, Error> {
        let mut e = MemoryElement::new("values");
        e.write(|out| write_values(out, values)).unwrap();
        e.read(|r| read_values(r, settings))
    }

    fn width() -> Parameter {
        let kind = ValueKind::Number(crate::params::Number::plain().range(0.0, 10.0));
        Parameter::new("width", "Width", kind, 1.0)
    }

    #[test]
    fn values_keep_what_no_descriptor_declares_and_save_it_back() {
        let mut values = Parameters::from([("width".to_owned(), Value::Number(2.0))]);
        let kept = [
            ("acmeGlow", Value::Number(2.5)),
            ("acmeSeed", Value::Integer(4)),
            ("acmeOn", Value::Bool(true)),
            ("acmeSide", Value::Text("lit".into())),
            ("acmeOffset", Value::Numbers(vec![1.0, 2.0])),
        ];
        values.extend(kept.map(|(name, v)| (name.to_owned(), v)));
        assert_eq!(back(&values, &[width()]).unwrap(), values);
        let ramp = Ramp::default_of(true);
        let kind = ValueKind::Ramp {
            color: true,
            most: None,
        };
        let d = Parameter::new("ramp", "Ramp", kind, ramp.clone());
        let values = Parameters::from([("ramp".to_owned(), Value::Ramp(ramp))]);
        assert_eq!(back(&values, &[d]).unwrap(), values);
    }

    #[test]
    fn a_missing_value_and_one_out_of_range_or_of_another_kind_are_refused() {
        let mut values = Parameters::from([("width".to_owned(), Value::Number(-1.0))]);
        let why = back(&values, &[width()]).unwrap_err().to_string();
        assert!(why.contains("width is out of range"), "{why}");
        values.remove("width");
        let why = back(&values, &[width()]).unwrap_err().to_string();
        assert!(why.contains("width is missing"), "{why}");
        values.insert("width".into(), Value::Text("wide".into()));
        let why = back(&values, &[width()]).unwrap_err().to_string();
        assert!(why.contains("width is not a float64"), "{why}");
    }

    #[test]
    fn an_object_value_saves_as_its_id_or_none() {
        let kind = ValueKind::Object(crate::params::RefFilter::objects([]));
        let dome = Parameter::new("dome", "Dome", kind, Value::Object(None));
        for held in [None, Some(ObjectRef(42))] {
            let values = Parameters::from([("dome".to_owned(), Value::Object(held))]);
            assert_eq!(back(&values, std::slice::from_ref(&dome)).unwrap(), values);
        }
        let wrong = Parameters::from([("dome".to_owned(), Value::Text("/Sky".into()))]);
        let why = back(&wrong, &[dome]).unwrap_err().to_string();
        assert!(why.contains("dome is not an object's id"), "{why}");
    }

    #[test]
    fn a_damaged_ramp_or_curve_is_refused() {
        assert!(
            curves_of(&[0.0, 2.0, 0.0, 0.0, 1.0]).is_none(),
            "a short curve"
        );
        assert!(curves_of(&[2.0, 0.0]).is_none(), "an unknown interpolation");
        assert!(
            ramp_of(&[0.0, 1.0, 9.0], false).is_none(),
            "an unknown interpolation"
        );
        assert!(
            ramp_of(&[0.0, 1.0, 1.0], true).is_none(),
            "a color ramp's key is five numbers"
        );
        assert!(ramp_of(&[], false).is_none(), "no key");
    }
}
