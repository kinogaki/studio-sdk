//! The attribute model every standard value shares: named, typed columns
//! per element domain ([`Attribute`] of an [`AttributeType`]), named sets
//! of elements ([`Elements`]), and the table of both a value holds
//! ([`Attributes`]).
//!
//! An [`Attribute`] checks its own values once, when it is made. An
//! [`Attributes`] table checks names and the [standard names](names)'
//! types and domains as entries are added. The value that holds it checks
//! the rest against its own counts, for the entries it doesn't already
//! hold: each attribute and group has exactly one entry per element of
//! its domain ([`Geometry`](crate::Geometry), [`Body`](crate::brep::Body)).
mod attribute;
mod domain;
mod ids;
pub use ids::highest_id;
pub mod names;
mod text;
mod types;

pub use attribute::{Attribute, Tuple, Values};
pub use domain::Domain;
pub use names::check_name;
pub use text::Text;
pub use types::{AttributeType, Meaning, Storage};

use crate::Error;
use crate::elements::Elements;
use names::Standard;
use std::collections::BTreeMap;
use std::sync::Arc;

type Named<T> = BTreeMap<Domain, BTreeMap<Arc<str>, Arc<T>>>;

/// A value's attributes and groups, by domain and name.
///
/// Immutable and shared: [`with`](Self::with), [`without`](Self::without)
/// and [`with_group`](Self::with_group) return a new table sharing every
/// entry they didn't touch, and check only the entry they add. A table
/// being made adds its entries in place ([`set`](Self::set),
/// [`set_group`](Self::set_group)), copying nothing, so a reader adding
/// many takes time in proportion to them. Attributes and groups are two
/// namespaces.
///
/// **Rules** (checked as entries are added): every name passes
/// [`check_name`]; a [standard name](names) has its standard type and
/// lives only on its domains; `P` is never an attribute (positions are the
/// value's own column); the stable `id` names each element of its domain
/// once (checked in parallel, only when the column is added).
///
/// ```
/// use kinogaki_geometry::{Attribute, Attributes, Domain, Meaning, names};
///
/// let colors = Attribute::tuples(vec![[1.0f32, 0.0, 0.0]; 4], Meaning::Color).unwrap();
/// let table = Attributes::new().with(Domain::Point, names::CD, colors).unwrap();
/// assert_eq!(table.tuples::<[f32; 3]>(Domain::Point, "Cd").map(|c| c.len()), Some(4));
/// // A standard name keeps its type.
/// let reals = Attribute::tuples(vec![[1.0f64, 0.0, 0.0]; 4], Meaning::Color).unwrap();
/// assert!(table.with(Domain::Point, names::CD, reals).is_err());
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Attributes {
    values: Named<Attribute>,
    groups: Named<Elements>,
}

impl Attributes {
    /// No attributes and no groups.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attribute `name` of `domain`.
    pub fn get(&self, domain: Domain, name: &str) -> Option<&Attribute> {
        self.values.get(&domain)?.get(name).map(|a| &**a)
    }

    /// Attribute `name` of `domain`, shared.
    pub(crate) fn get_shared(&self, domain: Domain, name: &str) -> Option<&Arc<Attribute>> {
        self.values.get(&domain)?.get(name)
    }

    /// Group `name` of `domain`.
    pub fn group(&self, domain: Domain, name: &str) -> Option<&Elements> {
        self.groups.get(&domain)?.get(name).map(|g| &**g)
    }

    /// Group `name` of `domain`, shared: what a cache compares by
    /// identity.
    pub(crate) fn group_shared(&self, domain: Domain, name: &str) -> Option<&Arc<Elements>> {
        self.groups.get(&domain)?.get(name)
    }

    /// Every attribute, by domain then name.
    pub fn iter(&self) -> impl Iterator<Item = (Domain, &str, &Attribute)> {
        self.values
            .iter()
            .flat_map(|(&d, m)| m.iter().map(move |(n, a)| (d, &**n, &**a)))
    }

    /// Every group, by domain then name.
    pub fn groups(&self) -> impl Iterator<Item = (Domain, &str, &Elements)> {
        self.groups
            .iter()
            .flat_map(|(&d, m)| m.iter().map(move |(n, g)| (d, &**n, &**g)))
    }

