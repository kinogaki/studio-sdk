//! The process's shared workers: every parallel step of a package and of
//! this crate (validation, the topology index, triangulation, shading) runs
//! on them.
//!
//! Packages never start threads: the host runs their parallel work within
//! its own budget, beside the UI and every other cook. The host installs
//! its [`Workers`] once, as it starts ([`install`]); until then, and in a
//! process that installs none (a package's own tests), everything runs on
//! the calling thread, in order. Studio's workers learn from each call
//! site how costly its items are and size later runs by that, so every
//! call forwards its caller's [`Location`].
//!
//! Runs nest: work running on the workers may start another run, which
//! the workers take on with the helpers they have free (or on its caller
//! alone), never waiting for a helper. Results never depend on how many
//! threads a run got.
use std::convert::Infallible;
use std::ops::Range;
use std::panic::Location;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

/// Runs parallel work: what a host [`install`]s.
pub trait Workers: Send + Sync {
    /// Calls `work` with disjoint consecutive ranges covering `0..len`,
    /// on the calling thread and helpers, and returns once every call has
    /// finished. Each range holds at least `minimum` items (the last may
    /// hold fewer). Once a call returns `false`, no further range is
    /// handed out. A panic in `work` is resumed on the caller after the
    /// others finish. `work` may itself start a run (see the module).
    /// `site` is the call site, for the cost memory.
    fn run(
        &self,
        site: &'static Location<'static>,
        len: usize,
        minimum: usize,
        work: &(dyn Fn(Range<usize>) -> bool + Sync),
    );
}

/// Everything on the calling thread, in order: the workers until a host
/// installs its own.
struct Serial;

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

static INSTALLED: OnceLock<&'static dyn Workers> = OnceLock::new();

/// Runs every later parallel step of the process on `workers`. The host
/// calls it once, as it starts, before any package runs; a later call is
/// ignored, so every run of a process sees the same workers.
pub fn install(workers: &'static dyn Workers) {
    let _ = INSTALLED.set(workers);
}

/// The installed workers, or the calling thread alone.
fn installed() -> &'static dyn Workers {
    INSTALLED.get().copied().unwrap_or(&Serial)
}

/// At most this many pieces a slice is split into: the workers schedule
/// pieces, never single items.
const MOST_PIECES: usize = 4096;

/// `work(first_index, block)` over disjoint blocks of `values`, each of at
/// least `minimum` items (the last may hold fewer), on the installed
/// workers. The first error stops handing out blocks and is returned once
/// every running block has finished.
#[track_caller]
pub fn chunks_mut<T: Send, E: Send>(
    values: &mut [T],
    minimum: usize,
    work: impl Fn(usize, &mut [T]) -> Result<(), E> + Sync,
) -> Result<(), E> {
    chunks_on(installed(), Location::caller(), values, minimum, work)
}

/// `work` over consecutive ranges of `0..len`, `block` items each (the
/// last may be shorter), on the installed workers; results in range order,
/// so outputs of variable length per item concatenate deterministically.
/// Errors behave as in [`chunks_mut`].
#[track_caller]
pub fn map_blocks<T: Send, E: Send>(
    len: usize,
    block: usize,
    work: impl Fn(Range<usize>) -> Result<T, E> + Sync,
) -> Result<Vec<T>, E> {
    map_on(installed(), Location::caller(), len, block, work)
}

