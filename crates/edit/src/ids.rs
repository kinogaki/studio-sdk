//! Stable ids: the one rule kernels keep the standard `id` attribute by.
//!
//! Kept elements keep their ids; new or copied elements take
//! [`fresh`] ones, above every id the value holds; a
//! [`gather`](crate::editor::gather) keeps its first input's ids and gives
//! later inputs' colliding (or missing) ids fresh ones. So an id names one element for as long as it lives,
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

/// The stable id of element `element` of `domain` in `value`: its `id`,
/// or its number on a value without ids there (what a step written on
/// such a value names it by).
///
/// ```
/// use kinogaki_edit::ids;
/// use kinogaki_geometry::{Attributes, Domain, Geometry, Primitives};
///
/// let t = Primitives::of_polygons(3, vec![0, 3].into(), vec![0, 1, 2].into()).unwrap();
/// let bare = Geometry::new(vec![[0.0; 3]; 3].into(), t, Attributes::new()).unwrap();
/// assert_eq!(ids::id_of(&bare, Domain::Point, 2), 2);
/// ```
pub fn id_of(value: &Geometry, domain: Domain, element: u32) -> i64 {
    value
        .ids(domain)
        .map_or(i64::from(element), |ids| ids[element as usize])
}

/// The elements of `domain` in `value` with these ids, in their order;
/// those gone left out. On a value without ids there, an id is the
/// element's number ([`id_of`]).
///
/// ```
/// use kinogaki_edit::ids;
/// use kinogaki_geometry::{Attributes, Domain, Geometry, Primitives};
///
/// let t = Primitives::of_polygons(3, vec![0, 3].into(), vec![0, 1, 2].into()).unwrap();
/// let bare = Geometry::new(vec![[0.0; 3]; 3].into(), t, Attributes::new()).unwrap();
/// assert_eq!(ids::elements(&bare, Domain::Point, &[2, 7, 0]), [2, 0]);
/// let numbered = ids::numbered(&bare).unwrap();
/// assert_eq!(ids::elements(&numbered, Domain::Point, &[2, 7, 0]), [2, 0]);
/// ```
pub fn elements(value: &Geometry, domain: Domain, ids: &[i64]) -> Vec<u32> {
    let find = element_of(value, domain);
    ids.iter().filter_map(|&id| find(id)).collect()
}

/// A lookup of `domain`'s elements in `value` by stable id ([`id_of`]'s
/// inverse): the element with an id, `None` for one gone. What it searches
/// by is made once per ids column and shared by every value holding it
/// ([`Column::derived`]), so a step finding its few elements in a million
/// costs what it looks up.
pub fn element_of(value: &Geometry, domain: Domain) -> impl Fn(i64) -> Option<u32> + use<> {
    let count = value.count(domain).unwrap_or(0) as u64;
    let ids = value.ids(domain).cloned();
    let lookup = ids.as_ref().map(|ids| ids.derived(Lookup::of));
    move |id| match (&ids, &lookup) {
        (Some(ids), Some(lookup)) => lookup.find(ids, id),
        _ => u32::try_from(id).ok().filter(|&e| u64::from(e) < count),
    }
}

/// How an ids column is searched: by halving while its ids ascend (as
/// numbering, appending and removing leave them), else through a map.
enum Lookup {
    Ascending,
    Map(HashMap<i64, u32>),
}

impl Lookup {
    fn of(ids: &Column<i64>) -> Self {
        let mut last = None;
        let ascending = ids
            .iter()
            .all(|&id| last.replace(id).is_none_or(|l| l < id));
        match ascending {
            true => Lookup::Ascending,
            false => Lookup::Map(
                ids.iter()
                    .enumerate()
                    .map(|(i, &id)| (id, i as u32))
                    .collect(),
            ),
        }
    }

    /// The element of `ids` holding `id`.
    fn find(&self, ids: &Column<i64>, id: i64) -> Option<u32> {
        match self {
            Lookup::Map(at) => at.get(&id).copied(),
            Lookup::Ascending => {
                let (mut low, mut high) = (0, ids.len());
                while low < high {
                    let middle = low + (high - low) / 2;
                    match ids[middle].cmp(&id) {
                        std::cmp::Ordering::Less => low = middle + 1,
                        std::cmp::Ordering::Greater => high = middle,
                        std::cmp::Ordering::Equal => return Some(middle as u32),
                    }
                }
                None
            }
        }
    }
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
pub(crate) enum Input<'a> {
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
pub(crate) fn merged(inputs: &[Input<'_>]) -> Option<Vec<i64>> {
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

#[cfg(test)]
mod tests {
    use super::*;
    use kinogaki_geometry::{Attributes, Primitives};

    #[test]
    fn ids_are_found_whether_they_ascend_or_not() {
        let with = |ids: Vec<i64>| {
            let t = Primitives::of_polygons(5, vec![0, 3].into(), vec![0, 1, 2].into()).unwrap();
            let ids = Attribute::tuples(ids, Meaning::None).unwrap();
            let held = Attributes::new()
                .with(Domain::Point, names::ID, ids)
                .unwrap();
            Geometry::new(vec![[0.0; 3]; 5].into(), t, held).unwrap()
        };
        // Numbered, then a point gone: ascending with a gap.
        let ascending = with(vec![0, 1, 3, 4, 9]);
        assert_eq!(
            elements(&ascending, Domain::Point, &[9, 2, 0, 3, -1, 10]),
            [4, 0, 2]
        );
        let shuffled = with(vec![7, 3, 9, 0, 4]);
        assert_eq!(elements(&shuffled, Domain::Point, &[0, 9, 1, 7]), [3, 2, 0]);
        for (k, &id) in [7, 3, 9, 0, 4].iter().enumerate() {
            assert_eq!(element_of(&shuffled, Domain::Point)(id), Some(k as u32));
        }
    }

    #[test]
    fn merge_keeps_the_first_inputs_ids_and_renames_later_collisions() {
        let got = merged(&[
            Input::Ids(&[5, 6, 7]),
            Input::Without(2),
            Input::Ids(&[7, 1, 1]),
        ])
        .unwrap();
        assert_eq!(got, [5, 6, 7, 8, 9, 10, 1, 11]);
        // Unique, and the first input's untouched.
        let mut sorted = got.clone();
        sorted.sort_unstable();
        sorted.dedup();
        assert_eq!(sorted.len(), got.len());
        assert_eq!(merged(&[Input::Ids(&[i64::MAX]), Input::Without(1)]), None);
    }
}
