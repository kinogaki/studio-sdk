//! [`ReadBudget`]: what one read of a file may spend, and its
//! cancellation.
//!
//! A file says how much it holds (a count, a size, a depth), and a hostile
//! one says more than it holds. A reader checks each claim against the
//! budget before acting on it, so a small file can't ask Studio for more
//! memory than the machine has, or recurse past the stack it reads on:
//!
//! ```
//! use kinogaki_studio_api::format::{ReadBudget, ReadLimits};
//! let budget = ReadBudget::new(ReadLimits { total: 1 << 20, ..ReadLimits::default() });
//! let fine: Vec<u32> = budget.vec(1000).unwrap();
//! assert!(fine.capacity() >= 1000);
//! // A count from the file asking for 4 GiB in all is refused before anything is allocated.
//! assert!(budget.vec::<u32>(1 << 30).is_err());
//! assert!(budget.nest(300).is_err(), "deeper than 256");
//! assert!(budget.expand(10, 100_000).is_err(), "a decoder giving 10,000 bytes a byte");
//! assert!(budget.pixels(1 << 15, 1 << 15).is_err(), "a picture over 2²⁸ pixels");
//! ```
use crate::Error;
use crate::cancel::CancelToken;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

/// The most a read may spend ([`ReadBudget`]); the defaults suit a
/// workstation reading a file it was handed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ReadLimits {
    /// Bytes of the files a read reads besides the file itself, together
    /// (the files it names, [`Named::read`](super::files::Named::read)):
    /// 16 GiB.
    pub bytes: u64,
    /// Bytes of one allocation sized from what the file says: 4 GiB.
    pub allocation: u64,
    /// Bytes of every such allocation of the read together: 16 GiB.
    pub total: u64,
    /// How deep a reader follows nesting (elements in elements, a layer
    /// naming a layer); 256 levels recurse within the 2 MiB stack a read
    /// runs on.
    pub depth: usize,
    /// Bytes a decoder gives for each byte it decodes: 1032, the most
    /// DEFLATE gives.
    pub ratio: u64,
    /// Pixels of one picture (an image, a part of one, the window its parts
    /// join into): 2²⁸, a picture of 16,384 × 16,384.
    pub pixels: u64,
}

impl Default for ReadLimits {
    fn default() -> Self {
        Self {
            bytes: 16 << 30,
            allocation: 4 << 30,
            total: 16 << 30,
            depth: 256,
            ratio: 1032,
            pixels: 1 << 28,
        }
    }
}

/// What one read may spend, and its cancellation ([`Reading::budget`]).
/// Clones share what was spent, so a reader's workers and the files a
/// file names draw on one budget. Each check also stops a cancelled read,
/// and costs an atomic operation or two: a decoder calls one before each
/// allocation a file sizes, each level it nests, each stream it decodes.
///
/// [`Reading::budget`]: super::Reading::budget
#[derive(Clone, Debug, Default)]
pub struct ReadBudget {
    limits: ReadLimits,
    cancel: CancelToken,
    /// Bytes read of other files, and bytes allocated.
    spent: Arc<[AtomicU64; 2]>,
}

impl ReadBudget {
    /// A budget of `limits`, never cancelled.
    pub fn new(limits: ReadLimits) -> Self {
        Self {
            limits,
            ..Self::default()
        }
    }

    /// The same budget, stopped once `cancel` is raised.
    pub fn cancelled_by(self, cancel: CancelToken) -> Self {
        Self { cancel, ..self }
    }

    /// Its limits.
    pub fn limits(&self) -> &ReadLimits {
        &self.limits
    }

    /// The token stopping the read, for a reader whose own work polls the
    /// flag beneath it ([`CancelToken::shared_flag`]).
    pub fn cancel(&self) -> &CancelToken {
        &self.cancel
    }

    /// `Err(Error::cancelled())` once the read is no longer wanted.
    pub fn check(&self) -> Result<(), Error> {
        self.cancel.check()
    }

    /// Charges `bytes` read from another file than the read's own; refused
    /// past [`ReadLimits::bytes`] in all.
    pub fn take(&self, bytes: u64) -> Result<(), Error> {
        self.check()?;
        let read = self.spent[0].fetch_add(bytes, Ordering::Relaxed);
        match read.saturating_add(bytes) <= self.limits.bytes {
            true => Ok(()),
            false => Err(over("the files read take", bytes, self.limits.bytes)),
        }
    }

    /// Charges an allocation of `bytes` the file sized, before it is made:
    /// refused over [`ReadLimits::allocation`], or past
    /// [`ReadLimits::total`] with what the read allocated before. A size
    /// computed with `saturating_mul` is refused when it overflows.
    pub fn allocate(&self, bytes: u64) -> Result<(), Error> {
        self.check()?;
        if bytes > self.limits.allocation {
            return Err(over("one allocation takes", bytes, self.limits.allocation));
        }
        let before = self.spent[1].fetch_add(bytes, Ordering::Relaxed);
        match before.saturating_add(bytes) <= self.limits.total {
            true => Ok(()),
            false => Err(over(
                "the read's allocations take",
                bytes,
                self.limits.total,
            )),
        }
    }

