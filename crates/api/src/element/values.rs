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
//! | Texts | [`Texts`](Field::Texts) |
//! | Ramp | [`F64s`](Field::F64s): a key after another, each its position, its value (one channel, or three) and its interpolation's place in `Interpolation::ALL` |
//! | ToneCurves | [`F64s`](Field::F64s): a curve after another, its interpolation (0 monotone, 1 linear), its key count, then its keys' x, y |
//!
//! A value reads back by its declared kind; one no descriptor declares reads
//! back by its field's type alone, and writes back the same.
use super::{Field, FieldRef};
use crate::Error;
use crate::params::ramp::{Interpolation, Key, Ramp};
use crate::params::tone_curve::{self, ToneCurve};
use crate::params::{Value, ValueKind};

/// `value` as a field (named `name`, for what it says).
///
/// # Errors
/// An object (it saves with its kind's codec), a ramp without keys, or data
/// (it saves as an element of its own).
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
        Value::Texts(t) => Field::Texts(t.clone()),
        Value::Object(_) => return Err(refused("an object saves with its kind's codec")),
        Value::Ramp(r) if r.keys.is_empty() => return Err(refused("a ramp with no key")),
        Value::Ramp(r) => Field::F64s(ramp_numbers(r)),
        Value::ToneCurves(curves) => Field::F64s(curve_numbers(curves)),
        Value::Data(_) => return Err(refused("data saves as a data element, not a field")),
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
    let texts = || match field {
        FieldRef::Texts(t) => Ok(t.to_vec()),
        _ => Err(wrong("a list of strings")),
    };
    Ok(match kind {
        Some(K::Number(_)) => Value::Number(number()?),
        Some(K::Integer { .. }) => Value::Integer(integer()?),
        Some(K::Toggle) => Value::Bool(boolean()?),
        Some(K::Choice { .. }) => Value::Choice(text()?.into()),
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
        Some(K::Color) => match *numbers()? {
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
        Some(K::Texts) => Value::Texts(texts()?),
        Some(K::Object(_)) => return Err(wrong("a node's or a tool's value")),
        Some(K::Data(_)) => return Err(wrong("a data element")),
        None => match field {
            FieldRef::Float(x) => Value::Number(x),
            FieldRef::F64s(n) => Value::Numbers(n.to_vec()),
            FieldRef::Int(n) => Value::Integer(n),
            FieldRef::Bool(b) => Value::Bool(b),
            FieldRef::Text(t) => Value::Text(t.to_owned()),
            FieldRef::Texts(t) => Value::Texts(t.to_vec()),
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
