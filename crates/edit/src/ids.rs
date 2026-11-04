//! Stable ids: the one rule kernels keep the standard `id` attribute by.
//!
//! Kept elements keep their ids; new or copied elements take
//! [`fresh`] ones, above every id the value holds; a Merge keeps its first
//! input's ids and gives later inputs' colliding (or missing) ids fresh
//! ones ([`merged`]). So an id names one element for as long as it lives,
//! whatever is inserted or removed around it, and a selection holding ids
//! is found again after a change upstream ([`reresolve`](crate::select::reresolve)).
use kinogaki_geometry::{
    Attribute, Column, Domain, Error, Geometry, Incidence, Meaning, highest_id, names,
};
use std::collections::{HashMap, HashSet};
use std::ops::Range;

/// `value` with ids on its points and primitives, where it has none: what
/// stored edits name elements by. Those without take their numbers.
///
/// ```
/// use kinogaki_edit::ids;
/// use kinogaki_geometry::{Attributes, Domain, Geometry, Incidence, Primitives};
///
/// let t = Primitives::of_polygons(3, vec![0, 3].into(), vec![0, 1, 2].into()).unwrap();
/// let bare = Geometry::new(vec![[0.0; 3]; 3].into(), t, Attributes::new()).unwrap();
/// let numbered = ids::numbered(&bare).unwrap();
/// assert_eq!(numbered.ids(Domain::Point).unwrap().to_vec(), [0, 1, 2]);
/// assert_eq!(numbered.ids(Domain::Primitive).unwrap().to_vec(), [0]);
/// ```
///
/// # Errors
/// What the attribute table refuses.
pub fn numbered(value: &Geometry) -> Result<Geometry, Error> {
    if value.ids(Domain::Point).is_some() && value.ids(Domain::Primitive).is_some() {
        return Ok(value.clone());
    }
    let mut attributes = (**value.attributes()).clone();
    for domain in [Domain::Point, Domain::Primitive] {
        if value.ids(domain).is_none() {
            let count = value.count(domain).unwrap_or(0) as i64;
            let ids = Attribute::tuples((0..count).collect::<Vec<i64>>(), Meaning::None)?;
            attributes = attributes.with(domain, names::ID, ids)?;
        }
    }
    value.with_attributes(attributes)
}

/// The elements of `domain` in `value` with these ids, in their order;
/// those gone left out.
///
/// ```
/// use kinogaki_edit::ids;
/// use kinogaki_geometry::{Attributes, Domain, Geometry, Primitives};
///
/// let t = Primitives::of_polygons(3, vec![0, 3].into(), vec![0, 1, 2].into()).unwrap();
/// let bare = Geometry::new(vec![[0.0; 3]; 3].into(), t, Attributes::new()).unwrap();
/// let numbered = ids::numbered(&bare).unwrap();
/// assert_eq!(ids::elements(&numbered, Domain::Point, &[2, 7, 0]), [2, 0]);
/// ```
pub fn elements(value: &Geometry, domain: Domain, ids: &[i64]) -> Vec<u32> {
    let Some(now) = value.ids(domain) else {
        return Vec::new();
    };
    let at: HashMap<i64, u32> = (now.iter().enumerate())
        .map(|(i, &id)| (id, i as u32))
        .collect();
    ids.iter().filter_map(|id| at.get(id).copied()).collect()
}

/// `count` ids above every one of `existing`: what new or copied
/// elements take. `None` when they would pass `i64::MAX`.
///
/// ```
/// use kinogaki_edit::ids;
///
/// let some = |ids: Vec<i64>| ids.into();
/// assert_eq!(ids::fresh(&some(vec![4, 9, 2]), 3), Some(10..13));
/// assert_eq!(ids::fresh(&some(vec![]), 2), Some(0..2));
/// assert_eq!(ids::fresh(&some(vec![i64::MAX]), 1), None);
/// ```
pub fn fresh(existing: &Column<i64>, count: usize) -> Option<Range<i64>> {
    let start = match highest_id(existing) {
        Some(max) => max.checked_add(1)?,
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
    let mut next = fresh(&Column::new(every), 0)?.start;
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
