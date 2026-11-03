//! [`Sampled`]: a value over a frame's shutter.
use crate::error::{ErrorKind, GeometryError};
use std::sync::Arc;

/// A value at one or more moments around a frame: `values[k]` at
/// `times[k]`, in frames from the frame (fractional: a shutter's samples),
/// ascending. One sample means it holds still: what most values are. A
/// render prim's placements across the shutter are one ([`Sampled::still`]
/// for a prim that doesn't move).
///
/// The samples are shared (`Arc`), so sending a sampled value again costs
/// a reference count, never a copy.
///
/// ```
/// use kinogaki_geometry::Sampled;
///
/// let still = Sampled::still(1.0);
/// assert!(still.is_still() && still.values() == [1.0]);
/// let moving = Sampled::new(vec![-0.25, 0.25], vec![1.0, 3.0]).unwrap();
/// assert_eq!(moving.times(), [-0.25, 0.25]);
/// assert!(Sampled::new(vec![0.5, 0.0], vec![1.0, 2.0]).is_err());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Sampled<T> {
    times: Arc<[f32]>,
    values: Arc<[T]>,
}

impl<T> Sampled<T> {
    /// `value` holding still: one sample, at the frame.
    pub fn still(value: T) -> Self {
        Self {
            times: Arc::from([0.0]),
            values: Arc::from([value]),
        }
    }

    /// `values` at `times` (frames from the frame).
    ///
    /// # Errors
    /// No samples, a time for each value missing, or times not finite and
    /// ascending.
    pub fn new(
        times: impl Into<Arc<[f32]>>,
        values: impl Into<Arc<[T]>>,
    ) -> Result<Self, GeometryError> {
        let (times, values) = (times.into(), values.into());
        let ascending = times.windows(2).all(|t| t[0] < t[1]);
        let finite = times.iter().all(|t| t.is_finite());
        if times.is_empty() || times.len() != values.len() || !(ascending && finite) {
            return Err(GeometryError::whole(
                ErrorKind::Range,
                "samples need one finite, ascending time a value, and at least one value",
            ));
        }
        Ok(Self { times, values })
    }

    /// The sample times, in frames from the frame, ascending.
    pub fn times(&self) -> &[f32] {
        &self.times
    }

    /// The values, one a time.
    pub fn values(&self) -> &[T] {
        &self.values
    }

    /// Whether it holds one sample: it doesn't change around the frame.
    pub fn is_still(&self) -> bool {
        self.values.len() == 1
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn samples_are_checked() {
        let s = Sampled::new(vec![-0.5, 0.0, 0.5], vec!['a', 'b', 'c']).unwrap();
        assert_eq!(s.times(), [-0.5, 0.0, 0.5]);
        assert_eq!(s.values(), ['a', 'b', 'c']);
        assert!(!s.is_still());
        assert!(Sampled::<u8>::new(vec![], vec![]).is_err());
        assert!(Sampled::new(vec![0.0], vec![1, 2]).is_err());
        assert!(Sampled::new(vec![0.0, 0.0], vec![1, 2]).is_err());
        assert!(Sampled::new(vec![f32::NAN], vec![1]).is_err());
    }
}
