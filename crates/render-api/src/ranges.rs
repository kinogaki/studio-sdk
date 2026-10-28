//! Changed element ranges of a stream (a drag's points), coalesced: spans
//! closer than a small gap join, and past [`Ranges::MOST`] spans the
//! change counts as the whole stream.

use std::ops::Range;

/// Coalesced, sorted, disjoint element ranges, or the whole stream.
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub struct Ranges {
    spans: Vec<Range<u32>>,
    whole: bool,
}

impl Ranges {
    /// Spans kept before the ranges become the whole stream.
    pub const MOST: usize = 1024;
    /// Elements between two spans that cost less to rewrite than a second span.
    pub const GAP: u32 = 16;

    /// No changed elements.
    pub fn new() -> Ranges {
        Ranges::default()
    }

    /// Every element changed.
    pub fn whole() -> Ranges {
        Ranges {
            spans: Vec::new(),
            whole: true,
        }
    }

    /// The single range `range`.
    pub fn of(range: Range<u32>) -> Ranges {
        let mut ranges = Ranges::new();
        ranges.add(range);
        ranges
    }

    /// Whether every element counts as changed.
    pub fn is_whole(&self) -> bool {
        self.whole
    }

    /// Whether nothing changed.
    pub fn is_empty(&self) -> bool {
        !self.whole && self.spans.is_empty()
    }

    /// The spans (empty when whole: the caller rewrites everything).
    pub fn spans(&self) -> &[Range<u32>] {
        &self.spans
    }

    /// Elements the spans cover (none when whole).
    pub fn len(&self) -> u64 {
        self.spans.iter().map(|r| u64::from(r.end - r.start)).sum()
    }

    /// Add `range`, joining spans it overlaps or comes within [`Self::GAP`] of.
    pub fn add(&mut self, mut range: Range<u32>) {
        if self.whole || range.start >= range.end {
            return;
        }
        let first = self
            .spans
            .partition_point(|r| r.end.saturating_add(Self::GAP) < range.start);
        let mut end = first;
        while end < self.spans.len() && self.spans[end].start <= range.end.saturating_add(Self::GAP)
        {
            range.start = range.start.min(self.spans[end].start);
            range.end = range.end.max(self.spans[end].end);
            end += 1;
        }
        self.spans.splice(first..end, [range]);
        if self.spans.len() > Self::MOST {
            *self = Ranges::whole();
        }
    }

    /// The union with `other`.
    pub fn merge(&mut self, other: &Ranges) {
        if other.whole {
            *self = Ranges::whole();
            return;
        }
        for range in &other.spans {
            self.add(range.clone());
        }
    }
}