/// [`chunks_mut`] on `workers`.
fn chunks_on<T: Send, E: Send>(
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
    // Pieces of at least `minimum` items: the workers hand out runs of
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

/// [`map_blocks`] on `workers`.
fn map_on<T: Send, E: Send>(
    workers: &dyn Workers,
    site: &'static Location<'static>,
    len: usize,
    block: usize,
    work: impl Fn(Range<usize>) -> Result<T, E> + Sync,
) -> Result<Vec<T>, E> {
    let block = block.max(1);
    let mut slots: Vec<Option<T>> = (0..len.div_ceil(block)).map(|_| None).collect();
    chunks_on(workers, site, &mut slots, 1, |first, chunk| {
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

/// What a run that cannot fail gave.
fn sure<T>(result: Result<T, Infallible>) -> T {
    match result {
        Ok(value) => value,
        Err(never) => match never {},
    }
}

/// [`map_blocks`] of work that cannot fail.
#[track_caller]
pub(crate) fn map<R: Send>(
    len: usize,
    block: usize,
    work: impl Fn(Range<usize>) -> R + Sync,
) -> Vec<R> {
    sure(map_blocks(len, block, |range| Ok(work(range))))
}

/// The first of `values` that `fine` refuses, searched in blocks of
/// `block`: a branch-free pass says whether a block holds one, and only
/// then is it searched (what validation does on large columns).
#[track_caller]
pub(crate) fn first_refused<T: Sync>(
    values: &[T],
    block: usize,
    fine: impl Fn(&T) -> bool + Sync,
) -> Option<usize> {
    let bad = map(values.len(), block, |range| {
        let start = range.start;
        let values = &values[range];
        if values.iter().fold(true, |ok, v| ok & fine(v)) {
            return None;
        }
        values.iter().position(|v| !fine(v)).map(|i| start + i)
    });
    bad.into_iter().flatten().next()
}

/// `work(i)` for every `i` in `0..len`, in blocks of `block`.
#[track_caller]
pub(crate) fn each(len: usize, block: usize, work: impl Fn(usize) + Sync) {
    map(len, block, |range| range.for_each(&work));
}

/// [`chunks_mut`] of work that cannot fail.
#[track_caller]
pub(crate) fn chunks<T: Send>(
    values: &mut [T],
    block: usize,
    work: impl Fn(usize, &mut [T]) + Sync,
) {
    sure(chunks_mut(values, block, |first, chunk| {
        work(first, chunk);
        Ok(())
    }));
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
                let site = Location::caller();
                chunks_on(workers, site, &mut values, minimum, |first, b| {
                    assert!(b.len() >= minimum.min(len - first));
                    for (i, v) in b.iter_mut().enumerate() {
                        assert_eq!(*v, usize::MAX);
                        *v = first + i;
                    }
                    Ok::<(), ()>(())
                })
                .unwrap();
                assert!(values.iter().enumerate().all(|(i, &v)| i == v));
            }
        }
    }

    #[test]
    fn maps_come_back_in_order_and_errors_stop() {
        let site = Location::caller();
        for workers in [&Serial as &dyn Workers, &Scattered] {
            let sums = map_on(workers, site, 10, 3, |r| Ok::<_, ()>(r.sum::<usize>())).unwrap();
            assert_eq!(sums, [3, 12, 21, 9]);
            let failed = map_on(workers, site, 100, 1, |r| {
                if r.start == 50 { Err(r.start) } else { Ok(()) }
            });
            assert_eq!(failed, Err(50));
            let empty = map_on(workers, site, 0, 10, |r| Ok::<_, ()>(r.len()));
            assert!(empty.unwrap().is_empty());
        }
    }

    #[test]
    fn prepared_blocks_and_chunks_cover_everything() {
        let mut values = vec![0usize; 10_001];
        chunks(&mut values, 100, |first, chunk| {
            for (i, v) in chunk.iter_mut().enumerate() {
                *v = first + i;
            }
        });
        assert!(values.iter().enumerate().all(|(i, &v)| i == v));
    }

    #[test]
    fn the_first_refused_value_is_found_in_any_block() {
        let mut values = vec![0.0_f64; 10_000];
        let finite = |x: &f64| x.is_finite();
        assert_eq!(first_refused(&values, 100, finite), None);
        values[7_777] = f64::INFINITY;
        values[9_000] = f64::NAN;
        values[123] = f64::NAN;
        for block in [1, 7, 100, 10_000, 20_000] {
            let found = first_refused(&values, block, finite);
            assert_eq!(found, values.iter().position(|x| !x.is_finite()), "{block}");
        }
        assert_eq!(first_refused(&[] as &[f64], 100, finite), None);
    }
}
