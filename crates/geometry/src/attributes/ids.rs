//! The stable-id rule: a value's standard `id` attribute names each
//! element of its domain once.
use crate::column::Column;
use crate::parallel;

/// Values per parallel block.
const BLOCK: usize = 65_536;
/// Hash buckets the values are sorted in, each checked on its own.
const BUCKETS: usize = 64;

/// Whether `new` is `old` with the ids of its other cells rearranged, some
/// gone, and new ones above all of them: names each element once if `old`
/// does, and tells so from the cells it doesn't share. What an edit's column
/// is (kept ids where they were, the last ones filling holes, fresh ids
/// after), so an edit doesn't check a million again.
pub(super) fn kept_or_above(new: &Column<i64>, old: &Column<i64>) -> bool {
    let Some(top) = highest_id(old) else {
        return false;
    };
    let shared = old.len().min(new.len());
    let differing = || (new.unshared(old)).map(|r| r.start.min(shared)..r.end.min(shared));
    let of = |ids: &Column<i64>| -> Vec<i64> {
        let mut list: Vec<i64> = differing().flatten().map(|i| ids[i]).collect();
        list.extend((shared..ids.len()).map(|i| ids[i]));
        list.sort_unstable();
        list
    };
    let (was, now) = (of(old), of(new));
    // Each id at most once, below the top from those the old cells held.
    let mut held = was.iter().peekable();
    let fine = now.iter().enumerate().all(|(i, &id)| {
        let once = i == 0 || now[i - 1] != id;
        once && (id > top || {
            while held.next_if(|&&h| h < id).is_some() {}
            held.peek() == Some(&&id)
        })
    });
    if fine {
        // What a later edit's check and fresh ids would look for.
        let highest = now.last().map_or(top, |&last| last.max(top));
        new.derived(|_| Highest(Some(highest)));
    }
    fine
}

/// The highest id of a column: its cache.
struct Highest(Option<i64>);

/// The highest of `ids`, found once per column.
pub fn highest_id(ids: &Column<i64>) -> Option<i64> {
    ids.derived(|ids| Highest(ids.iter().copied().max())).0
}

/// The first id (in bucket order) that `ids` holds twice, if any. Large
/// columns are split by a hash of the id into buckets, each sorted and
/// checked in parallel.
pub(super) fn repeated(ids: &[i64]) -> Option<i64> {
    if ids.len() <= BLOCK {
        let mut sorted = ids.to_vec();
        sorted.sort_unstable();
        return sorted.windows(2).find(|w| w[0] == w[1]).map(|w| w[0]);
    }
    let bucket = |id: i64| (id as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15) as usize >> 58;
    // Each block's ids by bucket, then each bucket's ids from every block.
    let blocks: Vec<Vec<Vec<i64>>> = parallel::map(ids.len(), BLOCK, |range| {
        let mut out = vec![Vec::new(); BUCKETS];
        for &id in &ids[range] {
            out[bucket(id)].push(id);
        }
        out
    });
    let found = parallel::map(BUCKETS, 1, |range| {
        range.into_iter().find_map(|b| {
            let mut all: Vec<i64> = blocks.iter().flat_map(|block| &block[b]).copied().collect();
            all.sort_unstable();
            all.windows(2).find(|w| w[0] == w[1]).map(|w| w[0])
        })
    });
    found.into_iter().flatten().next()
}
