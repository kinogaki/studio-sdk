//! A B-rep value's one element numbering ([`Brep::count`],
//! [`Brep::element`], [`Brep::id`]) and its fingerprint
//! ([`Brep::fingerprint`]), which a selection made on it keeps.
use super::{Brep, DatumKind};
use crate::attributes::Class;

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
    /// and the fingerprint of its persistent names in body order, then
    /// the free curves' count and the datums' counts of each class
    /// (planes, axes, points), each count a little-endian `u64`. It hashes
    /// structure and names alone, never geometry: equal fingerprints, the
    /// same elements under the same ids, so a selection made on one value
    /// still means the same on the other (a resize keeps it), and a stale
    /// one is seen when they differ (a face added or split).
    pub fn fingerprint(&self) -> [u8; 32] {
        let mut h = blake3::Hasher::new();
        h.update(&(self.bodies.len() as u64).to_le_bytes());
        for body in self.bodies.iter() {
            h.update(&body.topology().fingerprint());
            h.update(&body.names().fingerprint());
        }
        h.update(&(self.curves.curves() as u64).to_le_bytes());
        for class in [Class::Primitive, Class::Edge, Class::Point] {
            h.update(&(self.datums_of(class).count() as u64).to_le_bytes());
        }
        *h.finalize().as_bytes()
    }
}
