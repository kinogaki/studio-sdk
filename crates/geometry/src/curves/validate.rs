//! The curve rules ([`Curves`](super::Curves)' documentation lists them).
use super::{Basis, Curves, check_attributes};
use crate::error::{ErrorKind, GeometryError};
use crate::limits::{self, MAX_PRIMITIVES, MAX_VERTICES};
use crate::positions;
use crate::primitives::{Kind, validate};

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
    if c.periodic.count() != curves {
        return whole(format!(
            "the periodic group is over {} curves, not {curves}",
            c.periodic.count()
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
    }
    if let Some(knots) = &c.knots
        && (knots.offsets().len() != curves + 1
            || knots.offsets().first() != Some(&0)
            || knots.offsets()[curves] as usize != knots.values().len()
            || knots.offsets().windows(2).any(|w| w[1] < w[0]))
    {
        return whole("the knot offsets must run from 0 to the knot count, one a curve".into());
    }
    for i in 0..curves {
        let (start, end) = (c.offsets[i], c.offsets[i + 1]);
        if end < start || end as usize > vertices {
            return at(i, format!("curve {i} has its offsets decreasing"));
        }
        let kind = Kind::curve(c.basis, c.periodic.contains(i as u32));
        let knots = c.knots.as_ref().map_or(&[][..], |k| k.of(i));
        let weights = (c.weights.as_ref()).map(|w| &w[start as usize..end as usize]);
        let count = (end - start) as usize;
        if let Some(problem) = validate::curve(kind, c.degrees[i], count, knots, weights) {
            return at(i, format!("curve {i} {problem}"));
        }
    }
    check_attributes(&c.attributes, None, |d| c.count(d))
}