    /// An empty vector with room for `len` items the file counted, charged
    /// as [`allocate`](Self::allocate).
    pub fn vec<T>(&self, len: usize) -> Result<Vec<T>, Error> {
        let bytes = (len as u64).saturating_mul(size_of::<T>() as u64);
        self.allocate(bytes)?;
        Ok(Vec::with_capacity(len))
    }

    /// Refused past [`ReadLimits::depth`] levels of nesting.
    pub fn nest(&self, depth: usize) -> Result<(), Error> {
        self.check()?;
        match depth <= self.limits.depth {
            true => Ok(()),
            false => Err(Error::invalid(format_args!(
                "nested deeper than {} levels",
                self.limits.depth
            ))),
        }
    }

    /// Refused when decoding `from` bytes would give `to`, more than
    /// [`ReadLimits::ratio`] times as many: a stream claiming more than its
    /// encoding can hold.
    pub fn expand(&self, from: u64, to: u64) -> Result<(), Error> {
        self.check()?;
        match to <= from.saturating_mul(self.limits.ratio) {
            true => Ok(()),
            false => Err(Error::invalid(format_args!(
                "{} said to decode to {}, over {} times its size",
                size(from),
                size(to),
                self.limits.ratio
            ))),
        }
    }

    /// Refused when a picture of `width` × `height` pixels is over
    /// [`ReadLimits::pixels`]: what a picture's header claims is checked
    /// before its samples are sized.
    pub fn pixels(&self, width: u64, height: u64) -> Result<(), Error> {
        self.check()?;
        match width.saturating_mul(height) <= self.limits.pixels {
            true => Ok(()),
            false => Err(Error::invalid(format_args!(
                "{width} × {height} pixels, over the read's limit of {}",
                self.limits.pixels
            ))),
        }
    }
}

/// The budget refusing `what` `asked` bytes, `most` its limit.
fn over(what: &str, asked: u64, most: u64) -> Error {
    Error::invalid(format_args!(
        "{what} {}, over the read's budget of {}",
        size(asked),
        size(most)
    ))
}

/// `bytes` as a person reads a size.
fn size(bytes: u64) -> String {
    const UNITS: [&str; 4] = ["KiB", "MiB", "GiB", "TiB"];
    if bytes < 1024 {
        return format!("{bytes} B");
    }
    let mut value = bytes as f64 / 1024.0;
    let mut unit = 0;
    while value >= 1024.0 && unit + 1 < UNITS.len() {
        value /= 1024.0;
        unit += 1;
    }
    format!("{value:.1} {}", UNITS[unit])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allocations_are_refused_alone_and_together_and_clones_share_them() {
        let budget = ReadBudget::new(ReadLimits {
            allocation: 100,
            total: 250,
            ..ReadLimits::default()
        });
        assert!(budget.allocate(101).is_err());
        let worker = budget.clone();
        assert!(budget.allocate(100).is_ok() && worker.allocate(100).is_ok());
        let why = worker.allocate(100).unwrap_err();
        assert!(
            why.message().contains("over the read's budget of 250 B"),
            "{why}"
        );
        assert!(
            budget.vec::<u64>(usize::MAX).is_err(),
            "an overflowing size"
        );
    }

    #[test]
    fn files_read_depth_and_expansion_have_their_limits() {
        let budget = ReadBudget::new(ReadLimits {
            bytes: 10,
            ..ReadLimits::default()
        });
        assert!(budget.take(6).is_ok());
        assert!(budget.take(6).is_err());
        assert!(budget.nest(256).is_ok() && budget.nest(257).is_err());
        assert!(budget.expand(1, 1032).is_ok() && budget.expand(1, 1033).is_err());
        assert!(budget.expand(0, 0).is_ok());
        assert!(budget.pixels(1 << 14, 1 << 14).is_ok());
        assert!(budget.pixels((1 << 14) + 1, 1 << 14).is_err());
        assert!(budget.pixels(u64::MAX, 2).is_err(), "an overflowing area");
        assert_eq!(size(4 << 30), "4.0 GiB");
    }

    #[test]
    fn every_check_stops_a_cancelled_read() {
        let token = CancelToken::default();
        let budget = ReadBudget::default().cancelled_by(token.clone());
        assert!(budget.check().is_ok() && budget.allocate(1).is_ok());
        token.cancel();
        for stopped in [
            budget.check(),
            budget.allocate(1),
            budget.take(1),
            budget.nest(1),
            budget.expand(1, 1),
            budget.pixels(1, 1),
        ] {
            assert_eq!(stopped, Err(Error::cancelled()));
        }
        assert!(budget.cancel().is_cancelled());
    }
}
