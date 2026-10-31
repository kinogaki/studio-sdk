//! The topology rules ([`Topology::new`](super::Topology::new)).
use crate::error::{ErrorKind, GeometryError};
use crate::limits::{self, MAX_POINTS, MAX_PRIMITIVES, MAX_VERTICES};
use crate::parallel;

/// Primitives per parallel block.
const BLOCK: usize = 16_384;

pub(super) fn topology(
    points: usize,
    offsets: &[u32],
    vertices: &[u32],
    polygons: usize,
) -> Result<(), GeometryError> {
    limits::check(points, MAX_POINTS, "points")?;
    limits::check(
        offsets.len().saturating_sub(1),
        MAX_PRIMITIVES,
        "primitives",
    )?;
    limits::check(vertices.len(), MAX_VERTICES, "vertices")?;
    let whole = |m: String| Err(GeometryError::whole(ErrorKind::Topology, m));
    if offsets.first() != Some(&0) {
        return whole("the offsets must start at 0".into());
    }
    if *offsets.last().expect("non-empty") as usize != vertices.len() {
        return whole(format!(
            "the offsets end at {}, not at the vertex count {}",
            offsets[offsets.len() - 1],
            vertices.len()
        ));
    }
    let primitives = offsets.len() - 1;
    if polygons > primitives {
        return whole(format!(
            "{polygons} polygons, but only {primitives} primitives"
        ));
    }
    let faults = parallel::map_blocks(primitives, BLOCK, |range| {
        range
            .into_iter()
            .find_map(|p| primitive(points, offsets, vertices, p, p < polygons).map(|m| (p, m)))
    });
    match faults.into_iter().flatten().next() {
        Some((p, problem)) => {
            let kind = if p < polygons { "polygon" } else { "polyline" };
            Err(GeometryError::at(
                ErrorKind::Topology,
                p,
                format!("{kind} {p} {problem}"),
            ))
        }
        None => Ok(()),
    }
}

/// What is wrong with primitive `p`, if anything.
fn primitive(
    points: usize,
    offsets: &[u32],
    vertices: &[u32],
    p: usize,
    polygon: bool,
) -> Option<&'static str> {
    let (start, end) = (offsets[p], offsets[p + 1]);
    if end < start {
        return Some("has its offsets decreasing");
    }
    if end as usize > vertices.len() {
        return Some("has its offsets past the vertices");
    }
    let ids = &vertices[start as usize..end as usize];
    if polygon && ids.len() < 3 {
        return Some("has fewer than three vertices");
    }
    if !polygon && ids.len() < 2 {
        return Some("has fewer than two vertices");
    }
    if ids.iter().any(|&v| v as usize >= points) {
        return Some("references a missing point");
    }
    if polygon && repeats(ids) {
        return Some("repeats a point");
    }
    if !polygon && ids.windows(2).any(|w| w[0] == w[1]) {
        return Some("repeats a point consecutively");
    }
    None
}

/// Whether any point is in `ids` twice.
fn repeats(ids: &[u32]) -> bool {
    match *ids {
        [a, b, c] => a == b || b == c || a == c,
        [a, b, c, d] => a == b || a == c || a == d || b == c || b == d || c == d,
        _ if ids.len() <= 8 => ids.iter().enumerate().any(|(i, v)| ids[..i].contains(v)),
        _ => {
            let mut sorted = ids.to_vec();
            sorted.sort_unstable();
            sorted.windows(2).any(|w| w[0] == w[1])
        }
    }
}
