//! Saving a standard [`Selection`]: the fields `class`, `all`, `indices`,
//! `active` (−1 for none) and `topology` (the 32-byte fingerprint it was
//! made on, or no bytes).
use super::{attributes, components, invalid};
use crate::components::Components;
use crate::element::{ElementReader, ElementWriter, Field};
use crate::error::Error;
use crate::id::DataTypeId;
use kinogaki_geometry::{Class, Selection};

pub(super) fn write(components: &Components, out: &mut dyn ElementWriter) -> Result<bool, Error> {
    let Some(s) = components.selection() else {
        return Err(Error::invalid(
            "a standard type's components are a Selection",
        ));
    };
    out.set("class", Field::Text(s.class().name().into()))?;
    out.set("all", Field::Bool(s.is_all()))?;
    out.set("indices", Field::U32s(s.indices().to_vec()))?;
    out.set("active", Field::Int(s.active().map_or(-1, i64::from)))?;
    let topology = s.topology().map_or(Vec::new(), |t| t.to_vec());
    out.set("topology", Field::U8s(topology))?;
    Ok(true)
}

pub(super) fn read(
    from: &dyn ElementReader,
    ty: DataTypeId,
    count: impl Fn(Class) -> Option<usize>,
) -> Result<Components, Error> {
    let at = from.location();
    let name = from.text("class")?;
    let class = Class::from_name(name)
        .ok_or_else(|| Error::invalid(format_args!("{at}: no class {name:?}")))?;
    let elements = count(class)
        .ok_or_else(|| Error::invalid(format_args!("{at}: the value has no {name} class")))?;
    let topology = match attributes::u8s(from, "topology")? {
        [] => None,
        bytes => Some(
            <[u8; 32]>::try_from(bytes)
                .map_err(|_| Error::invalid(format_args!("{at}: a fingerprint is 32 bytes")))?,
        ),
    };
    let selection = if from.bool("all")? {
        Selection::all(class, topology)
    } else {
        let active =
            match from.int("active")? {
                -1 => None,
                a => Some(u32::try_from(a).map_err(|_| {
                    Error::invalid(format_args!("{at}: active {a} is not an element"))
                })?),
            };
        let indices = from.u32s("indices")?.to_vec();
        if let Some(&past) = indices.iter().find(|&&i| i as usize >= elements) {
            return Err(Error::invalid(format_args!(
                "{at}: selected {name} {past} is past the {elements} elements"
            )));
        }
        Selection::new(class, indices, active, topology).map_err(|e| invalid(&at, e))?
    };
    Ok(components(ty, selection, elements))
}
