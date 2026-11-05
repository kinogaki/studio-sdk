//! Typed values following a deformation ([`Editor::deform`]): where each
//! moved point goes is the deformation's; the values of a [`Meaning`] that
//! turns (other points, vectors, normals, rotations, transforms) take the
//! placement the deformation is near their element, by the same rules a
//! placement turns them with ([`place`](super::place)). That placement is the
//! deformation's local linear part (its Jacobian, by central differences)
//! and where it takes the element's place. A point or a vertex reads it at
//! its point, a primitive all of whose points move at its center, an edge
//! whose ends both move at its middle. `rest`, which names where a point
//! rests, stays; so does everything else.
//!
//! [`Meaning`]: kinogaki_geometry::Meaning
use super::Editor;
use super::place::{placed_each, turns};
use kinogaki_geometry::parallel::chunks_mut;
use kinogaki_geometry::{Attributes, Domain, Error, Geometry, Vec3, Xform, names};
use std::collections::{BTreeMap, HashMap};

/// A step small against any model and large against rounding: a
/// deformation is differenced over it.
const STEP: f64 = 1e-5;

/// Items a block of parallel work holds.
const BLOCK: usize = 2048;

/// A deformation: where the `i`th moved point, at a place, goes.
pub(super) type Field<'f> = &'f (dyn Fn(usize, Vec3) -> Vec3 + Sync);

/// The base elements whose typed values follow a deformation, each with
/// the placement it follows, by domain.
pub(super) type Followed = BTreeMap<Domain, Vec<(u32, Xform)>>;

/// The placement `field` is near `at`, for its `i`th point.
fn local(field: Field<'_>, i: usize, at: Vec3) -> Xform {
    let h = STEP * at.x.abs().max(at.y.abs()).max(at.z.abs()).max(1.0);
    let column = |axis: Vec3| (field(i, at + axis * h) - field(i, at - axis * h)) * (0.5 / h);
    let [x, y, z] = [column(Vec3::X), column(Vec3::Y), column(Vec3::Z)];
    let origin = field(i, at) - (x * at.x + y * at.y + z * at.z);
    Xform::from_axes(x, y, z, origin)
}

/// Whether `domain`'s attribute `name` turns with a deformation.
fn follows(attributes: &Attributes, domain: Domain, name: &str) -> bool {
    name != names::REST
        && matches!(
            domain,
            Domain::Point | Domain::Vertex | Domain::Primitive | Domain::Edge
        )
        && attributes.get(domain, name).is_some_and(|a| turns(a.ty()))
}

/// The domains of `value` holding values a deformation turns.
fn typed(value: &Geometry) -> Vec<Domain> {
    let attributes = value.attributes();
    let mut domains: Vec<Domain> = (attributes.iter())
        .filter(|(domain, name, _)| follows(attributes, *domain, name))
        .map(|(domain, _, _)| domain)
        .collect();
    domains.sort_unstable();
    domains.dedup();
    domains
}

/// What follows `field` moving the base's `points` from `from`: each
/// element of a typed domain whose points all move, with its placement.
pub(super) fn followed(
    base: &Geometry,
    points: &[u32],
    from: &[Vec3],
    field: Field<'_>,
) -> Result<Followed, Error> {
    let domains = typed(base);
    let mut out = Followed::new();
    if domains.is_empty() {
        return Ok(out);
    }
    let table = base.primitives();
    let index: HashMap<u32, usize> = (points.iter().enumerate())
        .filter(|&(_, &p)| (p as usize) < table.points())
        .map(|(i, &p)| (p, i))
        .collect();
    // Every moved point's placement: what its own values and its vertices'
    // follow.
    let mut at_points: Vec<Xform> = vec![Xform::IDENTITY; points.len()];
    if domains
        .iter()
        .any(|d| matches!(d, Domain::Point | Domain::Vertex))
    {
        chunks_mut(&mut at_points, BLOCK, |first, block| {
            for (k, slot) in block.iter_mut().enumerate() {
                *slot = local(field, first + k, from[first + k]);
            }
            Ok::<(), Error>(())
        })?;
    }
    // A place among the moved points' places, read by the first.
    let place = |members: &[u32]| -> Option<(usize, Vec3)> {
        let first = *index.get(members.first()?)?;
        let mut sum = Vec3::ZERO;
        for p in members {
            sum += from[*index.get(p)?];
        }
        Some((first, sum * (1.0 / members.len() as f64)))
    };
    let touched = || {
        let mut rows: Vec<u32> = (index.keys())
            .flat_map(|&p| {
                table
                    .index()
                    .point_primitives(p as usize)
                    .collect::<Vec<_>>()
            })
            .collect();
        rows.sort_unstable();
        rows.dedup();
        rows
    };
    for domain in domains {
        let elements: Vec<(u32, Xform)> = match domain {
            Domain::Point => (points.iter().zip(&at_points))
                .filter(|(p, _)| index.contains_key(p))
                .map(|(&p, &at)| (p, at))
                .collect(),
            Domain::Vertex => (touched().into_iter())
                .flat_map(|r| {
                    let range = table.primitive_vertices(r as usize);
                    let corners = table.primitive(r as usize);
                    (range.zip(corners))
                        .filter_map(|(v, p)| Some((v as u32, at_points[*index.get(p)?])))
                        .collect::<Vec<_>>()
                })
                .collect(),
            Domain::Primitive => (touched().into_iter())
                .filter_map(|r| {
                    let (i, center) = place(table.primitive(r as usize))?;
                    Some((r, local(field, i, center)))
                })
                .collect(),
            _ => {
                let edges = table.index();
                let mut ids: Vec<u32> = (index.keys())
                    .flat_map(|&p| edges.point_edges(p as usize).collect::<Vec<_>>())
                    .collect();
                ids.sort_unstable();
                ids.dedup();
                (ids.into_iter())
                    .filter_map(|e| {
                        let (i, middle) = place(&edges.edge(e as usize).points)?;
                        Some((e, local(field, i, middle)))
                    })
                    .collect()
            }
        };
        out.insert(domain, elements);
    }
    Ok(out)
}

/// `attributes` with what `followed` names turned, each element through
/// `number` (its number now; `None`: it went).
pub(super) fn turned(
    attributes: &Attributes,
    followed: &[Followed],
    number: impl Fn(Domain, u32) -> Option<u32>,
) -> Result<Attributes, Error> {
    let mut out = attributes.clone();
    for each in followed {
        for (&domain, elements) in each {
            let items: Vec<(u32, Xform)> = (elements.iter())
                .filter_map(|&(e, at)| Some((number(domain, e)?, at)))
                .collect();
            let current = out.clone();
            for (d, name, attribute) in current.iter().filter(|a| a.0 == domain) {
                if follows(&current, d, name) {
                    out = out.with(d, name, placed_each(attribute, &items)?)?;
                }
            }
        }
    }
    Ok(out)
}

impl Editor {
    /// The typed values following the deformations asked for, on `value`
    /// (moved points alone: the base's numbers).
    pub(super) fn follow_moved(&self, value: Geometry) -> Result<Geometry, Error> {
        if self.followed.is_empty() {
            return Ok(value);
        }
        let attributes = turned(value.attributes(), &self.followed, |_, e| Some(e))?;
        value.with_attributes(attributes)
    }
}
