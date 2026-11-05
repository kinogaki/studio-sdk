//! How a pick grows before it becomes a selection: [`Expand`] by a level's
//! [`Mode`], to connected pieces, edge loops or rings.
use kinogaki_geometry::{Components, Domain, Elements, Geometry, Selection, names};
use std::collections::HashMap;
use std::sync::Arc;

/// How a pick grows before it becomes a selection.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum Expand {
    /// As picked.
    #[default]
    None,
    /// To the connected pieces it touches: elements joined through shared
    /// points, and primitives sharing a non-blank `name` (an object's
    /// identity, docs/PARTS.md) kept together.
    Connected,
    /// An Edge selection to the [edge loops](kinogaki_geometry::Index::edge_loop)
    /// through it.
    Loop,
    /// An Edge selection to the [edge rings](kinogaki_geometry::Index::edge_ring)
    /// through it.
    Ring,
}

/// A selection level a context declares: the domain it selects and how
/// a pick grows there. A level of whole pieces (Mesh's) is
/// `Mode { domain: Primitive, expand: Connected }`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Mode {
    /// What it selects.
    pub domain: Domain,
    /// How a pick grows.
    pub expand: Expand,
}

/// The plain level of a domain: picks as they are.
impl From<Domain> for Mode {
    fn from(domain: Domain) -> Self {
        Mode {
            domain,
            expand: Expand::None,
        }
    }
}

/// `s` grown by `how` on `on`. With [`Expand::Connected`], every element
/// of its domain in a piece one of its elements lies in; with
/// [`Expand::Loop`] and [`Expand::Ring`], every edge on a walk through
/// one of its edges. A selection that doesn't belong to `on`, of a domain
/// the rule doesn't grow or of everything, stays as it is.
pub fn expand(s: &Selection, on: &Geometry, how: &Expand) -> Selection {
    if s.members().is_all() || s.validate(on).is_err() {
        return s.clone();
    }
    let table = on.primitives();
    let members = match (how, s.domain()) {
        (Expand::Loop, Domain::Edge) => walked(s, on, |e| table.index().edge_loop(e).edges),
        (Expand::Ring, Domain::Edge) => walked(s, on, |e| table.index().edge_ring(e).edges),
        (Expand::Connected, Domain::Point | Domain::Vertex | Domain::Edge | Domain::Primitive) => {
            let point = |i: u32| match s.domain() {
                Domain::Point => i,
                Domain::Vertex => table.vertices()[i as usize],
                Domain::Edge => table.edge(i as usize).points[0],
                _ => table.primitive(i as usize)[0],
            };
            let pieces = pieces(on);
            let mut held = vec![false; pieces.count()];
            for i in s.members().iter() {
                held[pieces.of_points()[point(i) as usize] as usize] = true;
            }
            let count = on.count(s.domain()).unwrap_or(0);
            Elements::from_fn(count, |i| {
                held[pieces.of_points()[point(i) as usize] as usize]
            })
        }
        _ => return s.clone(),
    };
    Selection::on(on, s.domain(), members, None).expect("a set of the domain's elements")
}

/// The edges of `s`'s walks, each found once.
fn walked(s: &Selection, on: &Geometry, walk: impl Fn(u32) -> Vec<u32>) -> Elements {
    let count = on.count(Domain::Edge).unwrap_or(0);
    let mut found = std::collections::HashSet::new();
    for e in s.members().iter() {
        if !found.contains(&e) {
            found.extend(walk(e));
        }
    }
    Elements::from_indices(count, found).expect("a walk's edges are the value's")
}

/// `on`'s pieces: points joined through a primitive
/// ([`Primitives::components`](kinogaki_geometry::Primitives::components),
/// built once, so a drag or a rename rebuilds nothing of them), and the
/// first points of primitives of one non-blank `name`, joined over a copy.
fn pieces(on: &Geometry) -> Arc<Components> {
    let table = on.primitives();
    let connected = table.components();
    let Some(name) = on.attributes().text(Domain::Primitive, names::NAME) else {
        return connected;
    };
    let mut first: HashMap<&str, u32> = HashMap::new();
    let mut pairs = Vec::new();
    for (p, value) in name.iter().enumerate() {
        let Some(value) = value.filter(|v| !v.trim().is_empty()) else {
            continue;
        };
        let point = table.primitive(p)[0];
        pairs.push((*first.entry(value).or_insert(point), point));
    }
    Arc::new(connected.joined(pairs))
}
