//! Selection rules: a [`Selection`] as another domain, the elements it
//! holds, how it is found again after its value changed
//! ([`reresolve`]), and how a pick grows before it becomes a selection.
//!
//! One rule for every value: conversions read the value's
//! [`Incidence`] (polygons and B-rep alike), so the levels row, the
//! spreadsheet, snapping and a kernel's tools all mean the same by
//! "these faces' points".
//!
//! ```
//! use kinogaki_edit::select;
//! use kinogaki_geometry::{Attributes, Domain, Elements, Polygons, Selection, Primitives};
//!
//! // Two triangles sharing the edge 1–2.
//! let t = Primitives::of_polygons(4, vec![0, 3, 6].into(), vec![0, 1, 2, 1, 3, 2].into()).unwrap();
//! let p = Polygons::new(vec![[0.0; 3]; 4].into(), t, Attributes::new()).unwrap();
//! let first = Elements::from_indices(2, [0]).unwrap();
//! let first = Selection::on(&p, Domain::Primitive, first, None).unwrap();
//! let points = select::convert(&first, &p, Domain::Point).unwrap();
//! assert_eq!(points.members().iter().collect::<Vec<_>>(), [0, 1, 2]);
//! // Back up, wholly: those points hold the first triangle alone.
//! let faces = select::convert(&points, &p, Domain::Primitive).unwrap();
//! assert_eq!(faces.members().iter().collect::<Vec<_>>(), [0]);
//! ```
use kinogaki_geometry::{
    Column, Domain, Elements, GeometryError, Incidence, Polygons, Selection, names,
};
use std::collections::HashMap;
use std::sync::Arc;

/// How [`convert_by`] relates a target element to the selected elements
/// it touches.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coverage {
    /// One selected touching element is enough (an edge of a selected
    /// polygon, a point of a selected edge).
    Any,
    /// Every touching element must be selected (a polygon all of whose
    /// points are selected).
    Whole,
}

/// The selection as `to` by the standard rule: [`Coverage::Whole`] into
/// Primitive and Object, and from Point into Edge; [`Coverage::Any`]
/// otherwise. The same domain is the selection itself; every element of
/// one domain is, in another, what touches them (a loose point is no
/// polygon's).
///
/// # Errors
/// As [`Selection::validate`], or a domain `on` has none of.
pub fn convert(s: &Selection, on: &dyn Incidence, to: Domain) -> Result<Selection, GeometryError> {
    s.validate(on)?;
    if to == s.domain() {
        return Ok(s.clone());
    }
    let whole = matches!(to, Domain::Primitive | Domain::Object)
        || (to == Domain::Edge && s.domain() == Domain::Point);
    let coverage = if whole {
        Coverage::Whole
    } else {
        Coverage::Any
    };
    convert_by(s, on, to, coverage)
}

/// The selection as `to`, each target element selected by `coverage` of
/// the elements it touches ([`Incidence::incident`]). Elements touching
/// nothing selected are never added; the same domain is the selection
/// itself.
///
/// # Errors
/// As [`convert`].
pub fn convert_by(
    s: &Selection,
    on: &dyn Incidence,
    to: Domain,
    coverage: Coverage,
) -> Result<Selection, GeometryError> {
    s.validate(on)?;
    if to == s.domain() {
        return Ok(s.clone());
    }
    let targets = target(on, to)?;
    let mut chosen = vec![false; on.count(s.domain()).unwrap_or(0)];
    for i in s.members().iter() {
        chosen[i as usize] = true;
    }
    let whole = coverage == Coverage::Whole;
    let mut touched = vec![false; targets];
    let mut missing = vec![false; if whole { targets } else { 0 }];
    on.incident(s.domain(), to, &mut |source, target| {
        let (source, target) = (source as usize, target as usize);
        if chosen[source] {
            touched[target] = true;
        } else if whole {
            missing[target] = true;
        }
    });
    let members = Elements::from_fn(targets, |i| {
        touched[i as usize] && (!whole || !missing[i as usize])
    });
    Selection::on(on, to, members, None)
}

/// The selected elements of `on`, ascending (shared when the selection
/// holds them as a list).
///
/// # Errors
/// As [`Selection::validate`].
pub fn resolve(s: &Selection, on: &dyn Incidence) -> Result<Column<u32>, GeometryError> {
    s.validate(on)?;
    Ok(s.members().indices())
}

