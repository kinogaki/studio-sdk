//! The primitive rules ([`Primitives`](super::Primitives) lists them):
//! one rule per kind, the curve rules shared with the curves value.
use super::{Basis, CurveData, Kind, Kinds};
use crate::Error;
use crate::limits::{self, MAX_DEGREE, MAX_POINTS, MAX_PRIMITIVES, MAX_VERTICES};
use crate::parallel;
use crate::spline::{self, KnotFault};

/// Primitives per parallel block.
const BLOCK: usize = 16_384;

pub(super) fn primitives(
    points: usize,
    offsets: &[u32],
    vertices: &[u32],
    kinds: &Kinds,
    primitives: usize,
    curves: Option<&CurveData>,
) -> Result<(), Error> {
    limits::check(points, MAX_POINTS, "points")?;
    limits::check(primitives, MAX_PRIMITIVES, "primitives")?;
    limits::check(vertices.len(), MAX_VERTICES, "vertices")?;
    let whole = |m: String| Err(Error::invalid(m));
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
    let kind = |p: usize| match kinds {
        Kinds::All(k) => *k,
        Kinds::Each(k) => k[p],
    };
    let curved = match kinds {
        Kinds::All(k) => primitives > 0 && !k.is_linear(),
        Kinds::Each(k) => k.iter().any(|k| !k.is_linear()),
    };
    match (curved, curves) {
        (true, None) => return whole("curve primitives need curve data".into()),
        (false, Some(_)) => return whole("curve data without a curve primitive".into()),
        (_, Some(c)) => curve_columns(c, primitives, vertices.len())?,
        _ => {}
    }
    let faults = parallel::blocks(primitives, BLOCK, |range| {
        range
            .into_iter()
            .find_map(|p| primitive(points, offsets, vertices, kind(p), curves, p).map(|m| (p, m)))
    });
    match faults.into_iter().flatten().next() {
        Some((p, problem)) => {
            let what = match kind(p) {
                Kind::POLYGON => "polygon",
                Kind::POLYLINE => "polyline",
                _ => "curve",
            };
            Err(Error::invalid(format!("{what} {p} {problem}")).element(p))
        }
        None => Ok(()),
    }
}

/// The curve data's columns sized to the table.
fn curve_columns(c: &CurveData, primitives: usize, vertices: usize) -> Result<(), Error> {
    let whole = |m: String| Err(Error::invalid(m));
    if c.degrees().len() != primitives {
        return whole(format!(
            "{} degrees for {primitives} primitives",
            c.degrees().len()
        ));
    }
    let knots = c.knots();
    if knots.offsets().len() != primitives + 1
        || knots.offsets().first() != Some(&0)
        || knots.offsets()[primitives] as usize != knots.values().len()
        || knots.offsets().as_slice().windows(2).any(|w| w[1] < w[0])
    {
        return whole("the knot offsets must run from 0 to the knot count, one a primitive".into());
    }
    if let Some(weights) = c.weights()
        && weights.len() != vertices
    {
        return whole(format!("{} weights for {vertices} vertices", weights.len()));
    }
    Ok(())
}

/// What is wrong with primitive `p` of `kind`, if anything.
fn primitive(
    points: usize,
    offsets: &[u32],
    vertices: &[u32],
    kind: Kind,
    curves: Option<&CurveData>,
    p: usize,
) -> Option<String> {
    let (start, end) = (offsets[p], offsets[p + 1]);
    if end < start {
        return Some("has its offsets decreasing".into());
    }
    if end as usize > vertices.len() {
        return Some("has its offsets past the vertices".into());
    }
    let ids = &vertices[start as usize..end as usize];
    if ids.iter().any(|&v| v as usize >= points) {
        return Some("references a missing point".into());
    }
    let degree = curves.map_or(1, |c| c.degrees()[p]);
    let knots = curves.map_or(&[][..], |c| c.knots().of(p));
    let weights = curves
        .and_then(|c| c.weights())
        .map(|w| &w.as_slice()[start as usize..end as usize]);
    if kind.is_linear() {
        if degree != 1 || !knots.is_empty() || weights.is_some_and(|w| w.iter().any(|&w| w != 1.0))
        {
            return Some("is linear but has curve data".into());
        }
        return linear(ids, kind.closed()).map(String::from);
    }
    curve(kind, degree, ids.len(), knots, weights)
}

/// What is wrong with a linear primitive's points, if anything.
pub(super) fn linear(ids: &[u32], closed: bool) -> Option<&'static str> {
    if closed && ids.len() < 3 {
        return Some("has fewer than three vertices");
    }
    if !closed && ids.len() < 2 {
        return Some("has fewer than two vertices");
    }
    if closed && repeats(ids) {
        return Some("repeats a point");
    }
    if !closed && ids.windows(2).any(|w| w[0] == w[1]) {
        return Some("repeats a point consecutively");
    }
    None
}

/// What is wrong with a curve of `kind`, `degree` and `count` control
/// points over `knots` and `weights`, if anything: the curve rules every
/// curve shares.
pub(crate) fn curve(
    kind: Kind,
    degree: u8,
    count: usize,
    knots: &[f64],
    weights: Option<&[f64]>,
) -> Option<String> {
    let basis = kind.basis();
    if let Some(problem) = degree_rule(basis, degree) {
        return Some(problem);
    }
    if let Some(problem) = count_rule(basis, degree as usize, count, kind.closed()) {
        return Some(problem);
    }
    if basis != Basis::Nurbs {
        if !knots.is_empty() {
            return Some(format!("is {} but has knots", basis.name()));
        }
        if weights.is_some_and(|w| w.iter().any(|&w| w != 1.0)) {
            return Some(format!("is {} but has weights", basis.name()));
        }
        return None;
    }
    if let Some(at) = weights.and_then(|w| w.iter().position(|w| !(w.is_finite() && *w > 0.0))) {
        return Some(format!("has a weight not above 0 at its vertex {at}"));
    }
    match spline::nurbs_check(degree, knots, count, false) {
        Ok(()) => None,
        Err(KnotFault::Count) => Some(format!(
            "has {} knots, not {} (count + degree + 1)",
            knots.len(),
            count + degree as usize + 1
        )),
        Err(KnotFault::Span) => Some("has knots that span no interval".into()),
        Err(_) => Some("has knots that decrease or are not finite".into()),
    }
}

/// What is wrong with `degree` for `basis`, if anything.
fn degree_rule(basis: Basis, degree: u8) -> Option<String> {
    if degree == 0 || degree > MAX_DEGREE {
        return Some(format!("has degree {degree}, not 1 to {MAX_DEGREE}"));
    }
    match basis {
        Basis::Linear if degree != 1 => Some(format!("is linear but of degree {degree}")),
        Basis::CatmullRom if degree != 3 => {
            Some(format!("is Catmull-Rom but of degree {degree}, not 3"))
        }
        _ => None,
    }
}

/// What is wrong with `count` control points for `basis` at `degree`,
/// if anything.
fn count_rule(basis: Basis, degree: usize, count: usize, closed: bool) -> Option<String> {
    let least = match basis {
        Basis::BSpline | Basis::Nurbs => degree + 1,
        _ => 2,
    };
    if count < least {
        return Some(format!("has {count} control points, fewer than {least}"));
    }
    if basis == Basis::Bezier {
        let fits = if closed {
            count.is_multiple_of(degree)
        } else {
            (count - 1).is_multiple_of(degree)
        };
        if !fits {
            let rule = if closed { "k·degree" } else { "k·degree + 1" };
            return Some(format!(
                "has {count} control points, not {rule} for degree {degree}"
            ));
        }
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
