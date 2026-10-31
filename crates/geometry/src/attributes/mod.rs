//! The attribute model every standard value shares: named, typed columns
//! per element class ([`Attribute`]), named sets of elements ([`Group`]),
//! and the table of both a value holds ([`Attributes`]).
//!
//! An [`Attributes`] table checks names and standard types. The value that
//! holds it checks the rest, against its own counts: each attribute and
//! group has exactly one entry per element of its class
//! ([`Polygons`](crate::Polygons), [`Curves`](crate::Curves)).
mod class;
mod group;
pub mod names;
mod text;

pub use class::{Class, TypeInfo};
pub use group::{Group, GroupBuilder};
pub use names::check_name;
pub use text::{Text, TextBuilder};

use crate::column::Column;
use crate::error::{ErrorKind, GeometryError};
use names::Standard;
use std::collections::BTreeMap;
use std::sync::Arc;

/// An attribute's values: one per element of its class.
///
/// New kinds may be added. A reader that doesn't know a kind leaves the
/// attribute as it is: it passes through edits unchanged and shows as
/// unreadable.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum AttributeValues {
    /// Real numbers.
    Float(Column<f64>),
    /// Whole numbers.
    Integer(Column<i64>),
    /// Three reals each (positions, normals, colors, texture coordinates).
    Vector(Column<[f64; 3]>),
    /// Indexed text.
    Text(Text),
}

/// What kind of values an attribute holds ([`AttributeValues`] without
/// the values).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum AttributeKind {
    /// [`AttributeValues::Float`].
    Float,
    /// [`AttributeValues::Integer`].
    Integer,
    /// [`AttributeValues::Vector`].
    Vector,
    /// [`AttributeValues::Text`].
    Text,
}

impl AttributeKind {
    /// Its name in messages and files: `float`, `integer`, `vector`,
    /// `text`. Never renamed.
    pub fn name(self) -> &'static str {
        match self {
            AttributeKind::Float => "float",
            AttributeKind::Integer => "integer",
            AttributeKind::Vector => "vector",
            AttributeKind::Text => "text",
        }
    }
}

impl AttributeValues {
    /// Their kind.
    pub fn kind(&self) -> AttributeKind {
        match self {
            AttributeValues::Float(_) => AttributeKind::Float,
            AttributeValues::Integer(_) => AttributeKind::Integer,
            AttributeValues::Vector(_) => AttributeKind::Vector,
            AttributeValues::Text(_) => AttributeKind::Text,
        }
    }

    /// How many elements they are for.
    pub fn len(&self) -> usize {
        match self {
            AttributeValues::Float(c) => c.len(),
            AttributeValues::Integer(c) => c.len(),
            AttributeValues::Vector(c) => c.len(),
            AttributeValues::Text(t) => t.len(),
        }
    }

    /// Whether they are for no element.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The values of the elements in `range`, sharing every buffer.
    pub(crate) fn slice(&self, range: std::ops::Range<usize>) -> Self {
        match self {
            AttributeValues::Float(c) => AttributeValues::Float(c.slice(range)),
            AttributeValues::Integer(c) => AttributeValues::Integer(c.slice(range)),
            AttributeValues::Vector(c) => AttributeValues::Vector(c.slice(range)),
            AttributeValues::Text(t) => AttributeValues::Text(t.slice(range)),
        }
    }
}

/// One attribute: its values and what they mean.
#[derive(Clone, Debug, PartialEq)]
pub struct Attribute {
    values: AttributeValues,
    info: TypeInfo,
}

impl Attribute {
    /// `values` meaning `info` (which only vectors use; give
    /// [`TypeInfo::None`] for the other kinds).
    pub fn new(values: AttributeValues, info: TypeInfo) -> Self {
        Self { values, info }
    }

    /// Real numbers.
    pub fn float(values: impl Into<Column<f64>>) -> Self {
        Self::new(AttributeValues::Float(values.into()), TypeInfo::None)
    }

    /// Whole numbers.
    pub fn integer(values: impl Into<Column<i64>>) -> Self {
        Self::new(AttributeValues::Integer(values.into()), TypeInfo::None)
    }

    /// Vectors meaning `info`.
    pub fn vector(values: impl Into<Column<[f64; 3]>>, info: TypeInfo) -> Self {
        Self::new(AttributeValues::Vector(values.into()), info)
    }

    /// Indexed text.
    pub fn text(text: Text) -> Self {
        Self::new(AttributeValues::Text(text), TypeInfo::None)
    }

    /// The values.
    pub fn values(&self) -> &AttributeValues {
        &self.values
    }

    /// What vectors mean.
    pub fn info(&self) -> TypeInfo {
        self.info
    }

    /// The kind of values.
    pub fn kind(&self) -> AttributeKind {
        self.values.kind()
    }

