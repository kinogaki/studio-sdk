//! The rule every value's positions follow: finite, each component at
//! most [`MAX_COORDINATE`] in magnitude, at most [`MAX_POINTS`] of them.
use crate::column::Column;
use crate::error::{ErrorKind, GeometryError};
use crate::limits::{self, MAX_COORDINATE, MAX_POINTS};
use crate::parallel;

/// Points per parallel block.
const BLOCK: usize = 65_536;

/// Checks `positions`; the error names the first bad point in order.
pub(crate) fn check(positions: &Column<[f64; 3]>) -> Result<(), GeometryError> {
    limits::check(positions.len(), MAX_POINTS, "points")?;
    // NaN fails every comparison.
    let fine = |p: &[f64; 3]| {
        (p[0].abs() <= MAX_COORDINATE)
            & (p[1].abs() <= MAX_COORDINATE)
            & (p[2].abs() <= MAX_COORDINATE)
    };
    match parallel::first_refused(positions, BLOCK, fine) {
        Some(i) => Err(GeometryError::at(
            ErrorKind::Position,
            i,
            format!("point {i} is not finite or is beyond 1e100"),
        )),
        None => Ok(()),
    }
}

/// The smallest box holding `positions`; `None` for none.
pub(crate) fn bounds(positions: &[[f64; 3]]) -> Option<[[f64; 3]; 2]> {
    let first = *positions.first()?;
    let boxes = parallel::map_blocks(positions.len(), BLOCK, |range| {
        positions[range].iter().fold([first, first], |[lo, hi], p| {
            [
                [lo[0].min(p[0]), lo[1].min(p[1]), lo[2].min(p[2])],
                [hi[0].max(p[0]), hi[1].max(p[1]), hi[2].max(p[2])],
            ]
        })
    });
    boxes.into_iter().reduce(|[lo, hi], [l, h]| {
        [
            [lo[0].min(l[0]), lo[1].min(l[1]), lo[2].min(l[2])],
            [hi[0].max(h[0]), hi[1].max(h[1]), hi[2].max(h[2])],
        ]
    })
}

/// [`bounds`], cached on the positions column.
pub(crate) fn cached_bounds(positions: &Column<[f64; 3]>) -> Option<[[f64; 3]; 2]> {
    /// The bounds of one positions column: this crate's cache.
    struct Bounds(Option<[[f64; 3]; 2]>);
    positions.derived(|p| Bounds(bounds(p))).0
}
