//! The scene time work runs at.
use std::cmp::Ordering;
use std::hash::{Hash, Hasher};

/// A moment of the scene: seconds from its start, a finite number. Cooks,
/// conversions, keys and render prims that change over time read it.
///
/// A moment holds no frame rate: the document's frame range says how many
/// frames a second it shows, so a moment stays where it is when the rate
/// changes. [`frame`](Self::frame) gives the (fractional) frame at a rate.
///
/// Moments compare and order by their seconds. Only finite seconds make
/// one ([`seconds`](Self::seconds) and [`from_frame`](Self::from_frame)
/// refuse the rest with `None`), so the order is total; `-0` is `0`.
#[derive(Clone, Copy, Debug)]
pub struct Time(f64);

impl Time {
    /// The scene's start.
    pub const ZERO: Self = Self(0.0);

    /// `seconds` from the scene's start; `None` when it isn't finite.
    pub const fn seconds(seconds: f64) -> Option<Self> {
        // Adding zero turns -0 into 0, so equal moments hash alike.
        if seconds.is_finite() {
            Some(Self(seconds + 0.0))
        } else {
            None
        }
    }

    /// Frame `frame` at `fps` frames a second (fractional between frames:
    /// motion blur, sub-frame samples); `None` when either isn't finite,
    /// `fps` isn't above zero or the moment overflows.
    pub const fn from_frame(frame: f64, fps: f64) -> Option<Self> {
        if !(fps.is_finite() && fps > 0.0) {
            return None;
        }
        Self::seconds(frame / fps)
    }

    /// Its seconds from the scene's start.
    pub const fn as_seconds(self) -> f64 {
        self.0
    }

    /// Its frame at `fps` frames a second, fractional between frames.
    pub const fn frame(self, fps: f64) -> f64 {
        self.0 * fps
    }
}

impl PartialEq for Time {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Time {}

impl PartialOrd for Time {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Time {
    /// By seconds: finite and without `-0`, so it agrees with `==`.
    fn cmp(&self, other: &Self) -> Ordering {
        self.0.total_cmp(&other.0)
    }
}

impl Hash for Time {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.to_bits().hash(state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_moment_is_its_seconds() {
        let t = Time::seconds(2.0).unwrap();
        assert_eq!(t.frame(24.0), 48.0);
        assert_eq!(t.frame(30.0), 60.0);
        assert_eq!(Time::from_frame(48.0, 24.0), Some(t));
        assert_eq!(Time::from_frame(60.0, 30.0), Some(t));
        assert_eq!(Time::seconds(-0.0), Some(Time::ZERO));
        assert!(Time::ZERO < t);
    }

    #[test]
    fn only_finite_moments_are_made() {
        assert_eq!(Time::seconds(f64::NAN), None);
        assert_eq!(Time::seconds(f64::INFINITY), None);
        assert_eq!(Time::from_frame(1.0, 0.0), None);
        assert_eq!(Time::from_frame(1.0, -24.0), None);
        assert_eq!(Time::from_frame(f64::MAX, f64::MIN_POSITIVE), None);
    }
}
