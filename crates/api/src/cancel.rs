//! Cooperative cancellation: [`CancelToken`], which the host raises and
//! long work polls.
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A shared flag long work polls to stop early. Clones share the flag: the
/// host keeps one clone and raises it, the package's work checks another.
///
/// ```
/// use kinogaki_studio_api::{CancelToken, Error};
/// let token = CancelToken::default();
/// let work = token.clone();
/// assert_eq!(work.check(), Ok(()));
/// token.cancel();
/// assert_eq!(work.check(), Err(Error::Cancelled));
/// ```
#[derive(Clone, Debug, Default)]
pub struct CancelToken(Arc<AtomicBool>);

impl CancelToken {
    /// A token raised by whoever raises `flag`: a host's own cancellation
    /// lent to a package's work without copying it.
    pub fn sharing(flag: Arc<AtomicBool>) -> Self {
        Self(flag)
    }

    /// Raises the flag: every clone now reports cancelled.
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Relaxed);
    }

    /// Whether the flag is raised.
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Relaxed)
    }

    /// `Err(Error::Cancelled)` once the flag is raised, for `?` in loops.
    pub fn check(&self) -> Result<(), crate::Error> {
        if self.is_cancelled() {
            Err(crate::Error::Cancelled)
        } else {
            Ok(())
        }
    }
}
