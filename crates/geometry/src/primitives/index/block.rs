//! A block of an [`Index`](super::Index): every row of [`POINTS`] points,
//! made from the corners on each point. A point's edges with a higher point
//! are numbered from 0 within the block, in order, and their rows kept
//! here, so nothing in a block changes when edges come or go elsewhere.
use super::Primitives;
use crate::primitives::Edge;
use std::ops::Range;
use std::sync::LazyLock;

/// `log2` of the points a block holds.
pub(super) const SHIFT: usize = 10;
/// The points a block holds.
pub(super) const POINTS: usize = 1 << SHIFT;

/// A block with no rows: every point past a table's blocks.
pub(super) static EMPTY: LazyLock<Block> = LazyLock::new(Block::default);

/// Rows of ids, stored contiguously. Rows past the last stored are empty
/// (a table with no entries at all stores none).
#[derive(Debug, Default)]
pub(super) struct Rows {
    offsets: Vec<u32>,
    entries: Vec<u32>,
}

impl Rows {
    /// Row `i`.
    #[inline]
    pub(super) fn row(&self, i: usize) -> &[u32] {
        match (self.offsets.get(i), self.offsets.get(i + 1)) {
            (Some(&start), Some(&end)) => &self.entries[start as usize..end as usize],
            _ => &[],
        }
    }

    /// Where row `i` starts in the entries.
    #[inline]
    fn start(&self, i: usize) -> u32 {
        self.offsets
            .get(i)
            .copied()
            .unwrap_or(self.entries.len() as u32)
    }

    /// Ends the row being written.
    fn end_row(&mut self) {
        if self.offsets.is_empty() {
            self.offsets.push(0);
        }
        self.offsets.push(self.entries.len() as u32);
    }

    /// `row` next.
    fn push(&mut self, row: &[u32]) {
        self.entries.extend_from_slice(row);
        self.end_row();
    }

    /// This table at its size, storing nothing when it has no entries.
    fn compact(&mut self) {
        if self.entries.is_empty() {
            self.offsets = Vec::new();
        }
        self.offsets.shrink_to_fit();
        self.entries.shrink_to_fit();
    }

    fn bytes(&self) -> usize {
        4 * (self.offsets.capacity() + self.entries.capacity())
    }
}

/// What one point's rows are made from: the corners on it, gathered through
/// [`add`](Self::add).
#[derive(Default)]
pub(super) struct Corners {
    faces: Vec<u32>,
    lines: Vec<u32>,
    /// `(other point, primitive, whether a polygon)`: each segment at the
    /// point.
    segments: Vec<(u32, u32, bool)>,
    above_faces: Vec<u32>,
    above_lines: Vec<u32>,
}

impl Corners {
    /// Corner (vertex) `c` of primitive `f` of `t`, on the point: the
    /// primitive and the segments on either side of it.
    #[inline]
    pub(super) fn add(&mut self, t: &Primitives, f: usize, c: usize) {
        let row = t.primitive_vertices(f);
        let kind = t.kind(f);
        let polygon = kind.is_polygon();
        let closed = kind.closed();
        let f = f as u32;
        if polygon {
            self.faces.push(f);
        } else {
            self.lines.push(f);
        }
        let next = if c + 1 < row.end {
            Some(c + 1)
        } else if closed {
            Some(row.start)
        } else {
            None
        };
        let previous = if c > row.start {
            Some(c - 1)
        } else if closed {
            Some(row.end - 1)
        } else {
            None
        };
        for other in [next, previous].into_iter().flatten() {
            self.segments.push((t.vertices[other], f, polygon));
        }
    }
}

/// Every row of a block's points.
#[derive(Debug, Default)]
pub(super) struct Block {
    /// Per point, the polygons through it.
    pub(super) point_faces: Rows,
    /// Per point, the lines through it, once each.
    pub(super) point_lines: Rows,
    /// Per point, the higher (or equal) ends of its edges: those edges, in
    /// order, are the block's.
    highs: Rows,
    /// Per edge of the block, the polygons with it as a side.
    pub(super) edge_faces: Rows,
    /// Per edge of the block, the lines on it, once each.
    pub(super) edge_lines: Rows,
    /// Per point, the other ends of the edges polygons use.
    pub(super) around: Rows,
    /// Per point, the other ends of the edges only lines use.
    pub(super) around_lines: Rows,
}

impl Block {
    /// The block of `points`, each point's corners gathered by `gather`.
    pub(super) fn build(
        points: Range<usize>,
        corners: &mut Corners,
        mut gather: impl FnMut(usize, &mut Corners),
    ) -> Self {
        let mut block = Self::default();
        for p in points {
            gather(p, corners);
            block.write_point(p as u32, corners);
        }
        block.compact();
        block
    }

