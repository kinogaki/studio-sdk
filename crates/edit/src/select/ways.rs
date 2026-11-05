//! The one shortest-way search over elements ([`Ways`], Dijkstra's): the
//! Shortest Path command's way between two elements and a soft selection's
//! reach along the surface both run it.
use super::at;
use kinogaki_geometry::Geometry;
use std::cmp::Ordering;
use std::collections::{BinaryHeap, HashMap};

/// The shortest ways from a set of starts over a graph of elements: how far
/// each element reached lies and the element it was reached from.
#[derive(Debug)]
pub struct Ways {
    best: HashMap<u32, (f64, Option<u32>)>,
    end: Option<u32>,
}

impl Ways {
    /// Searches outward from every element of `from` (each at 0) over the
    /// graph `around` gives (it calls `next(element, distance)` for each
    /// neighbour of an element), reaching no element `limit` or more away,
    /// and stops at the first element `stop` accepts, the nearest such.
    pub fn search(
        from: &[u32],
        limit: f64,
        stop: impl Fn(u32) -> bool,
        around: impl Fn(u32, &mut dyn FnMut(u32, f64)),
    ) -> Self {
        let mut best: HashMap<u32, (f64, Option<u32>)> = HashMap::new();
        let mut queue = BinaryHeap::new();
        for &start in from {
            best.insert(start, (0.0, None));
            queue.push(Reached(0.0, start));
        }
        while let Some(Reached(d, place)) = queue.pop() {
            if best.get(&place).is_some_and(|(known, _)| d > *known) {
                continue;
            }
            if stop(place) {
                return Self {
                    best,
                    end: Some(place),
                };
            }
            around(place, &mut |next, far| {
                let further = d + far;
                if further < limit && best.get(&next).is_none_or(|(known, _)| further < *known) {
                    best.insert(next, (further, Some(place)));
                    queue.push(Reached(further, next));
                }
            });
        }
        Self { best, end: None }
    }

    /// The element the search stopped at, when `stop` accepted one.
    pub fn end(&self) -> Option<u32> {
        self.end
    }

    /// The way to `to`, from `to` back to the start it was reached from;
    /// empty when it wasn't reached.
    pub fn path(&self, to: u32) -> Vec<u32> {
        let mut path = Vec::new();
        let mut at = self.best.contains_key(&to).then_some(to);
        while let Some(place) = at {
            path.push(place);
            at = self.best.get(&place).and_then(|(_, back)| *back);
        }
        path
    }

    /// Every element reached and how far it lies, in no order.
    pub fn distances(&self) -> impl Iterator<Item = (u32, f64)> + '_ {
        self.best.iter().map(|(&place, &(d, _))| (place, d))
    }
}

/// `value`'s points as a graph for [`Ways::search`]: each point's
/// neighbours along an edge, as far as the edge is long.
pub fn along_edges(value: &Geometry) -> impl Fn(u32, &mut dyn FnMut(u32, f64)) + '_ {
    let index = value.primitives().index();
    move |p, next| {
        for q in index.point_neighbours(p as usize) {
            next(q, (at(value, q) - at(value, p)).length());
        }
    }
}

/// An element at a distance, nearest first in a max-heap.
struct Reached(f64, u32);

impl PartialEq for Reached {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other).is_eq()
    }
}
impl Eq for Reached {}
impl PartialOrd for Reached {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}
impl Ord for Reached {
    fn cmp(&self, other: &Self) -> Ordering {
        other.0.total_cmp(&self.0).then(self.1.cmp(&other.1))
    }
}
