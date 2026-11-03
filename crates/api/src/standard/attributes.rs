//! Saving an [`Attributes`] table: one child element per attribute
//! (`a0`, `a1`…, kind `attribute`) and per group (`g0`…, kind `group`),
//! in the table's order.
//!
//! An attribute's element has `name`, `domain`, `storage`, `meaning`
//! (their names: [`Domain::name`], [`Storage::name`], [`Meaning::name`]),
//! `width` and its `values` in the storage's array (`F32s`, `F64s`,
//! `I32s`, `I64s`; rows of `width`), or for text `values` as indices
//! (`u32::MAX` for no value) plus `table`, the strings used in the order of
//! first use ([`Text::compacted`]). An array attribute adds `offsets`
//! (`U32s`); one without has none. A group's element has `name`, `domain`,
//! `elements` and its bit `words` (each `u64` as the `i64` with the same
//! bits). Reading is strict: a standard name of another type or domain
//! refuses the file, with the element's path.
use super::invalid;
use crate::element::{ElementReader, ElementWriter, Field, FieldRef};
use crate::error::Error;
use kinogaki_geometry::{
    Attribute, AttributeType, Attributes, Column, Domain, Elements, Meaning, Storage, Text, Values,
};
use std::sync::Arc;

pub(super) fn write(attributes: &Attributes, out: &mut dyn ElementWriter) -> Result<(), Error> {
    for (k, (domain, name, attribute)) in attributes.iter().enumerate() {
        let mut child = out.child(&format!("a{k}"), "attribute")?;
        let ty = attribute.ty();
        child.set("name", Field::Text(name.into()))?;
        child.set("domain", Field::Text(domain.name().into()))?;
        child.set("storage", Field::Text(ty.storage.name().into()))?;
        child.set("width", Field::Int(i64::from(ty.width)))?;
        child.set("meaning", Field::Text(ty.meaning.name().into()))?;
        let values = match attribute.values() {
            Values::F32(c) => Field::F32s(c.to_vec()),
            Values::F64(c) => Field::F64s(c.to_vec()),
            Values::I32(c) => Field::I32s(c.to_vec()),
            Values::I64(c) => Field::I64s(c.to_vec()),
            Values::Text(t) => {
                // The canonical table, so equal texts save alike.
                let t = t.compacted();
                let table = t.table().iter().map(|s| s.to_string()).collect();
                child.set("table", Field::Texts(table))?;
                Field::U32s(t.indices().to_vec())
            }
            _ => {
                return Err(Error::invalid(format_args!(
                    "attribute {name} is of a storage this version can't save"
                )));
            }
        };
        match ty.width {
            1 => child.set("values", values)?,
            width => child.set_rows("values", u32::from(width), values)?,
        }
        if let Some(offsets) = attribute.offsets() {
            child.set("offsets", Field::U32s(offsets.to_vec()))?;
        }
    }
    for (k, (domain, name, group)) in attributes.groups().enumerate() {
        let mut child = out.child(&format!("g{k}"), "group")?;
        child.set("name", Field::Text(name.into()))?;
        child.set("domain", Field::Text(domain.name().into()))?;
        write_elements(group, &mut *child)?;
    }
    Ok(())
}

pub(super) fn read(from: &dyn ElementReader) -> Result<Attributes, Error> {
    let mut attributes = Attributes::new();
    for name in from.children() {
        let Some(child) = from.child(&name) else {
            continue;
        };
        let at = child.location();
        match child.kind() {
            "attribute" => {
                let (domain, name) = (domain(&*child)?, child.text("name")?);
                let attribute = read_attribute(&*child)?;
                attributes = attributes
                    .with(domain, name, attribute)
                    .map_err(|e| invalid(&at, e))?;
            }
            "group" => {
                let (domain, name) = (domain(&*child)?, child.text("name")?);
                let group = elements(&*child)?;
                attributes = attributes
                    .with_group(domain, name, group)
                    .map_err(|e| invalid(&at, e))?;
            }
            _ => {}
        }
    }
    Ok(attributes)
}

/// The set `elements` and `words` say (a group's or a selection's).
pub(super) fn elements(from: &dyn ElementReader) -> Result<Elements, Error> {
    let count = count(from, "elements")?;
    let words: Vec<u64> = i64s(from, "words")?.iter().map(|&w| w as u64).collect();
    Elements::from_bits(count, Column::new(words)).map_err(|e| invalid(&from.location(), e))
}

