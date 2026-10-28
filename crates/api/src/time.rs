//! The scene time work runs at.

/// A moment in the scene: its frame, the same moment in seconds, and the
/// frames a second. Cooks, conversions and render prims that change over
/// time read it; the frame is fractional between frames (motion blur,
/// sub-frame samples).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Time {
    /// The frame, `seconds * fps`.
    pub frame: f64,
    /// Seconds from the scene's start.
    pub seconds: f64,
    /// Frames a second.
    pub fps: f64,
}

impl Time {
    /// `seconds` into the scene at `fps` frames a second.
    pub fn at(seconds: f64, fps: f64) -> Self {
        Self {
            frame: seconds * fps,
            seconds,
            fps,
        }
    }

    /// Frame `frame` at `fps` frames a second.
    pub fn at_frame(frame: f64, fps: f64) -> Self {
        Self {
            frame,
            seconds: frame / fps,
            fps,
        }
    }
}

impl Default for Time {
    /// The scene's start at 24 frames a second.
    fn default() -> Self {
        Self::at(0.0, 24.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn frames_and_seconds_agree() {
        let t = Time::at(2.0, 24.0);
        assert_eq!(t.frame, 48.0);
        assert_eq!(Time::at_frame(48.0, 24.0), t);
        assert_eq!(Time::default(), Time::at(0.0, 24.0));
    }
}
