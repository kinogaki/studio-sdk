//! [`Change`]: what an edit did, in a few numbers.
use std::ops::Range;

/// What an [`Editor`](super::Editor) changed: what a display patches and
/// what a tool reads back. Its size follows the edit, not the value.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Change {
    /// The kept points whose position changed, ascending, by their numbers
    /// in the new value.
    pub moved: Vec<u32>,
    /// What became of the points.
    pub points: Numbers,
    /// What became of the primitives.
    pub primitives: Numbers,
    /// The kept primitives whose corners changed, ascending, by their
    /// numbers in the new value.
    pub rewired: Vec<u32>,
    /// Whether the new value has another primitive table. Without it, its
    /// index, fingerprint and everything derived from them are shared.
    pub topology: bool,
}

/// What became of the elements of one domain.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Numbers {
    /// The new elements, by their numbers in the new value (always the
    /// last ones).
    pub added: Range<u32>,
    /// The elements gone, by their numbers in the edit, ascending.
    pub removed: Vec<u32>,
    /// The points welded away and those they became, by their numbers in
    /// the edit, ascending (never for primitives).
    pub(super) welded: Vec<(u32, u32)>,
    /// The kept elements that took the number of one gone, and those
    /// numbers, ascending. Every other kept element of the base keeps its
    /// number.
    pub(super) filled: Vec<(u32, u32)>,
    /// How many elements the base had: those past them are new.
    pub(super) base: u32,
}

impl Numbers {
    /// Where the element numbered `element` in the edit is in the new
    /// value: `None` once removed. A welded point is where its survivor
    /// is.
    pub fn of(&self, element: u32) -> Option<u32> {
        let element = match self.welded.binary_search_by_key(&element, |w| w.0) {
            Ok(at) => self.welded[at].1,
            Err(_) => element,
        };
        let before = match self.removed.binary_search(&element) {
            Ok(_) => return None,
            Err(before) => before as u32,
        };
        if element >= self.base {
            return Some(element - before);
        }
        Some(match self.filled.binary_search_by_key(&element, |f| f.0) {
            Ok(at) => self.filled[at].1,
            Err(_) => element,
        })
    }

    /// Whether every kept element kept its number.
    pub fn same_numbers(&self) -> bool {
        self.removed.is_empty()
    }
}
