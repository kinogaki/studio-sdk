//! [`across`]: the axes across a curve at each of its points, carried
//! along it so they never flip or spin (rotation-minimizing frames): what a
//! sweep of a profile and a tube of a curve place their rings on.
use crate::vec3::Vec3;

/// The axes across a curve at one of its points: perpendicular to the
/// curve and to each other, `x.cross(y)` the way the curve runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Across {
    /// The first axis across.
    pub x: Vec3,
    /// The second, a quarter turn on from `x` about the curve.
    pub y: Vec3,
}

/// The axes across the polyline through `at` at each of its points: `up`
/// (its part across the curve) at the first, else an axis away from the
/// first direction, then carried point to point by the least rotation that
/// follows the curve's turn. Points on top of the one before share its
/// axes; a curve's direction at a point halves the turn between its spans.
pub fn across(at: &[Vec3], up: Option<Vec3>) -> Vec<Across> {
    let n = at.len();
    let tangents: Vec<Vec3> = (0..n)
        .map(|i| {
            let before = (at[i] - at[i.saturating_sub(1)]).normalized();
            let after = (at[(i + 1).min(n - 1)] - at[i]).normalized();
            match (before, after) {
                (Some(b), Some(a)) => (a + b).normalized().unwrap_or(a),
                (Some(t), None) | (None, Some(t)) => t,
                (None, None) => Vec3::Y,
            }
        })
        .collect();
    let Some(&first) = tangents.first() else {
        return Vec::new();
    };
    let seed = up
        .and_then(|up| (up - first * up.dot(first)).normalized())
        .or_else(|| {
            let other = if first.y.abs() < 0.9 {
                Vec3::Y
            } else {
                Vec3::X
            };
            (other - first * other.dot(first)).normalized()
        })
        .unwrap_or(Vec3::X);
    let mut x = seed;
    (0..n)
        .map(|i| {
            if i > 0 {
                x = transported(x, tangents[i - 1], tangents[i]);
            }
            Across {
                x,
                y: tangents[i].cross(x),
            }
        })
        .collect()
}

/// `x`, across `from`, turned as `from` turns to `to` by the least rotation.
fn transported(x: Vec3, from: Vec3, to: Vec3) -> Vec3 {
    let axis = from.cross(to);
    let c = from.dot(to);
    if axis.length() < 1e-12 {
        return x;
    }
    // Rodrigues over the unnormalized axis: (1 / (1 + c)) folds the half-angle.
    let k = 1.0 / (1.0 + c).max(1e-12);
    let turned = x * c + axis.cross(x) + axis * (axis.dot(x) * k);
    (turned - to * turned.dot(to)).normalized().unwrap_or(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn axes_carried_round_a_bend_stay_across_and_never_spin() {
        let turned: Vec<Vec3> = (0..200)
            .map(|i| {
                let a = f64::from(i) * 0.05;
                Vec3::new(10.0 * a.cos(), f64::from(i) * 0.1, 10.0 * a.sin())
            })
            .collect();
        let done = across(&turned, None);
        for (i, f) in done.iter().enumerate().skip(1) {
            let step = (f.x - done[i - 1].x).length();
            assert!(step < 0.2, "a small step at {i}: {step}");
            let along = turned[(i + 1).min(199)] - turned[i - 1];
            assert!(
                f.x.dot(along).abs() < 1e-6 * along.length(),
                "across at {i}"
            );
            assert!((f.x.dot(f.y)).abs() < 1e-9 && (f.y.length() - 1.0).abs() < 1e-9);
        }
    }

    #[test]
    fn up_seeds_the_first_axes_and_a_straight_line_keeps_them() {
        let line: Vec<Vec3> = (0..4).map(|i| Vec3::new(f64::from(i), 0.0, 0.0)).collect();
        let done = across(&line, Some(Vec3::new(0.3, 0.0, 1.0)));
        for f in &done {
            assert!((f.x - Vec3::Z).length() < 1e-12, "{f:?}");
        }
        assert!(across(&[], None).is_empty());
        let still = across(&[Vec3::ZERO, Vec3::ZERO], None);
        assert_eq!(
            still[0], still[1],
            "a point on top of the one before shares its axes"
        );
    }
}