    /// Whether it holds no attribute and no group.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty() && self.groups.is_empty()
    }

    /// Whether it holds an attribute or a group on `domain`.
    pub fn has(&self, domain: Domain) -> bool {
        self.values.contains_key(&domain) || self.groups.contains_key(&domain)
    }

    /// The table with attribute `name` of `domain` set to `attribute`
    /// (replacing one so named), sharing the rest. Only this entry is
    /// checked.
    ///
    /// # Errors
    /// A bad name, or a standard name of another type or domain.
    pub fn with(
        &self,
        domain: Domain,
        name: &str,
        attribute: Attribute,
    ) -> Result<Attributes, Error> {
        self.with_shared(domain, name, Arc::new(attribute))
    }

    /// Sets attribute `name` of `domain` to `attribute` in this table
    /// (replacing one so named), as [`with`](Self::with) sets it in a new
    /// one.
    ///
    /// # Errors
    /// As [`with`](Self::with); the table is left as it was.
    pub fn set(&mut self, domain: Domain, name: &str, attribute: Attribute) -> Result<(), Error> {
        self.set_shared(domain, name, Arc::new(attribute))
    }

    /// The table with `attribute`, read from an attribute of a checked table
    /// (a render's domain over a value's), set as `name` of `domain`: its
    /// type and domain are checked as [`with`](Self::with) checks them, but
    /// its elements may repeat a value (the triangles of one polygon share
    /// its `id`).
    ///
    /// # Errors
    /// A bad name, or a standard name of another type or domain.
    pub fn with_read(
        &self,
        domain: Domain,
        name: &str,
        attribute: Attribute,
    ) -> Result<Attributes, Error> {
        check_name(name)?;
        check_standard_type(domain, name, &attribute)?;
        let mut out = self.clone();
        out.values
            .entry(domain)
            .or_default()
            .insert(name.into(), Arc::new(attribute));
        Ok(out)
    }

    /// Whether `other` holds the same attributes in the same columns:
    /// nothing a reader of its values could tell apart, read in O(names).
    pub fn same_columns(&self, other: &Attributes) -> bool {
        let keyed = |a: &'_ Attributes| -> Vec<(Domain, Arc<str>, u64)> {
            (a.values.iter())
                .flat_map(|(&d, m)| m.iter().map(move |(n, a)| (d, n.clone(), a.values().key())))
                .collect()
        };
        keyed(self) == keyed(other)
    }

    /// [`with`](Self::with) for an attribute shared with another table:
    /// both hold the same one.
    ///
    /// # Errors
    /// As [`with`](Self::with).
    pub(crate) fn with_shared(
        &self,
        domain: Domain,
        name: &str,
        attribute: Arc<Attribute>,
    ) -> Result<Attributes, Error> {
        let mut out = self.clone();
        out.set_shared(domain, name, attribute)?;
        Ok(out)
    }

    /// [`set`](Self::set) for an attribute shared with another table.
    fn set_shared(
        &mut self,
        domain: Domain,
        name: &str,
        attribute: Arc<Attribute>,
    ) -> Result<(), Error> {
        check_name(name)?;
        check_standard_attribute(domain, name, &attribute, self.get(domain, name))?;
        (self.values.entry(domain).or_default()).insert(name.into(), attribute);
        Ok(())
    }

    /// The table without attribute `name` of `domain` (nothing changes
    /// when there is none).
    pub fn without(&self, domain: Domain, name: &str) -> Attributes {
        let mut out = self.clone();
        remove(&mut out.values, domain, name);
        out
    }

    /// The table without group `name` of `domain` (nothing changes when there
    /// is none).
    pub fn without_group(&self, domain: Domain, name: &str) -> Attributes {
        let mut out = self.clone();
        remove(&mut out.groups, domain, name);
        out
    }

    /// The table with group `name` of `domain` set to `group`.
    ///
    /// # Errors
    /// A bad name, or a standard group name on another domain (`sharp` is
    /// an Edge group, `.hidden` a Primitive group).
    pub fn with_group(
        &self,
        domain: Domain,
        name: &str,
        group: Elements,
    ) -> Result<Attributes, Error> {
        let mut out = self.clone();
        out.set_group(domain, name, group)?;
        Ok(out)
    }

    /// Sets group `name` of `domain` to `group` in this table, as
    /// [`with_group`](Self::with_group) sets it in a new one.
    ///
    /// # Errors
    /// As [`with_group`](Self::with_group); the table is left as it was.
    pub fn set_group(&mut self, domain: Domain, name: &str, group: Elements) -> Result<(), Error> {
        check_name(name)?;
        match names::standard(name) {
            Some(Standard::Group(must)) if must != domain => {
                return Err(refused(format!(
                    "{name} is a group of the {} domain, not of the {} domain",
                    must.name(),
                    domain.name()
                )));
            }
            Some(Standard::Attribute(..)) => {
                return Err(refused(format!(
                    "{name} is a standard attribute, not a group"
                )));
            }
            _ => {}
        }
        (self.groups.entry(domain).or_default()).insert(name.into(), Arc::new(group));
        Ok(())
    }

    /// Attribute `name` of `domain`'s items as `T`, when it holds that
    /// storage and width ([`Attribute::as_tuples`]).
    pub fn tuples<T: Tuple>(&self, domain: Domain, name: &str) -> Option<&[T]> {
        self.get(domain, name)?.as_tuples()
    }

    /// Text `name` of `domain`, when it holds text.
    pub fn text(&self, domain: Domain, name: &str) -> Option<&Text> {
        self.get(domain, name)?.as_text()
    }

    /// Checks each entry that `previous` (the table the value held
    /// before) doesn't hold as the same shared entry: its element count
    /// against `count(domain)` (`None` for a domain the value doesn't
    /// have, which no entry may use). `what` names the value in messages.
    /// So an edit replacing one column checks that column alone. Returns
    /// how many entries it checked.
    ///
    /// # Errors
    /// An entry on a domain the value doesn't have, or of another count.
    pub fn check(
        &self,
        what: &str,
        previous: Option<&Attributes>,
        mut count: impl FnMut(Domain) -> Option<usize>,
    ) -> Result<usize, Error> {
        let mut checked = 0;
        let domains: std::collections::BTreeSet<&Domain> =
            self.values.keys().chain(self.groups.keys()).collect();
        for domain in domains {
            let Some(expected) = count(*domain) else {
                return Err(refused(format!("{what} have no {} domain", domain.name())));
            };
            let was = previous.and_then(|p| p.values.get(domain));
            for (name, a) in self.values.get(domain).into_iter().flatten() {
                if was.is_some_and(|m| m.get(name).is_some_and(|e| Arc::ptr_eq(e, a))) {
                    continue;
                }
                checked += 1;
                if a.elements() != expected {
                    return Err(missized(*domain, name, "attribute", a.elements(), expected));
                }
            }
            let was = previous.and_then(|p| p.groups.get(domain));
            for (name, g) in self.groups.get(domain).into_iter().flatten() {
                if was.is_some_and(|m| m.get(name).is_some_and(|e| Arc::ptr_eq(e, g))) {
                    continue;
                }
                checked += 1;
                if g.count() != expected {
                    return Err(missized(*domain, name, "group", g.count(), expected));
                }
            }
        }
        Ok(checked)
    }
}

