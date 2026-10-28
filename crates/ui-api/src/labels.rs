//! Point numbers over a view (Houdini's point number display): where to
//! put them so a dense mesh shows an evenly spread, readable subset at a
//! bounded cost.
//!
//! Only points in view are labelled, at most one per screen cell. Points
//! are visited in a scattered order, and the walk stops once a long run of
//! them lands in cells already taken (or every cell is taken).
use crate::projection::ClipMatrix;
use kinogaki_studio_api::Vec3;

/// A label: where its point shows (pixels from the view's top left) and
/// the point's number.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Label {
    /// Where the point shows, in pixels from the view's top left.
    pub at: [f64; 2],
    /// The point's number.
    pub id: u32,
}

/// Points a walk visits after its last new label before it stops.
const PATIENCE: usize = 4096;

/// Labels for `positions` (object space) in a view of `size` pixels, one
/// per `cell` pixels (a label's footprint). `clip` is the view's clip
/// space from the object's (a [`Projection`](crate::projection::Projection)'s
/// `clip_from_object`, built for this view's aspect). `shown` says whether
/// a point may be labelled (hidden ones are not).
pub fn point_labels(
    positions: &[Vec3],
    shown: impl Fn(u32) -> bool,
    clip: &ClipMatrix,
    size: [f64; 2],
    cell: [f64; 2],
) -> Vec<Label> {
    let n = positions.len();
    if n == 0 || size[0] < 1.0 || size[1] < 1.0 || cell[0] < 1.0 || cell[1] < 1.0 {
        return Vec::new();
    }
    let columns = (size[0] / cell[0]).ceil() as usize;
    let rows = (size[1] / cell[1]).ceil() as usize;
    let mut taken = vec![false; columns * rows];
    let mut free = taken.len();
    let mut labels = Vec::new();
    let mut misses = 0;
    let step = scatter_step(n);
    let mut at = 0usize;
    for _ in 0..n {
        let id = at;
        at = (at + step) % n;
        if !shown(id as u32) {
            continue;
        }
        let p = positions[id];
        let [x, y, z, w] = clip.mul_vec4([p.x, p.y, p.z, 1.0]);
        let placed = (w > 0.0)
            .then(|| (x / w, y / w, z / w))
            .filter(|&(_, _, z)| (0.0..=1.0).contains(&z))
            .map(|(x, y, _)| [(x + 1.0) * 0.5 * size[0], (1.0 - y) * 0.5 * size[1]])
            .filter(|p| p[0] >= 0.0 && p[1] >= 0.0 && p[0] < size[0] && p[1] < size[1]);
        let Some(p) = placed else {
            continue;
        };
        let slot = (p[1] / cell[1]) as usize * columns + (p[0] / cell[0]) as usize;
        if taken[slot] {
            misses += 1;
            if misses > PATIENCE {
                break;
            }
            continue;
        }
        taken[slot] = true;
        free -= 1;
        misses = 0;
        labels.push(Label {
            at: p,
            id: id as u32,
        });
        if free == 0 {
            break;
        }
    }
    labels
}

/// A stride coprime with `n` near the golden ratio of it, so `k * step % n`
/// visits every index once in a well-spread order.
fn scatter_step(n: usize) -> usize {
    if n < 3 {
        return 1;
    }
    let mut step = ((n as f64) * 0.618_033_988_75) as usize | 1;
    while gcd(step, n) != 1 {
        step += 2;
    }
    step % n
}

fn gcd(mut a: usize, mut b: usize) -> usize {
    while b != 0 {
        (a, b) = (b, a % b);
    }
    a
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::projection::Projection;
    use crate::projection::test_camera::projection;

    fn camera(distance: f64, size: [f64; 2]) -> Projection {
        projection(Vec3::ZERO, distance, false, size[0] / size[1])
    }

    #[test]
    fn scatter_visits_every_index_once() {
        for n in [1, 2, 3, 10, 64, 1000, 4096, 99_991] {
            let step = scatter_step(n);
            let mut seen = vec![false; n];
            let mut at = 0;
            for _ in 0..n {
                assert!(!seen[at], "{n}: {at} twice");
                seen[at] = true;
                at = (at + step) % n;
            }
        }
    }

    #[test]
    fn few_points_are_all_labelled_and_hidden_or_offscreen_ones_never() {
        let points = [
            Vec3::new(-0.5, 0.0, 0.0),
            Vec3::new(0.5, 0.0, 0.0),
            Vec3::new(0.0, 0.5, 0.0),
            Vec3::new(500.0, 0.0, 0.0),
        ];
        let size = [800.0, 600.0];
        let view = camera(4.0, size);
        let labels = point_labels(
            &points,
            |id| id != 2,
            &view.clip_from_object,
            size,
            [30.0, 14.0],
        );
        let mut ids: Vec<_> = labels.iter().map(|l| l.id).collect();
        ids.sort();
        assert_eq!(ids, [0, 1]);
        for label in labels {
            let want = view.project(points[label.id as usize], size).unwrap();
            assert_eq!(label.at, want);
        }
    }

    /// A million points: one label per cell at most, and the walk stops
    /// long before visiting them all.
    #[test]
    fn a_million_points_give_one_label_per_cell_quickly() {
        let side = 1000;
        let points: Vec<_> = (0..side * side)
            .map(|i| {
                Vec3::new(
                    (i % side) as f64 / side as f64 - 0.5,
                    (i / side) as f64 / side as f64 - 0.5,
                    0.0,
                )
            })
            .collect();
        let size = [1400.0, 900.0];
        let cell = [36.0, 16.0];
        let labels = point_labels(
            &points,
            |_| true,
            &camera(4.0, size).clip_from_object,
            size,
            cell,
        );
        let mut cells = std::collections::BTreeSet::new();
        for l in &labels {
            assert!(cells.insert(((l.at[0] / cell[0]) as u32, (l.at[1] / cell[1]) as u32)));
        }
        assert!(labels.len() > 100, "{}", labels.len());
        // Zoomed in: most points are off screen and every one is projected
        // (the bound on the walk's cost).
        let close = camera(0.05, size);
        let labels = point_labels(&points, |_| true, &close.clip_from_object, size, cell);
        assert!(!labels.is_empty());
    }

    #[test]
    fn empty_views_and_cells_label_nothing() {
        let view = camera(4.0, [10.0, 10.0]);
        let points = [Vec3::ZERO];
        assert!(
            point_labels(
                &points,
                |_| true,
                &view.clip_from_object,
                [0.0, 10.0],
                [1.0, 1.0]
            )
            .is_empty()
        );
        assert!(
            point_labels(
                &points,
                |_| true,
                &view.clip_from_object,
                [10.0, 10.0],
                [0.5, 1.0]
            )
            .is_empty()
        );
        assert!(
            point_labels(
                &[],
                |_| true,
                &view.clip_from_object,
                [10.0, 10.0],
                [1.0, 1.0]
            )
            .is_empty()
        );
    }
}
