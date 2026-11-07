//! The process's workers as a host installs them, for a test or a
//! measurement of a package's parallel work: the calling thread and scoped
//! helpers taking runs of items from a shared counter ([`install`]).
use kinogaki_studio_api::geometry::parallel::{self, Workers};
use std::ops::Range;
use std::panic::Location;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// How many threads a run takes at most; what [`install`] last asked for.
static THREADS: AtomicUsize = AtomicUsize::new(1);

/// The calling thread and scoped helpers, [`THREADS`] in all.
struct Pool;

impl Workers for Pool {
    fn run(
        &self,
        _: &'static Location<'static>,
        len: usize,
        minimum: usize,
        work: &(dyn Fn(Range<usize>) -> bool + Sync),
    ) {
        let step = minimum.max(1);
        let threads = THREADS.load(Ordering::Relaxed).min(len.div_ceil(step));
        if threads <= 1 {
            if len > 0 {
                work(0..len);
            }
            return;
        }
        let (next, stop) = (AtomicUsize::new(0), AtomicBool::new(false));
        let take = || {
            while !stop.load(Ordering::Relaxed) {
                let start = next.fetch_add(step, Ordering::Relaxed);
                if start >= len {
                    break;
                }
                if !work(start..(start + step).min(len)) {
                    stop.store(true, Ordering::Relaxed);
                }
            }
        };
        std::thread::scope(|scope| {
            (1..threads).for_each(|_| _ = scope.spawn(take));
            take();
        });
    }
}

/// From now on every parallel step of the process runs on `threads`
/// threads, the calling one among them (1: the calling thread alone, in
/// order, as with a host's workers all busy; 0: one a processor). Called
/// again, it changes the count, so a test runs a package's work on one
/// thread and on several and compares.
///
/// ```
/// use kinogaki_studio_api::geometry::parallel;
/// kinogaki_studio_api_testing::workers::install(4);
/// assert_eq!(parallel::map(100, |i| i * 2)[99], 198);
/// ```
pub fn install(threads: usize) {
    let threads = match threads {
        0 => std::thread::available_parallelism().map_or(1, usize::from),
        n => n,
    };
    THREADS.store(threads, Ordering::Relaxed);
    parallel::install(&Pool);
}
