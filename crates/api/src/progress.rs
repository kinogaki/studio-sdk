//! How far a cook has come: [`Progress`], which a long node reports and the
//! host shows on its tile.
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

/// Steps a fraction is held in: finer than any bar draws.
const STEPS: f64 = 1_000_000.0;

/// How far a cook has come, a fraction from 0 to 1: a node reports it
/// ([`report`](Self::report)) from any thread, parallel code included. Clones
/// share what is reported. Reporting only stores it: the host reads it when it
/// draws, so a report never asks for a frame, and it is cheap enough for a tight
/// loop. It only goes forward: a report below the furthest one is ignored, so a
/// node can report from parallel workers without ordering them. A value outside
/// 0 to 1 is taken as the nearer end and NaN is ignored.
///
/// A cook that never reports shows that it is cooking, without saying how far.
///
/// ```
/// use kinogaki_studio_api::Progress;
/// let progress = Progress::shown();
/// assert_eq!(progress.fraction(), None);
/// progress.report(0.25);
/// progress.report(0.1);
/// assert_eq!(progress.fraction(), Some(0.25));
/// ```
#[derive(Clone, Debug, Default)]
pub struct Progress(Option<Arc<AtomicU32>>);

impl Progress {
    /// Progress someone reads: how a host makes one for a cook.
    /// [`Progress::default`] reports to no one.
    pub fn shown() -> Self {
        Self(Some(Arc::default()))
    }

    /// `fraction` of the cook done. Ignored unless it is further than what was
    /// reported.
    pub fn report(&self, fraction: f64) {
        let Some(cell) = &self.0 else {
            return;
        };
        if fraction.is_nan() {
            return;
        }
        // 0 is "nothing reported", so a report of 0 still shows.
        let held = 1 + (fraction.clamp(0.0, 1.0) * STEPS).round() as u32;
        if held > cell.load(Ordering::Relaxed) {
            cell.fetch_max(held, Ordering::Relaxed);
        }
    }

    /// The furthest fraction reported; `None` until a report. A host reads it to
    /// show it.
    pub fn fraction(&self) -> Option<f64> {
        let held = self.0.as_ref()?.load(Ordering::Relaxed);
        (held > 0).then(|| f64::from(held - 1) / STEPS)
    }

    /// Nothing reported again: for a host handing the same progress to its next
    /// cook.
    pub fn clear(&self) {
        if let Some(cell) = &self.0 {
            cell.store(0, Ordering::Relaxed);
        }
    }
}

/// What a cook sees where the host shows nothing.
pub(crate) static UNSHOWN: Progress = Progress(None);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_only_goes_forward_from_any_thread() {
        let progress = Progress::shown();
        std::thread::scope(|s| {
            for t in 0..8 {
                let progress = progress.clone();
                s.spawn(move || {
                    for i in 0..1000 {
                        progress.report(f64::from(i + t) / 1006.0);
                    }
                });
            }
        });
        assert_eq!(progress.fraction(), Some(1.0));
        progress.report(0.5);
        assert_eq!(progress.fraction(), Some(1.0));
    }

    #[test]
    fn a_fraction_outside_the_range_is_the_nearer_end_and_nan_nothing() {
        let progress = Progress::shown();
        progress.report(f64::NAN);
        assert_eq!(progress.fraction(), None);
        progress.report(-3.0);
        assert_eq!(progress.fraction(), Some(0.0));
        progress.report(f64::INFINITY);
        assert_eq!(progress.fraction(), Some(1.0));
        progress.report(f64::NAN);
        assert_eq!(progress.fraction(), Some(1.0));
    }

    #[test]
    fn unshown_progress_reports_to_no_one() {
        let progress = Progress::default();
        progress.report(0.5);
        assert_eq!(progress.fraction(), None);
    }

    #[test]
    fn cleared_progress_says_nothing_again() {
        let progress = Progress::shown();
        progress.report(0.5);
        progress.clear();
        assert_eq!(progress.fraction(), None);
    }
}
