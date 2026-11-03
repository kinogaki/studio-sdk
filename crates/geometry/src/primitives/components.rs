//! [`Components`]: the table's points by connected piece.
use super::Primitives;

/// Points joined through primitives: two points of one primitive are in
/// one component, and a point no primitive names is a component of its
/// own. Components are numbered from 0 by their smallest point.
///
/// ```
/// use kinogaki_geometry::Primitives;
///
/// // Two separate triangles, and point 6 alone.
/// let t = Primitives::of_polygons(7, vec![0, 3, 6].into(), vec![0, 1, 2, 3, 4, 5].into()).unwrap();
/// let pieces = t.components();
/// assert_eq!(pieces.count(), 3);
/// assert_eq!(pieces.of_points(), [0, 0, 0, 1, 1, 1, 2]);
/// // Anything else that keeps points together joins over a copy.
/// assert_eq!(pieces.joined([(2, 6)]).of_points(), [0, 0, 0, 1, 1, 1, 0]);
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Components {
    of_points: Vec<u32>,
    count: usize,
}

impl Components {
    /// How many components.
    pub fn count(&self) -> usize {
        self.count
    }

    /// Each point's component.
    pub fn of_points(&self) -> &[u32] {
        &self.of_points
    }

    /// These components with each pair of points in one: what a caller
    /// keeps together beside the table (a `name` of several pieces).
    pub fn joined(&self, pairs: impl IntoIterator<Item = (u32, u32)>) -> Components {
        let mut sets = Sets::new(self.count);
        for (a, b) in pairs {
            let (a, b) = (self.of_points[a as usize], self.of_points[b as usize]);
            sets.join(a, b);
        }
        let of_sets = sets.numbered();
        let count = of_sets
            .iter()
            .copied()
            .max()
            .map_or(0, |last| last as usize + 1);
        Components {
            of_points: (self.of_points.iter())
                .map(|&c| of_sets[c as usize])
                .collect(),
            count,
        }
    }
}

pub(super) fn of(t: &Primitives) -> Components {
    let mut sets = Sets::new(t.points());
    for p in 0..t.len() {
        let corners = t.primitive(p);
        for &q in corners.iter().skip(1) {
            sets.join(corners[0], q);
        }
    }
    let of_points = sets.numbered();
    let count = of_points
        .iter()
        .copied()
        .max()
        .map_or(0, |last| last as usize + 1);
    Components { of_points, count }
}

/// Disjoint sets of the ids below a count, each rooted at its smallest id.
struct Sets(Vec<u32>);

impl Sets {
    fn new(count: usize) -> Self {
        Sets((0..count as u32).collect())
    }

    /// The root of `i`'s set, halving the path on the way.
    fn root(&mut self, mut i: u32) -> u32 {
        while self.0[i as usize] != i {
            let up = self.0[self.0[i as usize] as usize];
            self.0[i as usize] = up;
            i = up;
        }
        i
    }

    /// `a`'s and `b`'s sets as one, rooted at the smaller root.
    fn join(&mut self, a: u32, b: u32) {
        let (a, b) = (self.root(a), self.root(b));
        self.0[a.max(b) as usize] = a.min(b);
    }

    /// Each id's set, numbered from 0 by the sets' smallest ids. A root is
    /// below everything in its set, so one ascending pass numbers it first.
    fn numbered(mut self) -> Vec<u32> {
        let mut next = 0;
        for i in 0..self.0.len() as u32 {
            let parent = self.0[i as usize];
            self.0[i as usize] = if parent == i {
                next += 1;
                next - 1
            } else {
                // The parent, smaller, already holds its set's number.
                self.0[parent as usize]
            };
        }
        self.0
    }
}
