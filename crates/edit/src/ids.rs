//! Stable ids: the one rule kernels keep the standard `id` attribute by.
//!
//! Kept elements keep their ids; new or copied elements take
//! [`fresh`] ones, above every id the value holds; a Merge keeps its first
//! input's ids and gives later inputs' colliding (or missing) ids fresh
//! ones ([`merged`]). So an id names one element for as long as it lives,
//! whatever is inserted or removed around it, and a selection holding ids
//! is found again after a change upstream ([`reresolve`](crate::select::reresolve)).
use std::collections::HashSet;
use std::ops::Range;

/// `count` ids above every one of `existing`: what new or copied
/// elements take. `None` when they would pass `i64::MAX`.
///
/// ```
/// use kinogaki_edit::ids;
///
/// assert_eq!(ids::fresh(&[4, 9, 2], 3), Some(10..13));
/// assert_eq!(ids::fresh(&[], 2), Some(0..2));
/// assert_eq!(ids::fresh(&[i64::MAX], 1), None);
/// ```
pub fn fresh(existing: &[i64], count: usize) -> Option<Range<i64>> {
    let start = match existing.iter().max() {
        Some(&max) => max.checked_add(1)?,
        None => 0,
    };
    let end = start.checked_add(i64::try_from(count).ok()?)?;
    Some(start..end)
}

/// One Merge input's elements of a domain, for [`merged`].
#[derive(Clone, Copy, Debug)]
pub enum Input<'a> {
    /// Elements carrying these ids.
    Ids(&'a [i64]),
    /// This many elements without ids.
    Without(usize),
}

impl Input<'_> {
    fn len(&self) -> usize {
        match self {
            Input::Ids(ids) => ids.len(),
            Input::Without(count) => *count,
        }
    }
}

/// The ids of the inputs' elements merged in order: the first input's
/// kept, every later id kept unless an earlier element took it, and the
/// colliding ones (and the elements without ids) given fresh ids above
/// all of them, in element order. `None` when the fresh ids would pass
/// `i64::MAX`.
///
/// ```
/// use kinogaki_edit::ids::{self, Input};
///
/// let merged = ids::merged(&[Input::Ids(&[0, 1, 2]), Input::Ids(&[2, 7]), Input::Without(1)]);
/// assert_eq!(merged.unwrap(), [0, 1, 2, 8, 7, 9]);
/// ```
pub fn merged(inputs: &[Input<'_>]) -> Option<Vec<i64>> {
    let total = inputs.iter().map(Input::len).sum();
    let every: Vec<i64> = inputs
        .iter()
        .flat_map(|input| match input {
            Input::Ids(ids) => *ids,
            Input::Without(_) => &[],
        })
        .copied()
        .collect();
    let mut next = fresh(&every, 0)?.start;
    let mut taken = HashSet::with_capacity(total);
    let mut out = Vec::with_capacity(total);
    for input in inputs {
        let ids: Box<dyn Iterator<Item = Option<i64>>> = match input {
            Input::Ids(ids) => Box::new(ids.iter().map(|&id| Some(id))),
            Input::Without(count) => Box::new(std::iter::repeat_n(None, *count)),
        };
        for id in ids {
            let id = match id.filter(|&id| taken.insert(id)) {
                Some(id) => id,
                None => {
                    let id = next;
                    next = next.checked_add(1)?;
                    taken.insert(id);
                    id
                }
            };
            out.push(id);
        }
    }
    Some(out)
}
