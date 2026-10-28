//! The environment's shared workers: [`Workers`], the host service behind
//! [`CookCx::parallel_for`](crate::CookCx::parallel_for) and
//! [`CookCx::parallel_map`](crate::CookCx::parallel_map).
//!
//! Packages never start threads: the host runs their parallel work within
//! its own budget, beside the UI and every other cook. Studio implements
//! [`Workers`] over its scheduler, which learns from each call site how
//! costly its items are and sizes later runs by that; so every call
//! forwards its caller's [`Location`]. [`Serial`] runs everything on the
//! calling thread, for package tests.
use std::ops::Range;
use std::panic::Location;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

/// Runs a package's parallel work.
pub trait Workers: Send + Sync {
    /// Calls `work` with disjoint consecutive ranges covering `0..len`,
    /// on the calling thread and helpers, and returns once every call has
    /// finished. Each range holds at least `minimum` items (the last may
    /// hold fewer). Once a call returns `false`, no further range is
    /// handed out. A panic in `work` is resumed on the caller after the
    /// others finish. `site` is the package's call site, for the cost
    /// memory.
    fn run(
        &self,
        site: &'static Location<'static>,
        len: usize,
        minimum: usize,
        work: &(dyn Fn(Range<usize>) -> bool + Sync),
    );
}

/// [`Workers`] that run everything on the calling thread, in order: for
/// tests and for hosts without helpers.
#[derive(Clone, Copy, Debug, Default)]
pub struct Serial;

impl Workers for Serial {
    fn run(
        &self,
        _site: &'static Location<'static>,
        len: usize,
        _minimum: usize,
        work: &(dyn Fn(Range<usize>) -> bool + Sync),
    ) {
        if len > 0 {
            work(0..len);
        }
    }
}

/// At most this many pieces a slice is split into for
/// [`for_each_block`]: the host schedules pieces, never single items.
const MOST_PIECES: usize = 4096;

/// `work(first_index, block)` over disjoint blocks of `values`, each of at
/// least `minimum` items, on `workers`. The first error stops handing out
/// blocks and is returned.
pub(crate) fn for_each_block<T: Send, E: Send>(
    workers: &dyn Workers,
    site: &'static Location<'static>,
    values: &mut [T],
    minimum: usize,
    work: impl Fn(usize, &mut [T]) -> Result<(), E> + Sync,
) -> Result<(), E> {
    let len = values.len();
    if len == 0 {
        return Ok(());
    }
    // Pieces of at least `minimum` items: the host hands out runs of
    // pieces, each taken from its lock exactly once.
    let piece = minimum.max(len.div_ceil(MOST_PIECES)).max(1);
    let pieces: Vec<Mutex<Option<&mut [T]>>> = values
        .chunks_mut(piece)
        .map(|c| Mutex::new(Some(c)))
        .collect();
    let error = Mutex::new(None);
    let stop = AtomicBool::new(false);
    workers.run(site, pieces.len(), 1, &|range| {
        for index in range {
            if stop.load(Ordering::Relaxed) {
                return false;
            }
            let taken = pieces[index]
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .take();
            let Some(block) = taken else { continue };
            if let Err(e) = work(index * piece, block) {
                stop.store(true, Ordering::Relaxed);
                error
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .get_or_insert(e);
                return false;
            }
        }
        true
    });
    match error.into_inner().unwrap_or_else(|e| e.into_inner()) {
        Some(e) => Err(e),
        None => Ok(()),
    }
}

/// `work` over consecutive ranges of `0..len`, `block` items each (the
/// last may be shorter), on `workers`; results in range order.
pub(crate) fn map_blocks<T: Send, E: Send>(
    workers: &dyn Workers,
    site: &'static Location<'static>,
    len: usize,
    block: usize,
    work: impl Fn(Range<usize>) -> Result<T, E> + Sync,
) -> Result<Vec<T>, E> {
    let block = block.max(1);
    let mut slots: Vec<Option<T>> = (0..len.div_ceil(block)).map(|_| None).collect();
    for_each_block(workers, site, &mut slots, 1, |first, chunk| {
        for (i, slot) in chunk.iter_mut().enumerate() {
            let start = (first + i) * block;
            *slot = Some(work(start..(start + block).min(len))?);
        }
        Ok(())
    })?;
    Ok(slots
        .into_iter()
        .map(|slot| slot.expect("every block ran"))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Workers that hand out ranges of `minimum` items from several
    /// threads, out of order, as a real host may.
    struct Scattered;
    impl Workers for Scattered {
        fn run(
            &self,
            _: &'static Location<'static>,
            len: usize,
            minimum: usize,
            work: &(dyn Fn(Range<usize>) -> bool + Sync),
        ) {
            let step = minimum.max(1);
            let starts: Vec<usize> = (0..len).step_by(step).rev().collect();
            std::thread::scope(|s| {
                for chunk in starts.chunks(3) {
                    s.spawn(move || {
                        for &start in chunk {
                            if !work(start..(start + step).min(len)) {
                                return;
                            }
                        }
                    });
                }
            });
        }
    }

    #[test]
    fn every_item_is_visited_once_with_its_index() {
        for workers in [&Serial as &dyn Workers, &Scattered] {
            for (len, minimum) in [(0, 1), (1, 1), (10, 3), (10_000, 1), (100_000, 64)] {
                let mut values = vec![usize::MAX; len];
                for_each_block(
                    workers,
                    Location::caller(),
                    &mut values,
                    minimum,
                    |first, b| {
                        assert!(b.len() >= minimum.min(len - first));
                        for (i, v) in b.iter_mut().enumerate() {
                            assert_eq!(*v, usize::MAX);
                            *v = first + i;
                        }
                        Ok::<(), ()>(())
                    },
                )
                .unwrap();
                assert!(values.iter().enumerate().all(|(i, &v)| i == v));
            }
        }
    }

    #[test]
    fn maps_come_back_in_order_and_errors_stop() {
        for workers in [&Serial as &dyn Workers, &Scattered] {
            let sums = map_blocks(workers, Location::caller(), 10, 3, |r| {
                Ok::<_, ()>(r.sum::<usize>())
            })
            .unwrap();
            assert_eq!(sums, [3, 12, 21, 9]);
            let failed = map_blocks(workers, Location::caller(), 100, 1, |r| {
                if r.start == 50 { Err(r.start) } else { Ok(()) }
            });
            assert_eq!(failed, Err(50));
        }
    }
}
