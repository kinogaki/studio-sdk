//! [`reblend`]: some elements' attributes and groups made again from their
//! sources, as an [`Editor`](super::Editor) made them, in place.
use super::carry::{blend_real, default_item, rule, strongest_item};
use super::layout::{Source, strongest};
use kinogaki_geometry::{
    Attribute, Column, ColumnWriter, Domain, Elements, Error, Geometry, Text, Values, names,
};

/// `value` with the `domain` attributes and groups of `elements` made
/// again from `from`'s elements of `domain`: each element listed with its
/// sources there (element and weight), its values blended by its
/// attribute's [`rule`] and its groups those of its strongest source,
/// exactly as an edit of `from` makes a new element. What a step replayed
/// at other rows writes when what its new elements take moves with its
/// new points (a bevel's corners). Only the cells of the listed elements
/// are copied; stable ids stay.
///
/// # Errors
/// An attribute of arrays (its strongest source decides its length: such
/// a step runs again), or one `from` lacks or holds as another type.
pub fn reblend(
    value: &Geometry,
    from: &Geometry,
    domain: Domain,
    elements: &[(u32, Vec<Source>)],
) -> Result<Geometry, Error> {
    if elements.is_empty() {
        return Ok(value.clone());
    }
    let (onto, sources) = (value.attributes(), from.attributes());
    let mut out = (**onto).clone();
    for (d, name, attribute) in onto.iter().filter(|a| a.0 == domain && a.1 != names::ID) {
        let was = (sources.get(d, name))
            .filter(|was| was.ty() == attribute.ty() && was.offsets().is_none())
            .ok_or_else(|| Error::refused(format!("{name} can't be blended again in place")))?;
        if attribute.offsets().is_some() {
            return Err(Error::refused(format!("{name} holds arrays")));
        }
        out = out.with(d, name, reblended(attribute, was, elements)?)?;
    }
    for (d, name, group) in onto.groups().filter(|g| g.0 == domain) {
        let was = sources.group(d, name);
        let member =
            |list: &[Source]| strongest(list).is_some_and(|e| was.is_some_and(|g| g.contains(e)));
        let mut members: Vec<u32> = group.iter().collect();
        for (element, list) in elements {
            match (members.binary_search(element), member(list)) {
                (Err(at), true) => members.insert(at, *element),
                (Ok(at), false) => _ = members.remove(at),
                _ => {}
            }
        }
        out = out.with_group(d, name, Elements::from_indices(group.count(), members)?)?;
    }
    value.with_attributes(out)
}

/// `attribute`'s values with `elements`' made again from `was`'s.
fn reblended(
    attribute: &Attribute,
    was: &Attribute,
    elements: &[(u32, Vec<Source>)],
) -> Result<Attribute, Error> {
    let ty = attribute.ty();
    let width = usize::from(ty.width);
    let (rule, default) = (rule(ty), default_item(ty));
    let values = match (attribute.values(), was.values()) {
        (Values::F32(onto), Values::F32(from)) => {
            Values::F32(written(onto, width, elements, |s, out| {
                blend_real(from, width, rule, s, &default, out)
            }))
        }
        (Values::F64(onto), Values::F64(from)) => {
            Values::F64(written(onto, width, elements, |s, out| {
                blend_real(from, width, rule, s, &default, out)
            }))
        }
        (Values::I32(onto), Values::I32(from)) => {
            Values::I32(written(onto, width, elements, |s, out| {
                strongest_item(from, width, s, 0, out)
            }))
        }
        (Values::I64(onto), Values::I64(from)) => {
            Values::I64(written(onto, width, elements, |s, out| {
                strongest_item(from, width, s, 0, out)
            }))
        }
        (Values::Text(onto), Values::Text(from)) if onto.table() == from.table() => {
            let indices = written(onto.indices(), 1, elements, |s, out| {
                strongest_item(from.indices(), 1, s, Text::NONE, out)
            });
            return Ok(Attribute::text(onto.with_indices(indices)?));
        }
        _ => {
            return Err(Error::refused(
                "values that can't be blended again in place",
            ));
        }
    };
    attribute.with_values(values)
}

/// `values` (`width` a item) with each of `elements`' items written over by
/// `item` from its sources.
fn written<T: Copy>(
    values: &Column<T>,
    width: usize,
    elements: &[(u32, Vec<Source>)],
    item: impl Fn(&[Source], &mut ColumnWriter<T>),
) -> Column<T> {
    let mut out = values.edit();
    for (element, sources) in elements {
        let mut made = ColumnWriter::new();
        item(sources, &mut made);
        let at = *element as usize * width;
        for (k, value) in made.finish().iter().enumerate() {
            out[at + k] = *value;
        }
    }
    out.finish()
}
