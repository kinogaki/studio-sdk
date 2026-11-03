//! The rule every value's positions follow: finite, each component at
//! most [`MAX_COORDINATE`] in magnitude, at most [`MAX_POINTS`] of them.
use crate::Error;
use crate::column::Column;
use crate::limits::{self, MAX_COORDINATE, MAX_POINTS};
use crate::parallel;
use crate::{Bounds, Vec3};

/// Points per parallel block.
const BLOCK: usize = 65_536;

/// Checks `positions`; the error names the first bad point in order.
pub(crate) fn check(positions: &Column<[f64; 3]>) -> Result<(), Error> {
    limits::check(positions.len(), MAX_POINTS, "points")?;
    // NaN fails every comparison.
    let fine = |p: &[f64; 3]| {
        (p[0].abs() <= MAX_COORDINATE)
            & (p[1].abs() <= MAX_COORDINATE)
            & (p[2].abs() <= MAX_COORDINATE)
    };
    match parallel::first_refused(positions, BLOCK, fine) {
        Some(i) => {
            Err(Error::invalid(format!("point {i} is not finite or is beyond 1e100")).element(i))
        }
        None => Ok(()),
    }
}

/// The smallest box holding `positions`; `None` for none.
pub(crate) fn bounds(positions: &[[f64; 3]]) -> Option<Bounds> {
    let points: &[Vec3] = bytemuck::cast_slice(positions);
    let boxes = parallel::map(points.len(), BLOCK, |range| {
        Bounds::of(points[range].iter().copied())
    });
    boxes.into_iter().flatten().reduce(Bounds::union)
}

/// [`bounds`], cached on the positions column.
pub(crate) fn cached_bounds(positions: &Column<[f64; 3]>) -> Option<Bounds> {
    /// The bounds of one positions column: this crate's cache.
    struct Cached(Option<Bounds>);
    positions.derived(|p| Cached(bounds(p))).0
}

/// The points whose position differs bit for bit between `before` and
/// `after`, ascending (every point past `before`'s count among them): what
/// a display patches when a value's points moved. Bitwise, so `-0.0` and
/// `0.0` differ (their bytes do) and equal bits never do.
pub(crate) fn moved(before: &[[f64; 3]], after: &[[f64; 3]]) -> Vec<u32> {
    let bits = |p: &[f64; 3]| p.map(f64::to_bits);
    let blocks = parallel::map(after.len(), BLOCK, |range| {
        (range.filter(|&p| before.get(p).is_none_or(|b| bits(b) != bits(&after[p]))))
            .map(|p| p as u32)
            .collect::<Vec<_>>()
    });
    blocks.into_iter().flatten().collect()
}
