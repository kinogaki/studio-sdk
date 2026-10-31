//! Saving an [`Attributes`] table: one child element per attribute
//! (`attribute-0`, `attribute-1`…, kind `attribute`) and per group
//! (`group-0`…, kind `group`), in the table's order.
//!
//! An attribute's element has `name`, `class`, `kind` and `info` (their
//! names: [`Class::name`], [`AttributeKind::name`], [`TypeInfo::name`]) and
//! its `values`: reals, whole numbers, vectors as rows of 3, or text as
//! `values` (indices, `u32::MAX` for no value) plus `table`. A group's
//! element has `name`, `class`, `elements` and its bit `words` (each `u64`
//! as the `i64` with the same bits).
use super::invalid;
use crate::element::{ElementReader, ElementWriter, Field, FieldRef};
use crate::error::Error;
use kinogaki_geometry::{
    Attribute, AttributeValues, Attributes, Class, Column, Group, Text, TypeInfo,
};
use std::sync::Arc;

pub(super) fn write(attributes: &Attributes, out: &mut dyn ElementWriter) -> Result<(), Error> {
    for (i, (class, name, attribute)) in attributes.iter().enumerate() {
        let mut child = out.child(&format!("attribute-{i}"), "attribute")?;
        child.set("name", Field::Text(name.into()))?;
        child.set("class", Field::Text(class.name().into()))?;
        child.set("kind", Field::Text(attribute.kind().name().into()))?;
        child.set("info", Field::Text(attribute.info().name().into()))?;
        match attribute.values() {
            AttributeValues::Float(c) => child.set("values", Field::F64s(c.to_vec()))?,
            AttributeValues::Integer(c) => child.set("values", Field::I64s(c.to_vec()))?,
            AttributeValues::Vector(c) => child.set_rows("values", 3, Field::F64s(flat(c)))?,
            AttributeValues::Text(t) => {
                child.set("values", Field::U32s(t.indices().to_vec()))?;
                let table = t.table().iter().map(|s| s.to_string()).collect();
                child.set("table", Field::Texts(table))?;
            }
            _ => {
                return Err(Error::invalid(format_args!(
                    "attribute {name} is of a kind this version can't save"
                )));
            }
        }
    }
    for (i, (class, name, group)) in attributes.groups().enumerate() {
        let mut child = out.child(&format!("group-{i}"), "group")?;
        child.set("name", Field::Text(name.into()))?;
        child.set("class", Field::Text(class.name().into()))?;
        child.set("elements", Field::Int(group.elements() as i64))?;
        let words = group.words().iter().map(|&w| w as i64).collect();
        child.set("words", Field::I64s(words))?;
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
                let (class, name) = (class(&*child)?, child.text("name")?);
                let attribute = read_attribute(&*child)?;
                attributes = attributes
                    .with(class, name, attribute)
                    .map_err(|e| invalid(&at, e))?;
            }
            "group" => {
                let (class, name) = (class(&*child)?, child.text("name")?);
                let elements = count(&*child, "elements")?;
                let words: Vec<u64> = i64s(&*child, "words")?.iter().map(|&w| w as u64).collect();
                let group =
                    Group::new(elements, Column::new(words)).map_err(|e| invalid(&at, e))?;
                attributes = attributes
                    .with_group(class, name, group)
                    .map_err(|e| invalid(&at, e))?;
            }
            _ => {}
        }
    }
    Ok(attributes)
}

fn read_attribute(child: &dyn ElementReader) -> Result<Attribute, Error> {
    let at = child.location();
    let info_name = child.text("info")?;
    let info = TypeInfo::from_name(info_name)
        .ok_or_else(|| Error::invalid(format_args!("{at}: no type info {info_name:?}")))?;
    let values = match child.text("kind")? {
        "float" => AttributeValues::Float(child.f64s("values")?.to_vec().into()),
        "integer" => AttributeValues::Integer(i64s(child, "values")?.to_vec().into()),
        "vector" => AttributeValues::Vector(rows(child, "values")?.into()),
        "text" => {
            let table: Arc<[Arc<str>]> = child
                .texts("table")?
                .iter()
                .map(|s| Arc::from(s.as_str()))
                .collect();
            let indices = child.u32s("values")?.to_vec().into();
            AttributeValues::Text(Text::new(indices, table).map_err(|e| invalid(&at, e))?)
        }
        other => {
            return Err(Error::invalid(format_args!(
                "{at}: no attribute kind {other:?}"
            )));
        }
    };
    Ok(Attribute::new(values, info))
}

fn class(child: &dyn ElementReader) -> Result<Class, Error> {
    let name = child.text("class")?;
    Class::from_name(name)
        .ok_or_else(|| Error::invalid(format_args!("{}: no class {name:?}", child.location())))
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