/// `s` on `on`, a later value of the one it was made on: itself while
/// `on` numbers the elements as that value did; after a change of
/// topology upstream, the elements of `on` whose stable ids it holds
/// (those still there; the active one too, when it is). `None` when it
/// can't be found again: it holds no ids, or `on` has none on its domain.
///
/// ```
/// use kinogaki_edit::select;
/// use kinogaki_geometry::{Attribute, Attributes, Domain, Elements, Polygons, Selection, Primitives, names};
///
/// let with_ids = |points: usize, ids: Vec<i64>| {
///     let t = Primitives::of_polygons(points, vec![0, 3].into(), vec![0, 1, 2].into()).unwrap();
///     let ids = Attributes::new().with(Domain::Point, names::ID, Attribute::tuples(ids, kinogaki_geometry::Meaning::None).unwrap()).unwrap();
///     Polygons::new(vec![[0.0; 3]; points].into(), t, ids).unwrap()
/// };
/// let before = with_ids(3, vec![10, 11, 12]);
/// let s = Selection::on(&before, Domain::Point, Elements::from_indices(3, [1, 2]).unwrap(), Some(2)).unwrap();
/// // A point inserted upstream, before the others.
/// let after = with_ids(4, vec![99, 10, 11, 12]);
/// let found = select::reresolve(&s, &after).unwrap();
/// assert_eq!(found.members().iter().collect::<Vec<_>>(), [2, 3]);
/// assert_eq!(found.active(), Some(3));
/// ```
pub fn reresolve(s: &Selection, on: &dyn Incidence) -> Option<Selection> {
    if s.validate(on).is_ok() {
        return Some(s.clone());
    }
    let (Some(kept), Some(now)) = (s.ids(), on.ids(s.domain())) else {
        return None;
    };
    let wanted: HashMap<i64, u32> = kept
        .iter()
        .zip(s.members().iter())
        .map(|(&id, member)| (id, member))
        .collect();
    let mut active = None;
    let mut found = Vec::with_capacity(kept.len());
    for (element, id) in now.iter().enumerate() {
        if let Some(&was) = wanted.get(id) {
            found.push(element as u32);
            if s.active() == Some(was) {
                active = Some(element as u32);
            }
        }
    }
    let members = Elements::from_indices(now.len(), found).ok()?;
    Selection::on(on, s.domain(), members, active).ok()
}

/// How many elements of `domain` `on` has, refusing a domain it has none
/// of as [`Selection::validate`] does.
fn target(on: &dyn Incidence, domain: Domain) -> Result<usize, GeometryError> {
    on.count(domain).ok_or_else(|| {
        Selection::none(domain, 0)
            .validate(on)
            .expect_err("a domain the value has none of")
    })
}

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

/// `s` grown by `how` on `on`: with [`Expand::Connected`], every element
/// of its domain in a piece one of its elements lies in. A selection
/// that doesn't belong to `on`, of a domain without pieces (Detail) or
/// of everything, stays as it is.
pub fn expand(s: &Selection, on: &Polygons, how: &Expand) -> Selection {
    let point_of = |domain: Domain| -> Option<Box<dyn Fn(u32) -> u32 + '_>> {
        let topology = on.primitives();
        Some(match domain {
            Domain::Point => Box::new(|p| p),
            Domain::Vertex => Box::new(|v| topology.vertices()[v as usize]),
            Domain::Edge => Box::new(|e| topology.edges()[e as usize].points[0]),
            Domain::Primitive => Box::new(|p| topology.primitive(p as usize)[0]),
            _ => return None,
        })
    };
    let (Expand::Connected, false, Ok(()), Some(point)) = (
        how,
        s.members().is_all(),
        s.validate(on),
        point_of(s.domain()),
    ) else {
        return s.clone();
    };
    let pieces = &pieces(on).0;
    let mut held = vec![false; pieces.len()];
    for i in s.members().iter() {
        held[pieces[point(i) as usize] as usize] = true;
    }
    let count = on.count(s.domain()).unwrap_or(0);
    let members = Elements::from_fn(count, |i| held[pieces[point(i) as usize] as usize]);
    Selection::on(on, s.domain(), members, None).expect("a set of the domain's elements")
}

/// Each point's piece, by the smallest point in it.
struct Pieces(Vec<u32>);

/// `on`'s pieces: points joined through a primitive, and the first points
/// of primitives of one non-blank `name`, are one piece. The joins through
/// primitives are a cache on the topology (built once, shared by every
/// value holding it, so a drag or a rename rebuilds nothing of them);
/// names join over a copy of it.
fn pieces(on: &Polygons) -> Arc<Pieces> {
    let topology = on.primitives();
    let connected = topology.derived(|t| {
        let mut parent: Vec<u32> = (0..t.points() as u32).collect();
        for p in 0..t.len() {
            let corners = t.primitive(p);
            for &q in &corners[1..] {
                join(&mut parent, corners[0], q);
            }
        }
        Pieces(roots(parent))
    });
    let Some(name) = on.attributes().text(Domain::Primitive, names::NAME) else {
        return connected;
    };
    let mut parent = connected.0.clone();
    let mut first: HashMap<&str, u32> = HashMap::new();
    for (p, value) in name.iter().enumerate() {
        let Some(value) = value.filter(|v| !v.trim().is_empty()) else {
            continue;
        };
        let point = topology.primitive(p)[0];
        let at = *first.entry(value).or_insert(point);
        join(&mut parent, at, point);
    }
    Arc::new(Pieces(roots(parent)))
}

/// The root of `p`'s set, halving the path on the way.
fn root(parent: &mut [u32], mut p: u32) -> u32 {
    while parent[p as usize] != p {
        let up = parent[parent[p as usize] as usize];
        parent[p as usize] = up;
        p = up;
    }
    p
}

/// `a`'s and `b`'s sets as one, rooted at the smaller root.
fn join(parent: &mut [u32], a: u32, b: u32) {
    let (a, b) = (root(parent, a), root(parent, b));
    parent[a.max(b) as usize] = a.min(b);
}

/// Every point's root.
fn roots(mut parent: Vec<u32>) -> Vec<u32> {
    for p in 0..parent.len() as u32 {
        parent[p as usize] = root(&mut parent, p);
    }
    parent
}
