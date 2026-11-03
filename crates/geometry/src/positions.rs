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
    let mut first = 0;
    for run in positions.runs() {
        if let Some(i) = parallel::first_refused(run, BLOCK, fine) {
            return Err(beyond(first + i));
        }
        first += run.len();
    }
    Ok(())
}

/// [`check`], of the cells of `positions` it doesn't share with `base`.
pub(crate) fn check_after(
    positions: &Column<[f64; 3]>,
    base: &Column<[f64; 3]>,
) -> Result<(), Error> {
    limits::check(positions.len(), MAX_POINTS, "points")?;
    let bad = (positions.unshared(base))
        .filter(|r| r.start < positions.len())
        .find_map(|r| {
            (r.start..r.end.min(positions.len()))
                .find(|&p| !positions[p].iter().all(|c| c.abs() <= MAX_COORDINATE))
        });
    bad.map_or(Ok(()), |p| Err(beyond(p)))
}

/// The error of bad point `p`.
fn beyond(p: usize) -> Error {
    Error::invalid(format!("point {p} is not finite or is beyond 1e100")).element(p)
}

/// The smallest box holding `positions`; `None` for none.
pub(crate) fn bounds(positions: &Column<[f64; 3]>) -> Option<Bounds> {
    let mut boxes = Vec::new();
    for run in positions.runs() {
        let points: &[Vec3] = bytemuck::cast_slice(run);
        boxes.extend(parallel::map(points.len(), BLOCK, |range| {
            Bounds::of(points[range].iter().copied())
        }));
    }
    boxes.into_iter().flatten().reduce(Bounds::union)
}

/// [`bounds`], cached on the positions column.
pub(crate) fn cached_bounds(positions: &Column<[f64; 3]>) -> Option<Bounds> {
    /// The bounds of one positions column: this crate's cache.
    struct Cached(Option<Bounds>);
    positions.derived(|p| Cached(bounds(p))).0
}

/// `positions` with `points` at `at`: only the cells they fall in are
/// copied and only the points given checked.
///
/// # Errors
/// A point past the column or a bad position, by the rule of [`check`].
pub(crate) fn with_moved(
    positions: &Column<[f64; 3]>,
    points: &[u32],
    at: &[[f64; 3]],
) -> Result<Column<[f64; 3]>, Error> {
    if points.len() != at.len() {
        return Err(Error::invalid(format!(
            "{} positions for {} points",
            at.len(),
            points.len()
        )));
    }
    let mut after = positions.edit();
    for (&p, &to) in points.iter().zip(at) {
        let Some(&was) = positions.get(p as usize) else {
            let last = positions.len();
            return Err(
                Error::invalid(format!("point {p} is past the last of {last}")).element(p as usize),
            );
        };
        if !to.iter().all(|c| c.abs() <= MAX_COORDINATE) {
            return Err(beyond(p as usize));
        }
        if was.map(f64::to_bits) != to.map(f64::to_bits) {
            after[p as usize] = to;
        }
    }
    Ok(after.finish())
}

/// The points whose position differs bit for bit between `before` and
/// `after`, ascending (every point past `before`'s count among them): what
/// a display patches when a value's points moved. Bitwise, so `-0.0` and
/// `0.0` differ (their bytes do) and equal bits never do. Reads only the
/// cells the columns don't share.
pub(crate) fn moved(before: &Column<[f64; 3]>, after: &Column<[f64; 3]>) -> Vec<u32> {
    let bits = |p: &[f64; 3]| p.map(f64::to_bits);
    let ranges: Vec<_> = after
        .unshared(before)
        .filter(|r| r.start < after.len())
        .collect();
    let blocks = parallel::map(ranges.len(), 1, |pieces| {
        let mut out = Vec::new();
        for range in &ranges[pieces] {
            let range = range.start..range.end.min(after.len());
            out.extend(
                range
                    .filter(|&p| before.get(p).is_none_or(|b| bits(b) != bits(&after[p])))
                    .map(|p| p as u32),
            );
        }
        out
    });
    blocks.into_iter().flatten().collect()
}
