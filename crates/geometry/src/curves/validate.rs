//! The curve rules ([`Curves`](super::Curves)' documentation lists them).
use super::{Basis, Curves, check_attributes};
use crate::error::{ErrorKind, GeometryError};
use crate::limits::{self, MAX_DEGREE, MAX_PRIMITIVES, MAX_VERTICES};
use crate::positions;

pub(super) fn curves(c: &Curves) -> Result<(), GeometryError> {
    positions::check(&c.positions)?;
    let whole = |m: String| Err(GeometryError::whole(ErrorKind::Curve, m));
    let at = |i: usize, m: String| Err(GeometryError::at(ErrorKind::Curve, i, m));
    if c.offsets.first() != Some(&0) {
        return whole("the offsets must start at 0".into());
    }
    let curves = c.offsets.len() - 1;
    limits::check(curves, MAX_PRIMITIVES, "curves")?;
    let vertices = match &c.points {
        Some(points) => {
            if let Some(v) = points.iter().position(|&p| p as usize >= c.positions.len()) {
                return Err(GeometryError::at(
                    ErrorKind::Curve,
                    v,
                    format!("vertex {v} names point {}, which is missing", points[v]),
                ));
            }
            points.len()
        }
        None => c.positions.len(),
    };
    limits::check(vertices, MAX_VERTICES, "vertices")?;
    if c.offsets[curves] as usize != vertices {
        return whole(format!(
            "the offsets end at {}, not at the vertex count {vertices}",
            c.offsets[curves]
        ));
    }
    if c.degrees.len() != curves {
        return whole(format!("{} degrees for {curves} curves", c.degrees.len()));
    }
    if c.periodic.elements() != curves {
        return whole(format!(
            "the periodic group is over {} curves, not {curves}",
            c.periodic.elements()
        ));
    }
    let nurbs = c.basis == Basis::Nurbs;
    match (&c.knots, nurbs) {
        (None, true) => return whole("NURBS curves need knots".into()),
        (Some(_), false) => {
            return whole(format!("{} curves take no knots", c.basis.name()));
        }
        _ => {}
    }
    if let Some(weights) = &c.weights {
        if !nurbs {
            return whole(format!("{} curves take no weights", c.basis.name()));
        }
        if weights.len() != vertices {
            return whole(format!("{} weights for {vertices} vertices", weights.len()));
        }
        if let Some(v) = weights.iter().position(|w| !(w.is_finite() && *w > 0.0)) {
            return Err(GeometryError::at(
                ErrorKind::Curve,
                v,
                format!("vertex {v}'s weight {} is not above 0", weights[v]),
            ));
        }
    }
    if let Some(knots) = &c.knots
        && (knots.offsets.len() != curves + 1
            || knots.offsets.first() != Some(&0)
            || knots.offsets[curves] as usize != knots.values.len())
    {
        return whole("the knot offsets must run from 0 to the knot count, one a curve".into());
    }
    for i in 0..curves {
        let (start, end) = (c.offsets[i], c.offsets[i + 1]);
        if end < start || end as usize > vertices {
            return at(i, format!("curve {i} has its offsets decreasing"));
        }
        let count = (end - start) as usize;
        let degree = c.degrees[i];
        let periodic = c.periodic.contains(i);
        if let Some(problem) = degree_rule(c.basis, degree) {
            return at(i, format!("curve {i} {problem}"));
        }
        if let Some(problem) = count_rule(c.basis, degree as usize, count, periodic) {
            return at(i, format!("curve {i} {problem}"));
        }
        if let Some(knots) = &c.knots {
            let (k0, k1) = (knots.offsets[i], knots.offsets[i + 1]);
            if k1 < k0
                || k1 as usize > knots.values.len()
                || (k1 - k0) as usize != count + degree as usize + 1
            {
                return at(
                    i,
                    format!(
                        "curve {i} has {} knots, not {} (count + degree + 1)",
                        k1.saturating_sub(k0),
                        count + degree as usize + 1
                    ),
                );
            }
            let row = knots.of(i);
            if row.iter().any(|k| !k.is_finite()) || row.windows(2).any(|w| w[1] < w[0]) {
                return at(i, format!("curve {i}'s knots decrease or are not finite"));
            }
            if row[degree as usize] >= row[count] {
                return at(i, format!("curve {i}'s knots span no interval"));
            }
        }
    }
    check_attributes(&c.attributes, None, |d| c.count(d))
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

/// What is wrong with `count` control vertices for `basis` at `degree`,
/// if anything.
fn count_rule(basis: Basis, degree: usize, count: usize, periodic: bool) -> Option<String> {
    let least = match basis {
        Basis::BSpline | Basis::Nurbs => degree + 1,
        _ => 2,
    };
    if count < least {
        return Some(format!("has {count} control points, fewer than {least}"));
    }
    if basis == Basis::Bezier {
        let fits = if periodic {
            count.is_multiple_of(degree)
        } else {
            (count - 1).is_multiple_of(degree)
        };
        if !fits {
            let rule = if periodic {
                "k·degree"
            } else {
                "k·degree + 1"
            };
            return Some(format!(
                "has {count} control points, not {rule} for degree {degree}"
            ));
        }
    }
    None
}
