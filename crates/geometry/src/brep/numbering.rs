//! A B-rep value's one element numbering ([`Brep::count`],
//! [`Brep::element`], [`Brep::id`]) and its fingerprint
//! ([`Brep::fingerprint`]), which a selection made on it keeps.
use super::{Brep, Datum, DatumKind};
use crate::attributes::{Class, Text};
use crate::kept::Kept;
use std::sync::Arc;

/// One element of a B-rep value as [`Brep::element`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Element {
    /// Element `index` of its class in body `body`: a face, an edge or a
    /// vertex.
    Body {
        /// The body, in body order.
        body: usize,
        /// The face, edge or vertex in it.
        index: usize,
    },
    /// Free curve `index` (an Edge).
    Curve(usize),
    /// Datum `index` of [`Brep::datums`]: a plane is a Primitive, an axis
    /// an Edge, a point a Point.
    Datum(usize),
}

/// The class a datum of `kind` is numbered in.
fn datum_class(kind: &DatumKind) -> Class {
    match kind {
        DatumKind::Plane(_) => Class::Primitive,
        DatumKind::Axis { .. } => Class::Edge,
        DatumKind::Point(_) => Class::Point,
    }
}

/// Whether [`Brep::element`] numbers `class`'s elements.
fn numbered(class: Class) -> bool {
    matches!(class, Class::Primitive | Class::Edge | Class::Point)
}

impl Brep {
    /// How many elements of `class` it holds, as one numbering across the
    /// value: Detail one; Object its bodies and Instance their placements;
    /// Primitive the bodies' faces, then the datum planes; Edge the bodies'
    /// edges, then the free curves, then the datum axes; Point the bodies'
    /// vertices, then the datum points. Bodies count in body order (a
    /// body's faces follow the faces of the bodies before it), datums in
    /// datum order. `None` for a class it doesn't have.
    pub fn count(&self, class: Class) -> Option<usize> {
        match class {
            Class::Detail => return Some(1),
            Class::Object | Class::Instance | Class::Primitive | Class::Edge | Class::Point => {}
            _ => return None,
        }
        let bodies: usize = (self.bodies.iter())
            .map(|b| b.count(class))
            .sum::<Option<usize>>()?;
        let curves = if class == Class::Edge {
            self.curves.curves()
        } else {
            0
        };
        Some(bodies + curves + self.datums_of(class).count())
    }

    /// What element `id` of `class` is ([`count`](Self::count)'s order);
    /// `None` past the count or for another class.
    pub fn element(&self, class: Class, id: u32) -> Option<Element> {
        if !numbered(class) {
            return None;
        }
        let mut id = id as usize;
        for (body, b) in self.bodies.iter().enumerate() {
            let n = b.count(class)?;
            if id < n {
                return Some(Element::Body { body, index: id });
            }
            id -= n;
        }
        if class == Class::Edge {
            if id < self.curves.curves() {
                return Some(Element::Curve(id));
            }
            id -= self.curves.curves();
        }
        self.datums_of(class).nth(id).map(Element::Datum)
    }

    /// The id of `element` in `class` ([`element`](Self::element)'s
    /// inverse); `None` when it isn't one of the value's of that class.
    pub fn id(&self, class: Class, element: Element) -> Option<u32> {
        let bodies = |upto: usize| -> Option<usize> {
            (self.bodies.get(..upto)?.iter())
                .map(|b| b.count(class))
                .sum::<Option<usize>>()
        };
        if !numbered(class) {
            return None;
        }
        let all = bodies(self.bodies.len())?;
        let curves = self.curves.curves();
        let id = match element {
            Element::Body { body, index } if index < self.bodies.get(body)?.count(class)? => {
                bodies(body)? + index
            }
            Element::Curve(index) if class == Class::Edge && index < curves => all + index,
            Element::Datum(index) if datum_class(&self.datums.get(index)?.kind) == class => {
                let before = self.datums_of(class).take_while(|&d| d < index).count();
                all + if class == Class::Edge { curves } else { 0 } + before
            }
            _ => return None,
        };
        u32::try_from(id).ok()
    }

    /// The datums numbered in `class`, by index, in datum order.
    fn datums_of(&self, class: Class) -> impl Iterator<Item = usize> + '_ {
        (self.datums.iter().enumerate())
            .filter(move |(_, d)| datum_class(&d.kind) == class)
            .map(|(i, _)| i)
    }

    /// What its numbering stands on, as one 32-byte hash: BLAKE3 of the
    /// body count, each body's [topology fingerprint](super::BrepTopology::fingerprint)
    /// and the fingerprint of its persistent names in body order; then the
    /// free curves' count and the hash of their Primitive `name`s in order
    /// (each its byte length and bytes, `u64::MAX` for none; 32 zero bytes
    /// without the attribute); then the hash of the datums, for planes,
    /// axes and points in turn their count and each one's persistent name
    /// in datum order. Counts are little-endian `u64`; the names' hashes
    /// are cached on their columns. It hashes structure and names alone,
    /// never geometry: equal fingerprints, the same elements under the
    /// same ids, so a selection made on one value still means the same on
    /// the other (a resize or a datum moved keeps it), and a stale one is
    /// seen when they differ (a face added or split, two datums or free
    /// curves swapped).
    pub fn fingerprint(&self) -> [u8; 32] {
        let mut h = blake3::Hasher::new();
        h.update(&(self.bodies.len() as u64).to_le_bytes());
        for body in self.bodies.iter() {
            h.update(&body.topology().fingerprint());
            h.update(&body.names().fingerprint());
        }
        h.update(&(self.curves.curves() as u64).to_le_bytes());
        let names = (self.curves.attributes()).text(Class::Primitive, crate::names::NAME);
        h.update(&names.map_or([0; 32], text_hash));
        h.update(
            &self
                .datums
                .derived(|datums| DatumNames(datums_hash(datums)))
                .0,
        );
        *h.finalize().as_bytes()
    }
}

/// The hash of a datums column's names, kept on it.
struct DatumNames([u8; 32]);

fn datums_hash(datums: &[Datum]) -> [u8; 32] {
    let mut h = blake3::Hasher::new();
    for class in [Class::Primitive, Class::Edge, Class::Point] {
        let of = || datums.iter().filter(|d| datum_class(&d.kind) == class);
        h.update(&(of().count() as u64).to_le_bytes());
        for datum in of() {
            super::fingerprint::hash_name(&mut h, &datum.name);
        }
    }
    *h.finalize().as_bytes()
}

/// The hash of `names` in order, kept on its index column for its table.
fn text_hash(names: &Text) -> [u8; 32] {
    let kept = names.indices().derived(|_| TextHashes::default());
    if let Some(hash) = kept.find(|table| Arc::ptr_eq(table, names.table())) {
        return *hash;
    }
    let mut h = blake3::Hasher::new();
    for name in names.iter() {
        match name {
            Some(name) => {
                h.update(&(name.len() as u64).to_le_bytes());
                h.update(name.as_bytes());
            }
            None => {
                h.update(&u64::MAX.to_le_bytes());
            }
        }
    }
    let hash = *h.finalize().as_bytes();
    kept.keep(names.table().clone(), Arc::new(hash));
    hash
}

type TextHashes = Kept<Arc<[Arc<str>]>, [u8; 32]>;
