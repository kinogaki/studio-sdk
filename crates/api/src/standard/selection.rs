//! Saving a standard [`Selection`]: the fields `domain`, its members as
//! `elements` and `words` (as a group saves them), `active` when one is,
//! `on` (the 32-byte fingerprint of the value it was made on, or no
//! bytes) and `ids` (the members' stable ids) when it holds them.
use super::{attributes, components, invalid};
use crate::components::Components;
use crate::element::{ElementReader, ElementWriter, Field};
use crate::error::Error;
use crate::id::DataTypeId;
use kinogaki_geometry::{Domain, Selection};

pub(super) fn write(components: &Components, out: &mut dyn ElementWriter) -> Result<bool, Error> {
    let Some(s) = components.selection() else {
        return Err(Error::invalid(
            "a standard type's components are a Selection",
        ));
    };
    out.set("domain", Field::Text(s.domain().name().into()))?;
    attributes::write_elements(s.members(), out)?;
    if let Some(active) = s.active() {
        out.set("active", Field::Int(i64::from(active)))?;
    }
    let on = s.made_on().map_or(Vec::new(), |t| t.to_vec());
    out.set("on", Field::U8s(on))?;
    if let Some(ids) = s.ids() {
        out.set("ids", Field::I64s(ids.to_vec()))?;
    }
    Ok(true)
}

pub(super) fn read(
    from: &dyn ElementReader,
    ty: DataTypeId,
    count: impl Fn(Domain) -> Option<usize>,
) -> Result<Components, Error> {
    let at = from.location();
    let name = from.text("domain")?;
    let domain = Domain::from_name(name)
        .ok_or_else(|| Error::invalid(format_args!("{at}: no domain {name:?}")))?;
    let elements = count(domain)
        .ok_or_else(|| Error::invalid(format_args!("{at}: the value has no {name} domain")))?;
    let members = attributes::elements(from)?;
    if members.count() != elements {
        return Err(Error::invalid(format_args!(
            "{at}: a selection of {} {name} elements on a value of {elements}",
            members.count()
        )));
    }
    let on = match attributes::u8s(from, "on")? {
        [] => None,
        bytes => Some(
            <[u8; 32]>::try_from(bytes)
                .map_err(|_| Error::invalid(format_args!("{at}: a fingerprint is 32 bytes")))?,
        ),
    };
    let active =
        match from.field("active") {
            None => None,
            Some(_) => {
                let a = from.int("active")?;
                Some(u32::try_from(a).map_err(|_| {
                    Error::invalid(format_args!("{at}: active {a} is not an element"))
                })?)
            }
        };
    let ids = match from.field("ids") {
        None => None,
        Some(_) => Some(attributes::i64s(from, "ids")?.to_vec().into()),
    };
    let selection = Selection::new(domain, members, active)
        .and_then(|s| s.restored(on, ids))
        .map_err(|e| invalid(&at, e))?;
    Ok(components(ty, selection))
}
