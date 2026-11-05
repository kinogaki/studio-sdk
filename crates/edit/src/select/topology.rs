//! What the selection commands find over a polygon value's topology: the
//! loops two selected neighbours lie on ([`loops`]), a selection's rim
//! ([`rim`]), the open borders it touches ([`border`]) and the shortest way
//! between two selected elements ([`shortest_path`]).
use super::{
    Coverage, Expand, Ways, along_edges, at, boundary, convert_by, expand, selection_of, shrink,
};
use kinogaki_geometry::{Domain, Error, Geometry, Selection, Vec3};
use std::collections::HashSet;

/// The open edges joined to `near`'s points (Border): the holes it
/// touches. `None`: every open edge.
///
/// # Errors
/// As [`Selection::validate`].
pub fn border(near: Option<&Selection>, value: &Geometry) -> Result<Selection, Error> {
    let table = value.primitives();
    let index = table.index();
    let Some(near) = near else {
        return selection_of(value, Domain::Edge, index.boundary_edges().iter().copied());
    };
    let points = convert_by(near, value, Domain::Point, Coverage::Any)?;
    let mut seen = HashSet::new();
    let mut queue: Vec<u32> = points.members().iter().collect();
    let mut visited: HashSet<u32> = queue.iter().copied().collect();
    while let Some(p) = queue.pop() {
        for e in index.point_every_edge(p as usize) {
            if index.is_boundary(e as usize) && seen.insert(e) {
                for q in table.edge(e as usize).points {
                    if visited.insert(q) {
                        queue.push(q);
                    }
                }
            }
        }
    }
    selection_of(value, Domain::Edge, seen)
}

/// What's selected next to something that isn't (Selection Boundary): a
/// face selection's outline, as edges ([`boundary`]); of edges or points,
/// those at its rim.
///
/// # Errors
/// As [`Selection::validate`].
pub fn rim(s: &Selection, value: &Geometry) -> Result<Selection, Error> {
    if s.domain() == Domain::Primitive {
        return boundary(s, value);
    }
    let inside = shrink(s, value);
    selection_of(
        value,
        s.domain(),
        s.members().iter().filter(|&i| !inside.contains(i)),
    )
}

/// The loops two selected neighbours lie on: points' edge loops as
/// points, faces' loops across the edge they share, edges' own loops
/// ([`Expand::Loop`]).
///
/// # Errors
/// As [`Selection::validate`].
pub fn loops(s: &Selection, value: &Geometry) -> Result<Selection, Error> {
    s.validate(value)?;
    let table = value.primitives();
    let index = table.index();
    match s.domain() {
        Domain::Point => {
            let mut found: HashSet<u32> = s.members().iter().collect();
            for p in s.members().iter() {
                for q in index.point_neighbours(p as usize) {
                    if p < q && s.contains(q) {
                        let e = table.edge_of(p, q).expect("neighbours share an edge");
                        for e in index.edge_loop(e).edges {
                            found.extend(table.edge(e as usize).points);
                        }
                    }
                }
            }
            selection_of(value, Domain::Point, found)
        }
        Domain::Primitive => {
            let mut found: HashSet<u32> = s.members().iter().collect();
            for f in s.members().iter() {
                for e in index.primitive_edges(f as usize) {
                    let next = index.edge_faces(e as usize);
                    if next.len() == 2 && next.iter().all(|&g| s.contains(g)) {
                        for rung in index.edge_ring(e).edges {
                            found.extend(index.edge_faces(rung as usize).iter().copied());
                        }
                    }
                }
            }
            selection_of(value, Domain::Primitive, found)
        }
        _ => Ok(expand(s, value, &Expand::Loop)),
    }
}

/// The shortest way between two selected elements, by distance: points
/// and the edges between them along edges, faces across the edges they
/// share (from middle to middle).
///
/// # Errors
/// Unless exactly two points, edges or faces are selected and a way joins
/// them; as [`Selection::validate`].
pub fn shortest_path(s: &Selection, value: &Geometry) -> Result<Selection, Error> {
    s.validate(value)?;
    let ends: Vec<u32> = s.members().iter().collect();
    let [first, second] = ends[..] else {
        return Err(Error::invalid("Select two elements first"));
    };
    let table = value.primitives();
    let index = table.index();
    let way = |ways: Ways| {
        (ways.end().map(|end| ways.path(end))).ok_or_else(|| Error::invalid("Nothing joins them"))
    };
    match s.domain() {
        Domain::Point => {
            let ways = Ways::search(&[first], f64::INFINITY, |p| p == second, along_edges(value));
            selection_of(value, Domain::Point, way(ways)?)
        }
        Domain::Edge => {
            let (a, b) = (
                table.edge(first as usize).points,
                table.edge(second as usize).points,
            );
            let ways = Ways::search(&a, f64::INFINITY, |p| b.contains(&p), along_edges(value));
            let path = way(ways)?;
            let between = path.windows(2).filter_map(|w| table.edge_of(w[0], w[1]));
            selection_of(value, Domain::Edge, between.chain([first, second]))
        }
        Domain::Primitive => {
            let middle = |f: u32| {
                let row = table.primitive(f as usize);
                let sum = row.iter().fold(Vec3::ZERO, |sum, &p| sum + at(value, p));
                sum / row.len() as f64
            };
            let across = |f: u32, next: &mut dyn FnMut(u32, f64)| {
                for e in index.primitive_edges(f as usize) {
                    for &g in index.edge_faces(e as usize) {
                        if g != f {
                            next(g, (middle(g) - middle(f)).length());
                        }
                    }
                }
            };
            let ways = Ways::search(&[first], f64::INFINITY, |f| f == second, across);
            selection_of(value, Domain::Primitive, way(ways)?)
        }
        _ => Err(Error::invalid("A way runs between points, edges or faces")),
    }
}
