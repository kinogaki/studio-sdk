//! Selection rules: a [`Selection`] as another domain, the elements it
//! holds, how it is found again after its value changed
//! ([`reresolve`]), how a pick grows before it becomes a selection
//! ([`expand`]: connected pieces, edge loops and rings), what lies next
//! to it ([`grow`], [`shrink`], [`boundary`]), what the selection
//! commands find from it ([`loops`], [`rim`], [`border`],
//! [`shortest_path`]; [`similar`], [`by_normal`], [`flood_by_angle`],
//! [`hard_edges`]) and the one shortest-way search they and a soft
//! selection's reach share ([`Ways`]). Topology questions go to the
//! table's one index, so a rule costs what it touches.
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
mod alike;
mod expand;
mod near;
mod topology;
mod ways;

pub use alike::{ANGLE, by_normal, flood_by_angle, hard_edges, similar};
pub use expand::{Expand, Mode, expand};
pub use near::{boundary, grow, shrink};
pub use topology::{border, loops, rim, shortest_path};
pub use ways::{Ways, along_edges};

use kinogaki_geometry::{Column, Domain, Elements, Error, Geometry, Incidence, Selection, Vec3};
use std::collections::HashMap;
use std::sync::Mutex;

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
    if to == s.domain() {
        s.validate(on)?;
        return Ok(s.clone());
    }
    Selection::on(on, to, members_by(s, on, to, coverage)?, None)
}

/// The elements [`convert_by`] selects, without the selection made of them
/// (its stable ids): what a count or a pass over them needs. What it finds
/// is kept with the selection for the table it was found on
/// ([`Selection::derived`]), so those asking the same of one selection (a
/// host's counts and a context's pivot, after a box) convert it once.
///
/// # Errors
/// As [`convert`].
pub fn members_by(
    s: &Selection,
    on: &dyn Incidence,
    to: Domain,
    coverage: Coverage,
) -> Result<Elements, Error> {
    s.validate(on)?;
    if to == s.domain() {
        return Ok(s.members().clone());
    }
    let key = (on.fingerprint(), to, coverage);
    let kept = s.derived(Converted::default);
    if let Some(found) = kept.find(&key) {
        return Ok(found);
    }
    let found = converted(s, on, to, coverage)?;
    kept.keep(key, found.clone());
    Ok(found)
}

/// What one selection came to, by the table, the domain and the coverage
/// of the conversion (the last few asked).
#[derive(Default)]
struct Converted(Mutex<Vec<(ConversionKey, Elements)>>);

/// A conversion: the table's fingerprint, the domain and how.
type ConversionKey = ([u8; 32], Domain, Coverage);

impl Converted {
    /// Conversions kept at most: the levels a host and a context ask for.
    const KEPT: usize = 4;

    fn find(&self, key: &ConversionKey) -> Option<Elements> {
        let kept = self.0.lock().unwrap_or_else(|e| e.into_inner());
        kept.iter().find(|(k, _)| k == key).map(|(_, e)| e.clone())
    }

    fn keep(&self, key: ConversionKey, found: Elements) {
        let mut kept = self.0.lock().unwrap_or_else(|e| e.into_inner());
        if kept.iter().any(|(k, _)| *k == key) {
            return;
        }
        if kept.len() == Self::KEPT {
            kept.remove(0);
        }
        kept.push((key, found));
    }
}

/// [`members_by`] worked out.
fn converted(
    s: &Selection,
    on: &dyn Incidence,
    to: Domain,
    coverage: Coverage,
) -> Result<Elements, Error> {
    let targets = target(on, to)?;
    let sources = on.count(s.domain()).unwrap_or(0);
    // A few elements cost what they touch, not what the value holds.
    if s.members().len().saturating_mul(FEW) < sources
        && let Some(members) = few_elements(s, on, to, coverage, targets)?
    {
        return Ok(members);
    }
    let mut chosen = vec![false; sources];
    for i in s.members().iter() {
        chosen[i as usize] = true;
    }
    match by_targets(s, on, to, coverage, targets, &chosen)? {
        Some(members) => Ok(members),
        None => Ok(by_incidence(s, on, to, coverage, targets, &chosen)),
    }
}

/// What [`convert_by`] makes of `s` (its members flagged in `chosen`) in
/// one pass over every incidence of its domain with `to`.
fn by_incidence(
    s: &Selection,
    on: &dyn Incidence,
    to: Domain,
    coverage: Coverage,
    targets: usize,
    chosen: &[bool],
) -> Elements {
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
    Elements::from_fn(targets, |i| {
        touched[i as usize] && (!whole || !missing[i as usize])
    })
}

/// A selection this many times smaller than its domain converts by
/// [`few_elements`].
const FEW: usize = 8;

/// What [`convert_by`] makes of `s` through its elements alone; `None` when
/// `on` has no such way for this pair of domains.
fn few_elements(
    s: &Selection,
    on: &dyn Incidence,
    to: Domain,
    coverage: Coverage,
    targets: usize,
) -> Result<Option<Elements>, Error> {
    let mut touched = Vec::new();
    let asked = &mut s.members().iter();
    if !on.incident_of(s.domain(), to, asked, &mut |_, target| touched.push(target)) {
        return Ok(None);
    }
    touched.sort_unstable();
    touched.dedup();
    if coverage == Coverage::Whole {
        let mut missing = Vec::new();
        let asked = &mut touched.iter().copied();
        let known = on.incident_of(to, s.domain(), asked, &mut |target, source| {
            if !s.members().contains(source) {
                missing.push(target);
            }
        });
        if !known {
            return Ok(None);
        }
        missing.sort_unstable();
        touched.retain(|target| missing.binary_search(target).is_err());
    }
    Elements::from_indices(targets, touched).map(Some)
}

