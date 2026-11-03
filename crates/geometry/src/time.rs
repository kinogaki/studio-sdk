//! [`Time`]: a moment on the one tick grid, and [`Rate`]: frames a second.

/// A moment of the scene: a whole number of ticks from its start, a tick
/// being 1/705,600,000 s (flicks). Cooks,
/// keys, playback and render prims that change over time read it.
///
/// A frame at 24, 25, 30, 48, 50, 60, 90, 100 or 120 frames a second is a
/// whole number of ticks, so a key and a frame meet exactly however their
/// times were computed (`n / fps`, a sum of frame steps). A moment holds
/// no frame rate: the document's [`Rate`] says how many frames a second it
/// shows, so a moment stays where it is when the rate changes.
///
/// Moments lie within ±2⁵⁰ ticks of the start (about 18 days, [`Time::MIN`]
/// to [`Time::MAX`]), where a moment's seconds as an `f64` read back as the
/// same tick: what a file saves.
///
/// ```
/// use kinogaki_geometry::{Rate, Time};
///
/// let fps = Rate::new(24.0).unwrap();
/// let t = Time::from_frame(48.0, fps).unwrap();
/// assert_eq!(t, Time::from_seconds(2.0).unwrap());
/// assert_eq!(t.frame(Rate::new(30.0).unwrap()), 60.0);
/// // Ten steps of a tenth of a second land on the second exactly.
/// let step = Time::from_seconds(0.1).unwrap();
/// assert_eq!((0..10).fold(Time::ZERO, |t, _| t + step), Time::from_seconds(1.0).unwrap());
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Time(i64);

impl Time {
    /// Ticks a second.
    pub(crate) const TICKS_PER_SECOND: i64 = 705_600_000;

    /// The scene's start.
    pub const ZERO: Time = Time(0);

    /// The latest moment.
    pub const MAX: Time = Time(1 << 50);

    /// The earliest moment.
    pub const MIN: Time = Time(-(1 << 50));

    /// The moment `ticks` ticks from the start, kept within [`Time::MIN`]
    /// and [`Time::MAX`].
    pub(crate) const fn from_ticks(ticks: i64) -> Self {
        Self(if ticks > Self::MAX.0 {
            Self::MAX.0
        } else if ticks < Self::MIN.0 {
            Self::MIN.0
        } else {
            ticks
        })
    }

    /// The moment nearest `seconds` from the start on the tick grid;
    /// `None` when it isn't finite or lies past [`Time::MIN`] or
    /// [`Time::MAX`].
    pub fn from_seconds(seconds: f64) -> Option<Self> {
        Self::rounded(seconds * Self::TICKS_PER_SECOND as f64)
    }

    /// Frame `frame` at `rate` (fractional between frames: motion blur,
    /// sub-frame samples), on the tick grid; `None` when it isn't finite
    /// or lies out of range.
    pub fn from_frame(frame: f64, rate: Rate) -> Option<Self> {
        Self::rounded(frame * Self::TICKS_PER_SECOND as f64 / rate.0)
    }

    /// The moment of the tick nearest `ticks`, when it is one.
    fn rounded(ticks: f64) -> Option<Self> {
        let ticks = ticks.round();
        let within = ticks.abs() <= Self::MAX.0 as f64;
        (ticks.is_finite() && within).then_some(Self(ticks as i64))
    }

    /// Its ticks from the start.
    pub const fn ticks(self) -> i64 {
        self.0
    }

    /// Its seconds from the start.
    pub fn seconds(self) -> f64 {
        self.0 as f64 / Self::TICKS_PER_SECOND as f64
    }

    /// Its frame at `rate`, fractional between frames (exact for a whole
    /// frame at the common rates).
    pub fn frame(self, rate: Rate) -> f64 {
        self.0 as f64 * rate.0 / Self::TICKS_PER_SECOND as f64
    }
}

impl std::ops::Add for Time {
    type Output = Time;
    /// The later moment, kept within the range.
    fn add(self, other: Time) -> Time {
        Time::from_ticks(self.0.saturating_add(other.0))
    }
}

impl std::ops::Sub for Time {
    type Output = Time;
    /// The difference, kept within the range.
    fn sub(self, other: Time) -> Time {
        Time::from_ticks(self.0.saturating_sub(other.0))
    }
}

/// Frames a second: finite and above zero.
#[derive(Clone, Copy, Debug, PartialEq, PartialOrd)]
pub struct Rate(f64);

impl Default for Rate {
    /// 24 frames a second.
    fn default() -> Self {
        Self::FILM
    }
}

impl Rate {
    /// 24 frames a second.
    pub const FILM: Rate = Rate(24.0);

    /// `fps` frames a second; `None` when it isn't finite and above zero.
    pub fn new(fps: f64) -> Option<Self> {
        (fps.is_finite() && fps > 0.0).then_some(Self(fps))
    }

    /// Frames a second.
    pub fn fps(self) -> f64 {
        self.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn moments_sit_on_the_tick_grid() {
        let fps = Rate::new(24.0).unwrap();
        let t = Time::from_seconds(2.0).unwrap();
        assert_eq!(t.ticks(), 2 * Time::TICKS_PER_SECOND);
        assert_eq!(t.frame(fps), 48.0);
        assert_eq!(Time::from_frame(48.0, fps), Some(t));
        assert_eq!(Time::from_seconds(-0.0), Some(Time::ZERO));
        assert!(Time::ZERO < t);
        // A frame at the common rates is a whole number of ticks.
        for fps in [24.0, 25.0, 30.0, 48.0, 50.0, 60.0, 90.0, 100.0, 120.0] {
            let rate = Rate::new(fps).unwrap();
            let frame = Time::from_frame(1.0, rate).unwrap();
            assert_eq!(frame.ticks() as f64 * fps, Time::TICKS_PER_SECOND as f64);
            assert_eq!(Time::from_frame(7.0, rate).unwrap().frame(rate), 7.0);
        }
        // Seconds read back are the same moment, out to the range's ends.
        for t in [123_456_789_012, (1 << 50) - 1, -(1 << 50) + 7] {
            let odd = Time::from_ticks(t);
            assert_eq!(Time::from_seconds(odd.seconds()), Some(odd));
        }
        assert_eq!(Time::from_seconds(Time::MAX.seconds()), Some(Time::MAX));
    }

    #[test]
    fn only_finite_moments_and_rates_are_made() {
        assert_eq!(Time::from_seconds(f64::NAN), None);
        assert_eq!(Time::from_seconds(f64::INFINITY), None);
        assert_eq!(Time::from_seconds(1e300), None);
        assert_eq!(Rate::new(0.0), None);
        assert_eq!(Rate::new(-24.0), None);
        assert_eq!(Rate::new(f64::NAN), None);
        assert_eq!(Rate::default().fps(), 24.0);
        assert_eq!(Time::MAX + Time::from_ticks(1), Time::MAX);
        assert_eq!(Time::from_ticks(i64::MIN), Time::MIN);
        assert_eq!(Time::from_seconds(Time::MAX.seconds() * 2.0), None);
    }
}
