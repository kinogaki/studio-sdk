//! [`Kept`]: results derived from a column together with other parts of a
//! value (a positions column with the topology or attribute columns, a
//! text's indices with its table), kept on that column for the last two
//! sets of other parts.
use std::sync::{Arc, Mutex};

/// How many sets of other parts a column keeps a result for.
const KEPT: usize = 2;

/// A column's results, each with the other parts it was derived
/// from. Each key holds those parts (shared buffers), so comparing them by
/// identity stays valid while they are kept (the crate's cache rule).
pub(crate) struct Kept<K, V> {
    entries: Mutex<Vec<(K, Arc<V>)>>,
}

impl<K, V> Default for Kept<K, V> {
    fn default() -> Self {
        Self {
            entries: Mutex::new(Vec::new()),
        }
    }
}

impl<K, V> Kept<K, V> {
    /// The result kept for the key `matches` accepts.
    pub(crate) fn find(&self, matches: impl Fn(&K) -> bool) -> Option<Arc<V>> {
        let entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries
            .iter()
            .find(|(k, _)| matches(k))
            .map(|(_, v)| v.clone())
    }

    /// Keeps `value` for `key`, the oldest dropped beyond [`KEPT`].
    pub(crate) fn keep(&self, key: K, value: Arc<V>) {
        let mut entries = self.entries.lock().unwrap_or_else(|e| e.into_inner());
        entries.push((key, value));
        if entries.len() > KEPT {
            entries.remove(0);
        }
    }
}