    /// How many elements it is for.
    pub fn len(&self) -> usize {
        self.values.len()
    }

    /// Whether it is for no element.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }

    /// The reals, when it holds reals.
    pub fn as_float(&self) -> Option<&Column<f64>> {
        match &self.values {
            AttributeValues::Float(c) => Some(c),
            _ => None,
        }
    }

    /// The whole numbers, when it holds them.
    pub fn as_integer(&self) -> Option<&Column<i64>> {
        match &self.values {
            AttributeValues::Integer(c) => Some(c),
            _ => None,
        }
    }

    /// The vectors, when it holds vectors.
    pub fn as_vector(&self) -> Option<&Column<[f64; 3]>> {
        match &self.values {
            AttributeValues::Vector(c) => Some(c),
            _ => None,
        }
    }

    /// The text, when it holds text.
    pub fn as_text(&self) -> Option<&Text> {
        match &self.values {
            AttributeValues::Text(t) => Some(t),
            _ => None,
        }
    }

    /// The attribute over the elements in `range`, sharing every buffer.
    pub(crate) fn slice(&self, range: std::ops::Range<usize>) -> Self {
        Self::new(self.values.slice(range), self.info)
    }
}

type Named<T> = BTreeMap<Class, BTreeMap<Arc<str>, Arc<T>>>;

/// A value's attributes and groups, by class and name.
///
/// Immutable and shared: [`with`](Self::with), [`without`](Self::without)
/// and [`with_group`](Self::with_group) return a new table sharing every
/// entry they didn't touch. Attributes and groups are two namespaces.
///
/// **Rules** (checked as entries are added): every name passes
/// [`check_name`]; a [standard name](names) has its standard type; `P` is
/// never a Point attribute (positions are the value's own column).
///
/// ```
/// use kinogaki_geometry::{Attribute, Attributes, Class, TypeInfo, names};
///
/// let colors = Attribute::vector(vec![[1.0, 0.0, 0.0]; 4], TypeInfo::Color);
/// let table = Attributes::new().with(Class::Point, names::CD, colors).unwrap();
/// assert_eq!(table.vector(Class::Point, "Cd").map(|c| c.len()), Some(4));
/// // A standard name keeps its type.
/// assert!(table.with(Class::Point, names::CD, Attribute::float(vec![0.0; 4])).is_err());
/// ```
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Attributes {
    values: Named<Attribute>,
    groups: Named<Group>,
}

impl Attributes {
    /// No attributes and no groups.
    pub fn new() -> Self {
        Self::default()
    }

    /// Attribute `name` of `class`.
    pub fn get(&self, class: Class, name: &str) -> Option<&Attribute> {
        self.values.get(&class)?.get(name).map(|a| &**a)
    }

    /// Attribute `name` of `class`, shared.
    pub fn get_shared(&self, class: Class, name: &str) -> Option<&Arc<Attribute>> {
        self.values.get(&class)?.get(name)
    }

    /// Group `name` of `class`.
    pub fn group(&self, class: Class, name: &str) -> Option<&Group> {
        self.groups.get(&class)?.get(name).map(|g| &**g)
    }

    /// Every attribute, by class then name.
    pub fn iter(&self) -> impl Iterator<Item = (Class, &str, &Attribute)> {
        self.values
            .iter()
            .flat_map(|(&c, m)| m.iter().map(move |(n, a)| (c, &**n, &**a)))
    }

    /// Every group, by class then name.
    pub fn groups(&self) -> impl Iterator<Item = (Class, &str, &Group)> {
        self.groups
            .iter()
            .flat_map(|(&c, m)| m.iter().map(move |(n, g)| (c, &**n, &**g)))
    }

    /// Whether it holds no attribute and no group.
    pub fn is_empty(&self) -> bool {
        self.values.is_empty() && self.groups.is_empty()
    }

    /// The table with attribute `name` of `class` set to `attribute`
    /// (replacing one so named), sharing the rest.
    ///
    /// # Errors
    /// A bad name, or a standard name of another type.
    pub fn with(
        &self,
        class: Class,
        name: &str,
        attribute: Attribute,
    ) -> Result<Attributes, GeometryError> {
        self.with_shared(class, name, Arc::new(attribute))
    }

    /// [`with`](Self::with) for an attribute shared with another table:
    /// both hold the same one.
    ///
    /// # Errors
    /// As [`with`](Self::with).
    pub fn with_shared(
        &self,
        class: Class,
        name: &str,
        attribute: Arc<Attribute>,
    ) -> Result<Attributes, GeometryError> {
        check_name(name)?;
        check_standard_attribute(class, name, &attribute)?;
        let mut out = self.clone();
        out.values
            .entry(class)
            .or_default()
            .insert(name.into(), attribute);
        Ok(out)
    }