    /// The block of `points`: `touched` (ascending) made again as
    /// [`build`](Self::build) makes them, the others' rows `was`'s.
    pub(super) fn patched(
        was: Option<&Block>,
        points: Range<usize>,
        touched: &[u32],
        corners: &mut Corners,
        mut gather: impl FnMut(usize, &mut Corners),
    ) -> Self {
        let mut block = Self::default();
        let mut touched = touched.iter().peekable();
        for p in points {
            if touched.next_if(|&&t| t as usize == p).is_some() {
                gather(p, corners);
                block.write_point(p as u32, corners);
            } else {
                block.copy_point(was, p & (POINTS - 1));
            }
        }
        block.compact();
        block
    }

    /// The next point's rows, from its corners, which it leaves empty.
    fn write_point(&mut self, p: u32, corners: &mut Corners) {
        let Corners {
            faces,
            lines,
            segments,
            above_faces,
            above_lines,
        } = corners;
        faces.sort_unstable();
        lines.sort_unstable();
        lines.dedup();
        self.point_faces.push(faces);
        self.point_lines.push(lines);
        segments.sort_unstable();
        segments.dedup();
        let (mut at, count) = (0, segments.len());
        while at < count {
            let other = segments[at].0;
            above_faces.clear();
            above_lines.clear();
            while at < count && segments[at].0 == other {
                let (_, f, polygon) = segments[at];
                if polygon {
                    above_faces.push(f);
                } else {
                    above_lines.push(f);
                }
                at += 1;
            }
            let around = if above_faces.is_empty() {
                &mut self.around_lines
            } else {
                &mut self.around
            };
            around.entries.push(other);
            if other >= p {
                self.highs.entries.push(other);
                self.edge_faces.push(above_faces);
                self.edge_lines.push(above_lines);
            }
        }
        self.highs.end_row();
        self.around.end_row();
        self.around_lines.end_row();
        faces.clear();
        lines.clear();
        segments.clear();
    }

    /// The next point's rows, those of point `local` of `was`.
    fn copy_point(&mut self, was: Option<&Block>, local: usize) {
        let Some(was) = was else {
            self.point_faces.end_row();
            self.point_lines.end_row();
            self.highs.end_row();
            self.around.end_row();
            self.around_lines.end_row();
            return;
        };
        self.point_faces.push(was.point_faces.row(local));
        self.point_lines.push(was.point_lines.row(local));
        self.around.push(was.around.row(local));
        self.around_lines.push(was.around_lines.row(local));
        let first = was.highs.start(local) as usize;
        let highs = was.highs.row(local);
        self.highs.push(highs);
        for e in first..first + highs.len() {
            self.edge_faces.push(was.edge_faces.row(e));
            self.edge_lines.push(was.edge_lines.row(e));
        }
    }

    fn compact(&mut self) {
        self.point_faces.compact();
        self.point_lines.compact();
        self.highs.compact();
        self.edge_faces.compact();
        self.edge_lines.compact();
        self.around.compact();
        self.around_lines.compact();
    }

    /// How many edges it numbers.
    pub(super) fn edge_count(&self) -> u32 {
        self.highs.entries.len() as u32
    }

    /// The number here of the edge from point `local` to `high`.
    #[inline]
    pub(super) fn edge_at(&self, local: usize, high: u32) -> Option<u32> {
        let at = self.highs.row(local).binary_search(&high).ok()?;
        Some(self.highs.start(local) + at as u32)
    }

    /// Edge `e` of the block, whose first point is `first`.
    pub(super) fn edge(&self, first: usize, e: usize) -> Edge {
        let local = self.highs.offsets.partition_point(|&o| o as usize <= e) - 1;
        Edge {
            points: [(first + local) as u32, self.highs.entries[e]],
        }
    }

    /// Its edges in order, its first point being `first`.
    pub(super) fn edges(&self, first: usize) -> impl Iterator<Item = Edge> + '_ {
        (0..self.highs.offsets.len().saturating_sub(1)).flat_map(move |local| {
            (self.highs.row(local).iter()).map(move |&high| Edge {
                points: [(first + local) as u32, high],
            })
        })
    }

    /// The bytes it holds.
    pub(super) fn bytes(&self) -> usize {
        self.point_faces.bytes()
            + self.point_lines.bytes()
            + self.highs.bytes()
            + self.edge_faces.bytes()
            + self.edge_lines.bytes()
            + self.around.bytes()
            + self.around_lines.bytes()
    }
}
