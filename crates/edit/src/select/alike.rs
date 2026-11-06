//! What the selection commands find by likeness: elements like the selected
//! ones ([`similar`]), faces by their facing ([`by_normal`],
//! [`flood_by_angle`]) and the hard edges of the pieces selected edges lie
//! on ([`hard_edges`]).
use super::{Expand, at, expand, selection_of};
use kinogaki_geometry::{Domain, Error, Geometry, Selection, Vec3};
use std::collections::{BTreeMap, HashSet};

/// How far two facings may differ, in degrees, for [`by_normal`],
/// [`flood_by_angle`] and [`hard_edges`].
pub const ANGLE: f64 = 30.0;

/// How near two measures count as alike, as a part of their size.
const ALIKE: f64 = 0.01;

/// The cosine of [`ANGLE`].
fn within() -> f64 {
    ANGLE.to_radians().cos()
}

/// Polygon `face`'s unit normal (zero for a polygon without area).
fn normal(value: &Geometry, face: u32) -> Vec3 {
    value
        .polygon_normal(face as usize)
        .normalized()
        .unwrap_or_default()
}

/// What an element is compared by: its kind (corners, faces on an edge,
/// edges at a point) and a size within it (area, length).
fn measure(value: &Geometry, domain: Domain, i: u32) -> (usize, f64) {
    let table = value.primitives();
    let index = table.index();
    match domain {
        Domain::Primitive => (
            table.primitive(i as usize).len(),
            value.polygon_normal(i as usize).length() * 0.5,
        ),
        Domain::Edge => {
            let [a, b] = table.edge(i as usize).points;
            (
                index.edge_faces(i as usize).len(),
                (at(value, a) - at(value, b)).length(),
            )
        }
        _ => (index.point_neighbours(i as usize).count(), 0.0),
    }
}

/// Every element like a selected one (Select Similar): faces of as many
/// corners and about the area, edges of as many faces and about the
/// length, points of as many edges.
///
/// # Errors
/// As [`Selection::validate`].
pub fn similar(s: &Selection, value: &Geometry) -> Result<Selection, Error> {
    s.validate(value)?;
    let domain = s.domain();
    let mut kinds: BTreeMap<usize, Vec<f64>> = BTreeMap::new();
    for i in s.members().iter() {
        let (kind, size) = measure(value, domain, i);
        kinds.entry(kind).or_default().push(size);
    }
    kinds
        .values_mut()
        .for_each(|sizes| sizes.sort_by(f64::total_cmp));
    let count = value.count(domain).unwrap_or(0) as u32;
    let like = (0..count).filter(|&i| {
        let (kind, size) = measure(value, domain, i);
        kinds.get(&kind).is_some_and(|sizes| {
            let at = sizes.partition_point(|&other| other < size);
            let near = |other: Option<&f64>| {
                other.is_some_and(|other| {
                    (other - size).abs() <= ALIKE * other.abs().max(size.abs())
                })
            };
            near(sizes.get(at)) || near(at.checked_sub(1).and_then(|at| sizes.get(at)))
        })
    });
    selection_of(value, domain, like)
}

/// Every face facing within [`ANGLE`] of the selected faces' facing,
/// anywhere.
///
/// # Errors
/// As [`Selection::validate`].
pub fn by_normal(faces: &Selection, value: &Geometry) -> Result<Selection, Error> {
    faces.validate(value)?;
    let sum =
        (faces.members().iter()).fold(Vec3::ZERO, |sum, f| sum + value.polygon_normal(f as usize));
    let facing = sum.normalized().unwrap_or_default();
    let count = value.count(Domain::Primitive).unwrap_or(0) as u32;
    selection_of(
        value,
        Domain::Primitive,
        (0..count).filter(|&f| normal(value, f).dot(facing) >= within()),
    )
}

/// The faces reached from the selected ones crossing only edges that turn
/// less than [`ANGLE`]: the flat or gently curved patch around them.
///
/// # Errors
/// As [`Selection::validate`].
pub fn flood_by_angle(faces: &Selection, value: &Geometry) -> Result<Selection, Error> {
    faces.validate(value)?;
    let index = value.primitives().index();
    let mut seen: HashSet<u32> = faces.members().iter().collect();
    let mut queue: Vec<u32> = seen.iter().copied().collect();
    while let Some(f) = queue.pop() {
        let facing = normal(value, f);
        for e in index.primitive_edges(f as usize) {
            for &g in index.edge_faces(e as usize) {
                if !seen.contains(&g) && normal(value, g).dot(facing) >= within() {
                    seen.insert(g);
                    queue.push(g);
                }
            }
        }
    }
    selection_of(value, Domain::Primitive, seen)
}

/// The edges turning at least [`ANGLE`] on the pieces the selected edges
/// lie on: the hard edges.
///
/// # Errors
/// As [`Selection::validate`].
pub fn hard_edges(edges: &Selection, value: &Geometry) -> Result<Selection, Error> {
    edges.validate(value)?;
    let on = expand(edges, value, &Expand::Connected);
    let index = value.primitives().index();
    selection_of(
        value,
        Domain::Edge,
        on.members().iter().filter(|&e| {
            let faces = index.edge_faces(e as usize);
            faces.len() == 2 && normal(value, faces[0]).dot(normal(value, faces[1])) < within()
        }),
    )
}