fn remove<T>(map: &mut Named<T>, domain: Domain, name: &str) {
    if let Some(named) = map.get_mut(&domain) {
        named.remove(name);
        if named.is_empty() {
            map.remove(&domain);
        }
    }
}

fn refused(why: String) -> Error {
    Error::invalid(why)
}

fn missized(domain: Domain, name: &str, what: &str, len: usize, expected: usize) -> Error {
    refused(format!(
        "{} {what} {name} has {len} values for {expected} elements",
        domain.name()
    ))
}

fn check_standard_attribute(
    domain: Domain,
    name: &str,
    attribute: &Attribute,
    replacing: Option<&Attribute>,
) -> Result<(), Error> {
    check_standard_type(domain, name, attribute)?;
    if name != names::ID {
        return Ok(());
    }
    let repeated = match (attribute.values(), replacing.map(Attribute::values)) {
        (Values::I64(new), Some(Values::I64(old))) if ids::kept_or_above(new, old) => None,
        (Values::I64(ids), _) => ids::repeated(ids.as_slice()),
        _ => None,
    };
    match repeated {
        Some(id) => Err(refused(format!(
            "{} id {id} names two elements",
            domain.name()
        ))),
        None => Ok(()),
    }
}

/// A standard name's type and domains.
fn check_standard_type(domain: Domain, name: &str, attribute: &Attribute) -> Result<(), Error> {
    match names::standard(name) {
        Some(Standard::Group(on)) => Err(refused(format!(
            "{name} is a standard group of the {} domain",
            on.name()
        ))),
        Some(Standard::Attribute(_, [])) => Err(refused(format!(
            "{name} is the value's own column, never an attribute"
        ))),
        Some(Standard::Attribute(_, domains)) if !domains.contains(&domain) => {
            Err(refused(format!(
                "{name} is not an attribute of the {} domain (only {})",
                domain.name(),
                domains
                    .iter()
                    .map(|d| d.name())
                    .collect::<Vec<_>>()
                    .join(", ")
            )))
        }
        Some(Standard::Attribute(ty, _)) if attribute.ty() != ty => Err(refused(format!(
            "{name} is {}, not {}",
            ty.describe(),
            attribute.ty().describe()
        ))),
        _ => Ok(()),
    }
}

#[cfg(test)]
mod tests;
