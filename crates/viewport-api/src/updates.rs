//! Sparse upload ranges for packets derived from an earlier packet.
use std::{ops::Range, sync::Arc};

/// Bounded, coalesced upload spans. Small gaps cost less than another copy;
/// beyond [`Self::MOST`] spans an edit counts as scattered and falls back to
/// one full stream. Renderers stage many spans in one upload buffer, so an
/// edit along a whole edge loop (a few thousand spans) stays a patch.
#[derive(Clone, Debug)]
pub struct BufferRanges {
    ranges: Vec<Range<usize>>,
    len: usize,
    gap: usize,
}
impl BufferRanges {
    /// Spans kept before the ranges become the whole stream.
    pub const MOST: usize = 8192;
    /// No ranges over a stream of `len` elements of `T`.
    pub fn new<T>(len: usize) -> Self {
        Self {
            ranges: Vec::new(),
            len,
            gap: 256 / std::mem::size_of::<T>(),
        }
    }
    /// Adds `range` (within the stream), merging close spans.
    pub fn add(&mut self, mut range: Range<usize>) {
        debug_assert!(range.start <= range.end && range.end <= self.len);
        if range.is_empty() {
            return;
        }
        let first = self
            .ranges
            .partition_point(|r| r.end.saturating_add(self.gap) < range.start);
        let mut end = first;
        while end < self.ranges.len()
            && self.ranges[end].start <= range.end.saturating_add(self.gap)
        {
            range.start = range.start.min(self.ranges[end].start);
            range.end = range.end.max(self.ranges[end].end);
            end += 1;
        }
        self.ranges.splice(first..end, [range]);
        if self.ranges.len() > Self::MOST {
            self.ranges.clear();
            self.ranges.push(0..self.len);
        }
    }
    /// The spans, in order.
    pub fn iter(&self) -> impl Iterator<Item = Range<usize>> + '_ {
        self.ranges.iter().cloned()
    }
    /// Add every range of `other` (the union), within this stream's length:
    /// elements a shorter stream dropped need no upload.
    pub fn merge(&mut self, other: &Self) {
        for range in other.iter() {
            self.add(range.start.min(self.len)..range.end.min(self.len));
        }
    }
}

