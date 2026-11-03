//! The stable-id rule: a value's standard `id` attribute names each
//! element of its domain once.
use crate::parallel;

/// Values per parallel block.
const BLOCK: usize = 65_536;
/// Hash buckets the values are sorted in, each checked on its own.
const BUCKETS: usize = 64;

/// Whether `new` is some of `old`'s ids in `old`'s order, then ascending
/// ids above all of them: names each element once if `old` does, and tells
/// so in one pass. What a kept column with fresh ids is, so an edit adding
/// a few elements doesn't check a million again.
pub(super) fn kept_or_above(new: &[i64], old: &[i64]) -> bool {
    let mut rest = old.iter();
    let kept = new.iter().take_while(|&id| rest.any(|o| o == id)).count();
    let Some(&top) = old.iter().max() else {
        return false;
    };
    let fresh = &new[kept..];
    fresh.first().is_none_or(|&first| first > top) && fresh.is_sorted_by(|a, b| a < b)
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
