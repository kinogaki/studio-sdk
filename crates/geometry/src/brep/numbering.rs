//! A B-rep value's one element numbering ([`Brep::count`],
//! [`Brep::element`], [`Brep::id`]) and its fingerprint
//! ([`Brep::fingerprint`]), which a selection made on it keeps.
use super::{Brep, Datum, DatumKind};
use crate::attributes::{Domain, Text};
use crate::kept::Kept;
use std::sync::Arc;

/// One element of a B-rep value as [`Brep::element`] names it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Element {
    /// Element `index` of its domain in body `body`: a face, an edge or a
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

/// The domain a datum of `kind` is numbered in.
fn datum_domain(kind: &DatumKind) -> Domain {
    match kind {
        DatumKind::Plane(_) => Domain::Primitive,
        DatumKind::Axis { .. } => Domain::Edge,
        DatumKind::Point(_) => Domain::Point,
    }
}

/// Whether [`Brep::element`] numbers `domain`'s elements.
fn numbered(domain: Domain) -> bool {
    matches!(domain, Domain::Primitive | Domain::Edge | Domain::Point)
}

impl Brep {
    /// How many elements of `domain` it holds, as one numbering across the
    /// value: Detail one; Object its bodies and Instance their placements;
    /// Primitive the bodies' faces, then the datum planes; Edge the bodies'
    /// edges, then the free curves, then the datum axes; Point the bodies'
    /// vertices, then the datum points. Bodies count in body order (a
    /// body's faces follow the faces of the bodies before it), datums in
    /// datum order. `None` for a domain it doesn't have.
    pub fn count(&self, domain: Domain) -> Option<usize> {
        match domain {
            Domain::Detail => return Some(1),
            Domain::Object
            | Domain::Instance
            | Domain::Primitive
            | Domain::Edge
            | Domain::Point => {}
            _ => return None,
        }
        let bodies: usize = (self.bodies.iter())
            .map(|b| b.count(domain))
            .sum::<Option<usize>>()?;
        let curves = if domain == Domain::Edge {
            self.curves.curves()
        } else {
            0
        };
        Some(bodies + curves + self.datums_of(domain).count())
    }

    /// What element `id` of `domain` is ([`count`](Self::count)'s order);
    /// `None` past the count or for another domain.
    pub fn element(&self, domain: Domain, id: u32) -> Option<Element> {
        if !numbered(domain) {
            return None;
        }
        let mut id = id as usize;
        for (body, b) in self.bodies.iter().enumerate() {
            let n = b.count(domain)?;
            if id < n {
                return Some(Element::Body { body, index: id });
            }
            id -= n;
        }
        if domain == Domain::Edge {
            if id < self.curves.curves() {
                return Some(Element::Curve(id));
            }
            id -= self.curves.curves();
        }
        self.datums_of(domain).nth(id).map(Element::Datum)
    }

    /// The id of `element` in `domain` ([`element`](Self::element)'s
    /// inverse); `None` when it isn't one of the value's of that domain.
    pub fn id(&self, domain: Domain, element: Element) -> Option<u32> {
        let bodies = |upto: usize| -> Option<usize> {
            (self.bodies.get(..upto)?.iter())
                .map(|b| b.count(domain))
                .sum::<Option<usize>>()
        };
        if !numbered(domain) {
            return None;
        }
        let all = bodies(self.bodies.len())?;
        let curves = self.curves.curves();
        let id = match element {
            Element::Body { body, index } if index < self.bodies.get(body)?.count(domain)? => {
                bodies(body)? + index
            }
            Element::Curve(index) if domain == Domain::Edge && index < curves => all + index,
            Element::Datum(index) if datum_domain(&self.datums.get(index)?.kind) == domain => {
                let before = self.datums_of(domain).take_while(|&d| d < index).count();
                all + if domain == Domain::Edge { curves } else { 0 } + before
            }
            _ => return None,
        };
        u32::try_from(id).ok()
    }

    /// The datums numbered in `domain`, by index, in datum order.
    fn datums_of(&self, domain: Domain) -> impl Iterator<Item = usize> + '_ {
        (self.datums.iter().enumerate())
            .filter(move |(_, d)| datum_domain(&d.kind) == domain)
            .map(|(i, _)| i)
    }

    /// What its numbering stands on, as one 32-byte hash: BLAKE3 of the
    /// body count, each body's topology fingerprint
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
        let names = (self.curves.attributes()).text(Domain::Primitive, crate::names::NAME);
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
    for domain in [Domain::Primitive, Domain::Edge, Domain::Point] {
        let of = || datums.iter().filter(|d| datum_domain(&d.kind) == domain);
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
