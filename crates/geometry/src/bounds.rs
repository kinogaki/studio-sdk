//! [`Bounds`]: the one axis-aligned box.
use crate::vec3::Vec3;

/// An axis-aligned box, `min` to `max` in each axis: what a value's extent,
/// framing, culling and the info panel read.
///
/// ```
/// use kinogaki_geometry::{Bounds, Vec3};
///
/// let b = Bounds::of([Vec3::new(0.0, 2.0, 1.0), Vec3::new(1.0, 0.0, 3.0)]).unwrap();
/// assert_eq!(b.center(), Vec3::new(0.5, 1.0, 2.0));
/// assert_eq!(Bounds::of([]), None);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    /// The smallest corner.
    pub min: Vec3,
    /// The largest corner.
    pub max: Vec3,
}

impl Bounds {
    /// The box holding only `p`.
    pub const fn point(p: Vec3) -> Self {
        Self { min: p, max: p }
    }

    /// The smallest box holding `points`; `None` when there are none.
    pub fn of(points: impl IntoIterator<Item = Vec3>) -> Option<Self> {
        let mut points = points.into_iter();
        let first = Self::point(points.next()?);
        Some(points.fold(first, |b, p| b.union(Self::point(p))))
    }

    /// The smallest box holding both `self` and `other`.
    pub fn union(self, other: Bounds) -> Bounds {
        Bounds {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// The box's center.
    pub fn center(self) -> Vec3 {
        (self.min + self.max) * 0.5
    }

    /// Its eight corners.
    pub fn corners(self) -> [Vec3; 8] {
        let (a, b) = (self.min, self.max);
        std::array::from_fn(|i| {
            Vec3::new(
                if i & 1 == 0 { a.x } else { b.x },
                if i & 2 == 0 { a.y } else { b.y },
                if i & 4 == 0 { a.z } else { b.z },
            )
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn boxes_join_center_and_have_corners() {
        let a = Bounds {
            min: Vec3::ZERO,
            max: Vec3::new(1.0, 1.0, 1.0),
        };
        let b = Bounds {
            min: Vec3::new(-1.0, 0.5, 0.0),
            max: Vec3::new(0.0, 3.0, 1.0),
        };
        let both = a.union(b);
        assert_eq!(both.min, Vec3::new(-1.0, 0.0, 0.0));
        assert_eq!(both.max, Vec3::new(1.0, 3.0, 1.0));
        assert_eq!(both.center(), Vec3::new(0.0, 1.5, 0.5));
        let corners = a.corners();
        assert_eq!((corners[0], corners[7]), (a.min, a.max));
        assert_eq!(Bounds::of(corners), Some(a));
    }
}