/// Targets a worker decides at once in [`by_targets`].
const TARGETS: usize = 16_384;

/// What [`convert_by`] makes of `s` (its members flagged in `chosen`)
/// target by target, each looking at the elements it touches, on the
/// workers: the same members as one pass over every incidence, at the
/// cost of the targets divided among threads. `None` when `on` can't list
/// a target's elements of `s`'s domain.
fn by_targets(
    s: &Selection,
    on: &dyn Incidence,
    to: Domain,
    coverage: Coverage,
    targets: usize,
    chosen: &[bool],
) -> Result<Option<Elements>, Error> {
    let whole = coverage == Coverage::Whole;
    let blocks = kinogaki_geometry::parallel::map_blocks(targets, TARGETS, |range| {
        let first = range.start;
        // Per target of the block: one of its elements chosen, and one not.
        let mut touched = vec![false; range.len()];
        let mut missing = vec![false; range.len()];
        let listed = on.incident_of(
            to,
            s.domain(),
            &mut range.map(|t| t as u32),
            &mut |target, source| {
                let at = target as usize - first;
                match chosen.get(source as usize) {
                    Some(true) => touched[at] = true,
                    _ => missing[at] = true,
                }
            },
        );
        let picked = (0..touched.len()).filter(|&k| touched[k] && (!whole || !missing[k]));
        let picked: Vec<u32> = picked.map(|k| (first + k) as u32).collect();
        Ok::<_, std::convert::Infallible>(listed.then_some(picked))
    });
    let Ok(blocks) = blocks;
    let picked: Option<Vec<Vec<u32>>> = blocks.into_iter().collect();
    picked
        .map(|blocks| Elements::from_indices(targets, blocks.concat()))
        .transpose()
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

/// `members` of `domain` on `value`, as a selection.
fn selection_of(
    value: &Geometry,
    domain: Domain,
    members: impl IntoIterator<Item = u32>,
) -> Result<Selection, Error> {
    let count = value.count(domain).unwrap_or(0);
    let members = Elements::from_indices(count, members.into_iter().collect::<Vec<_>>())?;
    Selection::on(value, domain, members, None)
}

/// Point `p`'s position.
fn at(value: &Geometry, p: u32) -> Vec3 {
    value.positions()[p as usize].into()
}

#[cfg(test)]
mod tests {
    use super::*;
    use kinogaki_geometry::{Attributes, Kind, Primitives};

    /// A 9 × 9 grid of quads, one triangle fanned in, a polyline and a
    /// loose point: every kind of incidence a conversion meets.
    fn mixed() -> Geometry {
        let n = 9u32;
        let side = n + 1;
        let mut offsets = vec![0u32];
        let mut corners = Vec::new();
        for j in 0..n {
            for i in 0..n {
                let a = j * side + i;
                corners.extend([a, a + 1, a + side + 1, a + side]);
                offsets.push(corners.len() as u32);
            }
        }
        corners.extend([0, side * side, 1]);
        offsets.push(corners.len() as u32);
        corners.extend([2, 3, 4]);
        offsets.push(corners.len() as u32);
        let points = side * side + 2;
        let mut kinds = vec![Kind::POLYGON; offsets.len() - 2];
        kinds.push(Kind::POLYLINE);
        let topology = Primitives::new(
            points as usize,
            offsets.into(),
            corners.into(),
            kinds.into(),
            None,
        )
        .expect("polygons and a polyline");
        let positions = (0..points)
            .map(|p| [p as f64, 0.0, 0.0])
            .collect::<Vec<_>>();
        Geometry::new(positions.into(), topology, Attributes::new()).expect("a value")
    }

    #[test]
    fn target_by_target_conversions_are_one_pass_conversions() {
        // Random selections of every domain, as every other, both ways of
        // covering: the workers' answer is the one-pass answer.
        let value = mixed();
        let domains = [
            Domain::Point,
            Domain::Edge,
            Domain::Primitive,
            Domain::Vertex,
        ];
        let mut state = 0x9e37_79b9_7f4a_7c15u64;
        let mut next = move || {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state
        };
        let mut compared = 0;
        for from in domains {
            let count = value.count(from).expect("a domain");
            for density in [3, 50, 97] {
                let members = (0..count as u32).filter(|_| next() % 100 < density);
                let members = Elements::from_indices(count, members.collect::<Vec<_>>()).unwrap();
                let s = Selection::on(&value, from, members, None).unwrap();
                let chosen: Vec<bool> = (0..count as u32).map(|i| s.contains(i)).collect();
                for to in domains.into_iter().filter(|&to| to != from) {
                    let targets = value.count(to).unwrap();
                    for coverage in [Coverage::Any, Coverage::Whole] {
                        let one = by_incidence(&s, &value, to, coverage, targets, &chosen);
                        let each = by_targets(&s, &value, to, coverage, targets, &chosen).unwrap();
                        if let Some(each) = each {
                            compared += 1;
                            assert_eq!(
                                each.indices().to_vec(),
                                one.indices().to_vec(),
                                "{from:?} to {to:?} by {coverage:?}"
                            );
                        }
                    }
                }
            }
        }
        // Points, edges and primitives each way (vertices go one pass).
        assert_eq!(compared, 6 * 3 * 2);
    }
}
