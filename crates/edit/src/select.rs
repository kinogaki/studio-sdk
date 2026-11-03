//! Selection rules: a [`Selection`] as another domain, the elements it
//! holds, how it is found again after its value changed
//! ([`reresolve`]), how a pick grows before it becomes a selection
//! ([`expand`]: connected pieces, edge loops and rings), and what lies
//! next to it ([`grow`], [`shrink`], [`boundary`]). Topology questions go
//! to the table's one index, so a rule costs what it touches.
//!
//! One rule for every value: conversions read the value's
//! [`Incidence`] (polygons and B-rep alike), so the levels row, the
//! spreadsheet, snapping and a kernel's tools all mean the same by
//! "these faces' points".
//!
//! ```
//! use kinogaki_edit::select;
//! use kinogaki_geometry::{Attributes, Domain, Elements, Geometry, Selection, Primitives};
//!
//! // Two triangles sharing the edge 1–2.
//! let t = Primitives::of_polygons(4, vec![0, 3, 6].into(), vec![0, 1, 2, 1, 3, 2].into()).unwrap();
//! let p = Geometry::new(vec![[0.0; 3]; 4].into(), t, Attributes::new()).unwrap();
//! let first = Elements::from_indices(2, [0]).unwrap();
//! let first = Selection::on(&p, Domain::Primitive, first, None).unwrap();
//! let points = select::convert(&first, &p, Domain::Point).unwrap();
//! assert_eq!(points.members().iter().collect::<Vec<_>>(), [0, 1, 2]);
//! // Back up, wholly: those points hold the first triangle alone.
//! let faces = select::convert(&points, &p, Domain::Primitive).unwrap();
//! assert_eq!(faces.members().iter().collect::<Vec<_>>(), [0]);
//! ```
use kinogaki_geometry::{
    Column, Components, Domain, Elements, Error, Geometry, Incidence, Selection, names,
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
pub fn convert(s: &Selection, on: &dyn Incidence, to: Domain) -> Result<Selection, Error> {
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
) -> Result<Selection, Error> {
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
pub fn resolve(s: &Selection, on: &dyn Incidence) -> Result<Column<u32>, Error> {
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
/// use kinogaki_geometry::{Attribute, Attributes, Domain, Elements, Geometry, Selection, Primitives, names};
///
/// let with_ids = |points: usize, ids: Vec<i64>| {
///     let t = Primitives::of_polygons(points, vec![0, 3].into(), vec![0, 1, 2].into()).unwrap();
///     let ids = Attributes::new().with(Domain::Point, names::ID, Attribute::tuples(ids, kinogaki_geometry::Meaning::None).unwrap()).unwrap();
///     Geometry::new(vec![[0.0; 3]; points].into(), t, ids).unwrap()
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
fn target(on: &dyn Incidence, domain: Domain) -> Result<usize, Error> {
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
                Domain::Edge => table.edges()[i as usize].points[0],
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

/// `s` with what lies next to it: a point's neighbours, the edges and
/// primitives sharing a point with an edge or primitive of it, and the
/// vertices on a selected vertex's point and on its neighbours. Costs what
/// it touches. A selection that doesn't belong to `on`, or of a domain
/// without neighbours (Detail), stays as it is.
///
/// ```
/// use kinogaki_edit::select;
/// use kinogaki_geometry::{Attributes, Domain, Elements, Geometry, Primitives, Selection};
///
/// // A strip of three quads.
/// let t = Primitives::of_polygons(8, vec![0, 4, 8, 12].into(), vec![0, 1, 5, 4, 1, 2, 6, 5, 2, 3, 7, 6].into()).unwrap();
/// let strip = Geometry::new(vec![[0.0; 3]; 8].into(), t, Attributes::new()).unwrap();
/// let first = Elements::from_indices(3, [0]).unwrap();
/// let first = Selection::on(&strip, Domain::Primitive, first, None).unwrap();
/// let two = select::grow(&first, &strip);
/// assert_eq!(two.members().iter().collect::<Vec<_>>(), [0, 1]);
/// assert_eq!(select::shrink(&two, &strip).members().iter().collect::<Vec<_>>(), [0]);
/// ```
pub fn grow(s: &Selection, on: &Geometry) -> Selection {
    if s.validate(on).is_err() || !has_neighbours(s.domain()) || s.members().is_all() {
        return s.clone();
    }
    let mut grown: Vec<u32> = s.members().iter().collect();
    for i in s.members().iter() {
        around(on, s.domain(), i, &mut |near| grown.push(near));
    }
    let count = on.count(s.domain()).unwrap_or(0);
    let members = Elements::from_indices(count, grown).expect("the value's own elements");
    Selection::on(on, s.domain(), members, s.active()).expect("the active element stays")
}

/// `s` without what [`grow`] of the rest would take: the elements with
/// everything next to them selected. So a whole surface stays whole.
/// Stays as it is when [`grow`] would.
pub fn shrink(s: &Selection, on: &Geometry) -> Selection {
    if s.validate(on).is_err() || !has_neighbours(s.domain()) || s.members().is_all() {
        return s.clone();
    }
    let kept = s.members().iter().filter(|&i| {
        let mut inside = true;
        around(on, s.domain(), i, &mut |near| {
            inside &= s.contains(near);
        });
        inside
    });
    let count = on.count(s.domain()).unwrap_or(0);
    let members = Elements::from_indices(count, kept).expect("the value's own elements");
    let active = s.active().filter(|&a| members.contains(a));
    Selection::on(on, s.domain(), members, active).expect("the active element stays")
}

fn has_neighbours(domain: Domain) -> bool {
    matches!(
        domain,
        Domain::Point | Domain::Vertex | Domain::Edge | Domain::Primitive
    )
}

/// Calls `near` with what lies next to element `i` of `domain`: the one
/// relation [`grow`] and [`shrink`] share, and a symmetric one, so
/// shrinking is growing the rest.
fn around(on: &Geometry, domain: Domain, i: u32, near: &mut dyn FnMut(u32)) {
    let table = on.primitives();
    let index = table.index();
    match domain {
        Domain::Point => index.point_neighbours(i as usize).for_each(near),
        Domain::Edge => {
            for p in table.edges()[i as usize].points {
                index.point_every_edge(p as usize).for_each(&mut *near);
            }
        }
        Domain::Primitive => {
            for &p in table.primitive(i as usize) {
                index.point_primitives(p as usize).for_each(&mut *near);
            }
        }
        _ => {
            let point = table.vertices()[i as usize];
            let mut on_point = |p: u32| {
                for prim in index.point_primitives(p as usize) {
                    for v in table.primitive_vertices(prim as usize) {
                        if table.vertices()[v] == p {
                            near(v as u32);
                        }
                    }
                }
            };
            on_point(point);
            index.point_neighbours(point as usize).for_each(on_point);
        }
    }
}

/// The edges of a Primitive selection's outline: those with exactly one
/// selected polygon among their polygons (of every polygon, the edges the
/// surface ends at, [`Index::boundary_edges`](kinogaki_geometry::Index::boundary_edges)).
/// Costs what it touches.
///
/// ```
/// use kinogaki_edit::select;
/// use kinogaki_geometry::{Attributes, Domain, Elements, Geometry, Primitives, Selection};
///
/// // Two quads side by side, sharing the edge 1–4.
/// let t = Primitives::of_polygons(6, vec![0, 4, 8].into(), vec![0, 1, 4, 3, 1, 2, 5, 4].into()).unwrap();
/// let quads = Geometry::new(vec![[0.0; 3]; 6].into(), t, Attributes::new()).unwrap();
/// let both = Selection::on(&quads, Domain::Primitive, Elements::all(2), None).unwrap();
/// let around = select::boundary(&both, &quads).unwrap();
/// assert_eq!(around.len(), 6);
/// assert!(!around.contains(quads.primitives().edge_of(1, 4).unwrap()));
/// ```
///
/// # Errors
/// As [`Selection::validate`], or a selection of another domain.
pub fn boundary(s: &Selection, on: &Geometry) -> Result<Selection, Error> {
    s.validate(on)?;
    if s.domain() != Domain::Primitive {
        return Err(Error::invalid(format!(
            "a boundary is of primitives, not of the {} domain",
            s.domain().name()
        )));
    }
    let table = on.primitives();
    let index = table.index();
    let count = table.edges().len();
    let outline = |edges: Vec<u32>| Elements::from_indices(count, edges);
    let members = if s.members().is_all() {
        outline(index.boundary_edges().to_vec())?
    } else {
        let mut edges = Vec::new();
        for p in s.members().iter() {
            for &e in index.primitive_edges(p as usize) {
                let polygons = &index.edge_faces()[e as usize];
                if polygons.iter().filter(|&&f| s.contains(f)).count() == 1 {
                    edges.push(e);
                }
            }
        }
        outline(edges)?
    };
    Selection::on(on, Domain::Edge, members, None)
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
