//! The rule every value's positions follow: finite, each component at
//! most [`MAX_COORDINATE`] in magnitude, at most [`MAX_POINTS`] of them.
use crate::Error;
use crate::column::{Column, Origin};
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

/// What a positions column records of the column it was edited from: the
/// points that differ. This crate's cache; a column made any other way
/// records nothing.
struct Lineage(Option<(Origin<[f64; 3]>, Column<u32>)>);

/// `positions` with `points` at `at`: one copy of the column, only the
/// points given checked, the ones that differ recorded on the new column
/// for [`moved_between`]. Returns those points, ascending.
///
/// # Errors
/// A point past the column or a bad position, by the rule of [`check`].
pub(crate) fn with_moved(
    positions: &Column<[f64; 3]>,
    points: &[u32],
    at: &[[f64; 3]],
) -> Result<(Column<[f64; 3]>, Column<u32>), Error> {
    if points.len() != at.len() {
        return Err(Error::invalid(format!(
            "{} positions for {} points",
            at.len(),
            points.len()
        )));
    }
    let mut after = positions.to_vec();
    let mut moved = Vec::with_capacity(points.len());
    for (&p, &to) in points.iter().zip(at) {
        let Some(slot) = after.get_mut(p as usize) else {
            let last = positions.len();
            return Err(
                Error::invalid(format!("point {p} is past the last of {last}")).element(p as usize),
            );
        };
        if !to.iter().all(|c| c.abs() <= MAX_COORDINATE) {
            return Err(
                Error::invalid(format!("point {p} is not finite or is beyond 1e100"))
                    .element(p as usize),
            );
        }
        if slot.map(f64::to_bits) != to.map(f64::to_bits) {
            *slot = to;
            moved.push(p);
        }
    }
    moved.sort_unstable();
    moved.dedup();
    let after = Column::new(after);
    let moved = Column::new(moved);
    let record = (Origin::of(positions), moved.clone());
    after.derived(|_| Lineage(Some(record)));
    Ok((after, moved))
}

/// What [`moved`] would find between these two columns, when one was made
/// from the other by [`with_moved`]: known without comparing them.
pub(crate) fn moved_between(a: &Column<[f64; 3]>, b: &Column<[f64; 3]>) -> Option<Column<u32>> {
    let recorded = |after: &Column<[f64; 3]>, before: &Column<[f64; 3]>| {
        let lineage = after.derived(|_| Lineage(None));
        let (origin, points) = lineage.0.as_ref()?;
        origin.is(before).then(|| points.clone())
    };
    recorded(a, b).or_else(|| recorded(b, a))
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