    /// The table without attribute `name` of `class` (nothing changes when
    /// there is none).
    pub fn without(&self, class: Class, name: &str) -> Attributes {
        let mut out = self.clone();
        remove(&mut out.values, class, name);
        out
    }

    /// The table with group `name` of `class` set to `group`.
    ///
    /// # Errors
    /// A bad name, or a standard group name on another class (`sharp` is
    /// an Edge group, `.hidden` a Primitive group).
    pub fn with_group(
        &self,
        class: Class,
        name: &str,
        group: Group,
    ) -> Result<Attributes, GeometryError> {
        check_name(name)?;
        match names::standard(name) {
            Some(Standard::Group(must)) if must != class => {
                return Err(GeometryError::whole(
                    ErrorKind::Attribute,
                    format!(
                        "{name} is a group of the {} class, not of the {} class",
                        must.name(),
                        class.name()
                    ),
                ));
            }
            Some(Standard::Attribute(..)) => {
                return Err(GeometryError::whole(
                    ErrorKind::Attribute,
                    format!("{name} is a standard attribute, not a group"),
                ));
            }
            _ => {}
        }
        let mut out = self.clone();
        out.groups
            .entry(class)
            .or_default()
            .insert(name.into(), Arc::new(group));
        Ok(out)
    }

    /// The table without group `name` of `class`.
    pub fn without_group(&self, class: Class, name: &str) -> Attributes {
        let mut out = self.clone();
        remove(&mut out.groups, class, name);
        out
    }

    /// Reals `name` of `class`, when it holds reals.
    pub fn float(&self, class: Class, name: &str) -> Option<&Column<f64>> {
        self.get(class, name)?.as_float()
    }

    /// Whole numbers `name` of `class`, when it holds them.
    pub fn integer(&self, class: Class, name: &str) -> Option<&Column<i64>> {
        self.get(class, name)?.as_integer()
    }

    /// Vectors `name` of `class`, when it holds vectors.
    pub fn vector(&self, class: Class, name: &str) -> Option<&Column<[f64; 3]>> {
        self.get(class, name)?.as_vector()
    }

    /// Text `name` of `class`, when it holds text.
    pub fn text(&self, class: Class, name: &str) -> Option<&Text> {
        self.get(class, name)?.as_text()
    }

    /// Checks every entry's length against `count(class)`; `None` for a
    /// class the value doesn't have, which no entry may use. `what` names
    /// the value in messages.
    pub(crate) fn check_counts(
        &self,
        what: &str,
        mut count: impl FnMut(Class) -> Option<usize>,
    ) -> Result<(), GeometryError> {
        let classes = self.values.keys().chain(self.groups.keys());
        for &class in classes {
            let Some(expected) = count(class) else {
                return Err(GeometryError::whole(
                    ErrorKind::Attribute,
                    format!("{what} have no {} class", class.name()),
                ));
            };
            let attributes = self.values.get(&class).into_iter().flatten();
            for (name, a) in attributes {
                if a.len() != expected {
                    return Err(missized(class, name, "attribute", a.len(), expected));
                }
            }
            for (name, g) in self.groups.get(&class).into_iter().flatten() {
                if g.elements() != expected {
                    return Err(missized(class, name, "group", g.elements(), expected));
                }
            }
        }
        Ok(())
    }
}

fn remove<T>(map: &mut Named<T>, class: Class, name: &str) {
    if let Some(named) = map.get_mut(&class) {
        named.remove(name);
        if named.is_empty() {
            map.remove(&class);
        }
    }
}

fn missized(class: Class, name: &str, what: &str, len: usize, expected: usize) -> GeometryError {
    GeometryError::whole(
        ErrorKind::Attribute,
        format!(
            "{} {what} {name} has {len} values for {expected} elements",
            class.name()
        ),
    )
}

fn check_standard_attribute(
    class: Class,
    name: &str,
    attribute: &Attribute,
) -> Result<(), GeometryError> {
    let refuse = |why: String| Err(GeometryError::whole(ErrorKind::Attribute, why));
    match names::standard(name) {
        Some(Standard::Group(class)) => refuse(format!(
            "{name} is a standard group of the {} class",
            class.name()
        )),
        Some(Standard::Attribute(..)) if name == names::P && class == Class::Point => {
            refuse("positions are the value's own column, not a point attribute P".into())
        }
        Some(Standard::Attribute(kind, info))
            if attribute.kind() != kind || attribute.info() != info =>
        {
            refuse(format!(
                "{name} is a {} attribute ({}), not {} ({})",
                kind.name(),
                info.name(),
                attribute.kind().name(),
                attribute.info().name()
            ))
        }
        _ => Ok(()),
    }
}