/// Writes `set` as `elements` (its count) and its bit `words`.
pub(super) fn write_elements(set: &Elements, out: &mut dyn ElementWriter) -> Result<(), Error> {
    out.set("elements", Field::Int(set.count() as i64))?;
    let words = set.words().iter().map(|&w| w as i64).collect();
    out.set("words", Field::I64s(words))
}

fn read_attribute(child: &dyn ElementReader) -> Result<Attribute, Error> {
    let at = child.location();
    let unknown = |field: &str| {
        let name = child.text(field).unwrap_or_default();
        Error::invalid(format_args!("{at}: no {field} {name:?}"))
    };
    let storage = Storage::from_name(child.text("storage")?).ok_or_else(|| unknown("storage"))?;
    let meaning = Meaning::from_name(child.text("meaning")?).ok_or_else(|| unknown("meaning"))?;
    let width = u8::try_from(child.int("width")?)
        .map_err(|_| Error::invalid(format_args!("{at}: width is not 1 to 255")))?;
    let offsets = match child.field("offsets") {
        Some(_) => Some(Column::new(child.u32s("offsets")?.to_vec())),
        None => None,
    };
    let ty = AttributeType {
        storage,
        width,
        meaning,
        array: offsets.is_some(),
    };
    let wrong = || {
        Error::invalid(format_args!(
            "{at}: values is not an array of {}",
            storage.name()
        ))
    };
    let values = match (storage, child.required("values")?) {
        (Storage::F32, FieldRef::F32s(v)) => Values::F32(v.to_vec().into()),
        (Storage::F64, FieldRef::F64s(v)) => Values::F64(v.to_vec().into()),
        (Storage::I32, FieldRef::I32s(v)) => Values::I32(v.to_vec().into()),
        (Storage::I64, FieldRef::I64s(v)) => Values::I64(v.to_vec().into()),
        (Storage::Text, FieldRef::U32s(v)) => {
            let table: Arc<[Arc<str>]> = (child.texts("table")?.iter())
                .map(|s| Arc::from(s.as_str()))
                .collect();
            Values::Text(Text::new(v.to_vec().into(), table).map_err(|e| invalid(&at, e))?)
        }
        _ => return Err(wrong()),
    };
    match offsets {
        Some(offsets) => Attribute::arrays(ty, offsets, values),
        None => Attribute::new(ty, values),
    }
    .map_err(|e| invalid(&at, e))
}

fn domain(child: &dyn ElementReader) -> Result<Domain, Error> {
    let name = child.text("domain")?;
    Domain::from_name(name)
        .ok_or_else(|| Error::invalid(format_args!("{}: no domain {name:?}", child.location())))
}

/// Vectors as a flat array, three reals each.
pub(super) fn flat(values: &[[f64; 3]]) -> Vec<f64> {
    values.iter().flatten().copied().collect()
}

/// Field `name` read as rows of three reals.
pub(super) fn rows(from: &dyn ElementReader, name: &str) -> Result<Vec<[f64; 3]>, Error> {
    let flat = from.f64s(name)?;
    if flat.len() % 3 != 0 {
        return Err(Error::invalid(format_args!(
            "{}: {name} is not rows of 3",
            from.location()
        )));
    }
    Ok(flat.as_chunks::<3>().0.to_vec())
}

/// Whole-number array field `name`.
pub(super) fn i64s<'a>(from: &'a dyn ElementReader, name: &str) -> Result<&'a [i64], Error> {
    match from.required(name)? {
        FieldRef::I64s(v) => Ok(v),
        _ => Err(Error::invalid(format_args!(
            "{}: {name} is not an array of i64",
            from.location()
        ))),
    }
}

/// Byte array field `name`.
pub(super) fn u8s<'a>(from: &'a dyn ElementReader, name: &str) -> Result<&'a [u8], Error> {
    match from.required(name)? {
        FieldRef::U8s(v) => Ok(v),
        _ => Err(Error::invalid(format_args!(
            "{}: {name} is not an array of bytes",
            from.location()
        ))),
    }
}

/// Whole-number field `name` as a count.
pub(super) fn count(from: &dyn ElementReader, name: &str) -> Result<usize, Error> {
    let n = from.int(name)?;
    usize::try_from(n)
        .map_err(|_| Error::invalid(format_args!("{}: {name} is negative", from.location())))
}
