//! Blocks of work spread over scoped threads: what validation and the
//! topology index use on large values. Work of one block runs inline, with
//! no thread started; otherwise up to one thread per core takes blocks in
//! turn. Results come back in block order, whatever the timing.
use std::ops::Range;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, OnceLock};

/// How many threads a parallel step uses at most.
fn threads() -> usize {
    static THREADS: OnceLock<usize> = OnceLock::new();
    *THREADS.get_or_init(|| std::thread::available_parallelism().map_or(1, |n| n.get()))
}

/// Runs `worker` on `count` threads (the calling one among them).
fn spread(count: usize, worker: impl Fn() + Sync) {
    std::thread::scope(|scope| {
        for _ in 1..count {
            scope.spawn(&worker);
        }
        worker();
    });
}

/// `work` over `0..len` cut into ranges of `block`, one result a range,
/// in range order.
pub(crate) fn map_blocks<R: Send>(
    len: usize,
    block: usize,
    work: impl Fn(Range<usize>) -> R + Sync,
) -> Vec<R> {
    let blocks = len.div_ceil(block);
    let range = |b: usize| b * block..((b + 1) * block).min(len);
    if blocks <= 1 || threads() == 1 {
        return (0..blocks).map(|b| work(range(b))).collect();
    }
    let next = AtomicUsize::new(0);
    let done = Mutex::new(Vec::with_capacity(blocks));
    spread(threads().min(blocks), || {
        let mut mine = Vec::new();
        loop {
            let b = next.fetch_add(1, Ordering::Relaxed);
            if b >= blocks {
                break;
            }
            mine.push((b, work(range(b))));
        }
        done.lock().unwrap_or_else(|e| e.into_inner()).extend(mine);
    });
    let mut done = done.into_inner().unwrap_or_else(|e| e.into_inner());
    done.sort_unstable_by_key(|(b, _)| *b);
    done.into_iter().map(|(_, r)| r).collect()
}

/// `work(i)` for every `i` in `0..len`, in blocks of `block`.
pub(crate) fn each(len: usize, block: usize, work: impl Fn(usize) + Sync) {
    map_blocks(len, block, |range| range.for_each(&work));
}

/// `work(first, chunk)` over consecutive chunks of `block` values, `first`
/// being the chunk's first index.
pub(crate) fn chunks_mut<T: Send>(
    values: &mut [T],
    block: usize,
    work: impl Fn(usize, &mut [T]) + Sync,
) {
    let blocks: Vec<(usize, &mut [T])> = values
        .chunks_mut(block.max(1))
        .enumerate()
        .map(|(b, chunk)| (b * block, chunk))
        .collect();
    each_block(blocks, |(first, chunk)| work(first, chunk));
}

/// `work` on each of `blocks`, prepared by the caller.
pub(crate) fn each_block<T: Send>(blocks: Vec<T>, work: impl Fn(T) + Sync) {
    if blocks.len() <= 1 || threads() == 1 {
        blocks.into_iter().for_each(work);
        return;
    }
    let count = threads().min(blocks.len());
    let queue = Mutex::new(blocks.into_iter());
    spread(count, || {
        loop {
            let taken = queue.lock().unwrap_or_else(|e| e.into_inner()).next();
            match taken {
                Some(block) => work(block),
                None => break,
            }
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn results_come_back_in_block_order() {
        let sums = map_blocks(100_000, 1000, |r| r.start);
        assert_eq!(sums, (0..100).map(|b| b * 1000).collect::<Vec<_>>());
        let mut values = vec![0usize; 10_001];
        chunks_mut(&mut values, 100, |first, chunk| {
            for (i, v) in chunk.iter_mut().enumerate() {
                *v = first + i;
            }
        });
        assert!(values.iter().enumerate().all(|(i, &v)| i == v));
        assert!(map_blocks(0, 10, |r| r.len()).is_empty());
    }
}