/// Ranges changed since a packet the renderer may hold. The tokens retain no
/// mesh data. A patch applies over its exact parent, or over one of a few
/// recent ancestors with the union of the changes since that ancestor: the
/// geometry worker can finish packets the UI never displays (a newer edit
/// arrived first), and the next packet must still patch the one on the GPU
/// rather than re-upload the whole mesh.
#[derive(Clone, Debug)]
pub struct DrawUpdate {
    /// The packet's exact parent.
    pub parent: Arc<()>,
    /// Point positions changed.
    pub positions: BufferRanges,
    /// Vertex records changed.
    pub vertices: BufferRanges,
    /// Triangle indices changed.
    pub indices: BufferRanges,
    /// Line segment records ([`crate::OverlayVertex`]) and point markers
    /// rewritten in place (a CV drag's hull and CV ranges).
    pub lines: BufferRanges,
    /// Point markers rewritten in place.
    pub points: BufferRanges,
    /// The packet extends its parent's topology: streams grew (ranges cover
    /// the appended elements, in the new lengths), edges renumbered (the
    /// line stream is rewritten whole) and point markers were appended
    /// (rewrite them over the position ranges).
    pub extends: bool,
    /// Older packets this update also applies over, newest first, each with
    /// the ranges changed since it.
    pub ancestors: Vec<Ancestor>,
}
#[derive(Clone, Debug)]
/// An older packet a [`DrawUpdate`] also applies over.
pub struct Ancestor {
    /// Its identity token.
    pub identity: Arc<()>,
    /// Point positions changed since it.
    pub positions: BufferRanges,
    /// Vertex records changed since it.
    pub vertices: BufferRanges,
    /// Triangle indices changed since it.
    pub indices: BufferRanges,
    /// Line segment records changed since it.
    pub lines: BufferRanges,
    /// Point markers changed since it.
    pub points: BufferRanges,
    /// Some packet since this one extended the topology.
    pub extends: bool,
}
/// The streams to rewrite over a held packet.
#[derive(Clone, Copy, Debug)]
pub struct UpdateRanges<'a> {
    /// Point positions.
    pub positions: &'a BufferRanges,
    /// Vertex records.
    pub vertices: &'a BufferRanges,
    /// Triangle indices.
    pub indices: &'a BufferRanges,
    /// Line segment records.
    pub lines: &'a BufferRanges,
    /// Point markers.
    pub points: &'a BufferRanges,
    /// See [`DrawUpdate::extends`].
    pub extends: bool,
}
impl DrawUpdate {
    /// Undisplayed packets bridged before a patch gives up (a full upload).
    pub const ANCESTORS: usize = 4;
    /// A new update with no ancestors. `streams` are the position, vertex,
    /// index, line and point ranges.
    pub fn new(parent: Arc<()>, streams: [BufferRanges; 5]) -> Self {
        let [positions, vertices, indices, lines, points] = streams;
        Self {
            parent,
            positions,
            vertices,
            indices,
            lines,
            points,
            extends: false,
            ancestors: Vec::new(),
        }
    }
    /// Ranges to rewrite over the packet `held`, if this update reaches it.
    pub fn ranges_from(&self, held: &Arc<()>) -> Option<UpdateRanges<'_>> {
        if Arc::ptr_eq(&self.parent, held) {
            return Some(UpdateRanges {
                positions: &self.positions,
                vertices: &self.vertices,
                indices: &self.indices,
                lines: &self.lines,
                points: &self.points,
                extends: self.extends,
            });
        }
        self.ancestors
            .iter()
            .find(|a| Arc::ptr_eq(&a.identity, held))
            .map(|a| UpdateRanges {
                positions: &a.positions,
                vertices: &a.vertices,
                indices: &a.indices,
                lines: &a.lines,
                points: &a.points,
                extends: a.extends,
            })
    }
    /// Also apply over the packets `previous` (the parent's update) reached:
    /// each with this update's ranges added to the ranges since it.
    pub fn inherit(&mut self, previous: &DrawUpdate) {
        // Ranges are kept in this packet's lengths; an older packet's
        // ranges past them (a stream that shrank) are dropped.
        let since = |identity: &Arc<()>, streams: [&BufferRanges; 5], extends: bool| {
            let mut ancestor = Ancestor {
                identity: identity.clone(),
                positions: self.positions.clone(),
                vertices: self.vertices.clone(),
                indices: self.indices.clone(),
                lines: self.lines.clone(),
                points: self.points.clone(),
                extends: extends || self.extends,
            };
            ancestor.positions.merge(streams[0]);
            ancestor.vertices.merge(streams[1]);
            ancestor.indices.merge(streams[2]);
            ancestor.lines.merge(streams[3]);
            ancestor.points.merge(streams[4]);
            ancestor
        };
        let mut ancestors = vec![since(
            &previous.parent,
            [
                &previous.positions,
                &previous.vertices,
                &previous.indices,
                &previous.lines,
                &previous.points,
            ],
            previous.extends,
        )];
        ancestors.extend(
            previous
                .ancestors
                .iter()
                .take(Self::ANCESTORS - 1)
                .map(|a| {
                    since(
                        &a.identity,
                        [&a.positions, &a.vertices, &a.indices, &a.lines, &a.points],
                        a.extends,
                    )
                }),
        );
        self.ancestors = ancestors;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_coalesce_small_gaps_and_fall_back_to_one_full_write() {
        let mut ranges = BufferRanges::new::<u32>(10_000);
        ranges.add(0..4);
        ranges.add(10..12);
        ranges.add(500..510);
        ranges.add(3..3);
        assert_eq!(ranges.iter().collect::<Vec<_>>(), [0..12, 500..510]);
        let spaced = BufferRanges::MOST + 1;
        let mut scattered = BufferRanges::new::<u32>(spaced * 100);
        for at in 0..spaced {
            scattered.add(at * 100..at * 100 + 1);
        }
        assert_eq!(scattered.iter().collect::<Vec<_>>(), vec![0..spaced * 100]);
    }

    fn ranges(len: usize, spans: &[std::ops::Range<usize>]) -> BufferRanges {
        let mut r = BufferRanges::new::<[f32; 3]>(len);
        for s in spans {
            r.add(s.clone());
        }
        r
    }
    fn update(parent: &Arc<()>, span: std::ops::Range<usize>) -> DrawUpdate {
        DrawUpdate::new(
            parent.clone(),
            [
                ranges(10_000, &[span]),
                ranges(10_000, &[]),
                ranges(10_000, &[]),
                ranges(10_000, &[]),
                ranges(10_000, &[]),
            ],
        )
    }

    #[test]
    fn updates_bridge_undisplayed_packets_with_the_union_of_their_changes() {
        // Packets a → b → c → …; the GPU may still hold any recent one.
        let ids: Vec<Arc<()>> = (0..8).map(|_| Arc::new(())).collect();
        let mut previous = update(&ids[0], 100..101);
        for (k, id) in ids.iter().enumerate().take(7).skip(1) {
            let mut next = update(id, 1000 * k..1000 * k + 1);
            next.inherit(&previous);
            previous = next;
        }
        // The newest update (6 → 7) patches its parent with its own range…
        let own = previous.ranges_from(&ids[6]).unwrap();
        assert_eq!(own.positions.iter().next(), Some(6000..6001));
        assert_eq!(own.positions.iter().count(), 1);
        // …and an older held packet with every change since it.
        let from_4 = previous.ranges_from(&ids[4]).unwrap();
        assert_eq!(
            from_4.positions.iter().collect::<Vec<_>>(),
            [4000..4001, 5000..5001, 6000..6001]
        );
        // Beyond the window the renderer uploads in full.
        assert!(
            previous
                .ranges_from(&ids[6 - DrawUpdate::ANCESTORS - 1])
                .is_none()
        );
        assert!(previous.ranges_from(&Arc::new(())).is_none());
    }
}
