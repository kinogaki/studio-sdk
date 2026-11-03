//! Selection rules: a [`Selection`] as another domain, the elements it
//! holds, and how a pick grows before it becomes a selection.
//!
//! One rule for every value: conversions read the value's
//! [`Incidence`] (polygons and B-rep alike), so the levels row, the
//! spreadsheet, snapping and a kernel's tools all mean the same by
//! "these faces' points".
//!
//! ```
//! use kinogaki_edit::select;
//! use kinogaki_geometry::{Attributes, Domain, Polygons, Selection, Topology};
//!
//! // Two triangles sharing the edge 1–2.
//! let t = Topology::new(4, vec![0, 3, 6].into(), vec![0, 1, 2, 1, 3, 2].into(), 2).unwrap();
//! let p = Polygons::new(vec![[0.0; 3]; 4].into(), t, Attributes::new()).unwrap();
//! let on = Some(p.topology().fingerprint());
//! let first = Selection::new(Domain::Primitive, vec![0], None, on).unwrap();
//! let points = select::convert(&first, &p, Domain::Point).unwrap();
//! assert_eq!(&**points.indices(), &[0, 1, 2]);
//! // Back up, wholly: those points hold the first triangle alone.
//! let faces = select::convert(&points, &p, Domain::Primitive).unwrap();
//! assert_eq!(&**faces.indices(), &[0]);
//! ```
use kinogaki_geometry::{Column, Domain, GeometryError, Incidence, Polygons, Selection, names};
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
/// otherwise. An all-selection stays all; the same domain is the
/// selection itself.
///
/// # Errors
/// As [`Selection::validate`], or a domain `on` has none of.
pub fn convert(s: &Selection, on: &dyn Incidence, to: Domain) -> Result<Selection, GeometryError> {
    s.validate(on)?;
    if to == s.domain() {
        return Ok(s.clone());
    }
    if s.is_all() {
        target(on, to)?;
        return Ok(Selection::all(to, Some(on.fingerprint())));
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
    let mut chosen = vec![s.is_all(); on.count(s.domain()).unwrap_or(0)];
    for &i in s.indices().iter() {
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
    let ids = (0..targets)
        .filter(|&i| touched[i] && (!whole || !missing[i]))
        .map(|i| i as u32)
        .collect();
    Selection::new(to, ids, None, Some(on.fingerprint()))
}

/// The selected elements, ascending: every one of `on`'s for an
/// all-selection, else the indices themselves (shared, no copy).
///
/// # Errors
/// As [`Selection::validate`].
pub fn resolve(s: &Selection, on: &dyn Incidence) -> Result<Column<u32>, GeometryError> {
    s.validate(on)?;
    Ok(if s.is_all() {
        let count = on.count(s.domain()).unwrap_or(0) as u32;
        Column::new((0..count).collect())
    } else {
        s.indices().clone()
    })
}

/// How many elements of `domain` `on` has, refusing a domain it has none
/// of as [`Selection::validate`] does.
fn target(on: &dyn Incidence, domain: Domain) -> Result<usize, GeometryError> {
    Selection::none(domain).validate(on)?;
    Ok(on.count(domain).unwrap_or(0))
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
        let topology = on.topology();
        Some(match domain {
            Domain::Point => Box::new(|p| p),
            Domain::Vertex => Box::new(|v| topology.vertices()[v as usize]),
            Domain::Edge => Box::new(|e| topology.edges()[e as usize].points[0]),
            Domain::Primitive => Box::new(|p| topology.primitive(p as usize)[0]),
            _ => return None,
        })
    };
    let (Expand::Connected, false, Ok(()), Some(point)) =
        (how, s.is_all(), s.validate(on), point_of(s.domain()))
    else {
        return s.clone();
    };
    let pieces = &pieces(on).0;
    let mut held = vec![false; pieces.len()];
    for &i in s.indices().iter() {
        held[pieces[point(i) as usize] as usize] = true;
    }
    let count = on.count(s.domain()).unwrap_or(0) as u32;
    let ids = (0..count)
        .filter(|&i| held[pieces[point(i) as usize] as usize])
        .collect();
    Selection::new(s.domain(), ids, None, Some(on.topology().fingerprint()))
        .expect("no active element")
}

/// Each point's piece, by the smallest point in it.
struct Pieces(Vec<u32>);

/// `on`'s pieces: points joined through a primitive, and the first points
/// of primitives of one non-blank `name`, are one piece. The joins through
/// primitives are a cache on the topology (built once, shared by every
/// value holding it, so a drag or a rename rebuilds nothing of them);
/// names join over a copy of it.
fn pieces(on: &Polygons) -> Arc<Pieces> {
    let topology = on.topology();
    let connected = topology.derived(|t| {
        let mut parent: Vec<u32> = (0..t.points() as u32).collect();
        for p in 0..t.primitives() {
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
