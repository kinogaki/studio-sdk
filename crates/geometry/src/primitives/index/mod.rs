//! The canonical [`Index`] of a primitive table: its edges and the
//! adjacency between points, edges and primitives, the same for every
//! kernel and for Studio's display.
//!
//! Everything is kept per point, in shared [`Block`]s of [`POINTS`] points
//! (`block.rs`), and every row of a point is made from the corners on that
//! point, one way ([`Corners`]). So a whole build makes every block in
//! parallel, and an edit makes the blocks of the points whose rows changed
//! and shares the rest ([`patched`]): an edge's number is the count of edges
//! before it, kept per block, never stored in a row.
mod block;
#[cfg(test)]
mod tests;
mod walk;

pub use walk::Walk;

use super::{Edge, Kinds, Primitives};
use crate::column::Column;
use crate::parallel;
use block::{Block, Corners, POINTS, SHIFT};
use std::ops::Range;
use std::sync::atomic::{AtomicU32, Ordering::Relaxed};
use std::sync::{Arc, OnceLock};

/// Vertices per parallel block of the whole build.
const VERTICES: usize = 16_384;

/// A table's edges and adjacency ([`Primitives::index`]), the one place
/// topology is asked: which elements touch which, a point's neighbours,
/// [edge loops and rings](Self::edge_loop) and
/// [boundaries](Self::boundary_edges). Built once when first asked, or
/// from the index of the table an edit came from at the cost of the rows it
/// changed, and shared by every value holding the table, so a question
/// costs what it reads and nothing per frame. Rows are ascending; primitives
/// are named by their ids. "Lines" are the primitives that aren't polygons:
/// polylines and curves (by their hulls).
///
/// ```
/// use kinogaki_geometry::Primitives;
///
/// // Two quads side by side: 0 1 4 3 and 1 2 5 4.
/// let t = Primitives::of_polygons(6, vec![0, 4, 8].into(), vec![0, 1, 4, 3, 1, 2, 5, 4].into()).unwrap();
/// let index = t.index();
/// assert_eq!(index.point_primitives(1).collect::<Vec<_>>(), [0, 1]);
/// assert_eq!(index.point_neighbours(1).collect::<Vec<_>>(), [0, 2, 4]);
/// let shared = t.edge_of(1, 4).unwrap() as usize;
/// assert_eq!(index.edge_primitives(shared).collect::<Vec<_>>(), [0, 1]);
/// assert!(!index.is_boundary(shared) && index.boundary_edges().len() == 6);
/// ```
#[derive(Debug)]
pub struct Index {
    blocks: Vec<Arc<Block>>,
    /// How many edges come before each block, and their count last.
    first_edge: Vec<u32>,
    /// The table's rows, for a primitive's edges.
    offsets: Column<u32>,
    vertices: Column<u32>,
    kinds: Kinds,
    boundary: OnceLock<Vec<u32>>,
}

impl Index {
    fn of(t: &Primitives, blocks: Vec<Arc<Block>>) -> Self {
        let mut first_edge = Vec::with_capacity(blocks.len() + 1);
        let mut total = 0u32;
        first_edge.push(0);
        for block in &blocks {
            total += block.edge_count();
            first_edge.push(total);
        }
        Self {
            blocks,
            first_edge,
            offsets: t.offsets.clone(),
            vertices: t.vertices.clone(),
            kinds: t.kinds.clone(),
            boundary: OnceLock::new(),
        }
    }

    /// Point `p`'s block and its place in it.
    #[inline]
    fn point(&self, p: usize) -> (&Block, usize) {
        match self.blocks.get(p >> SHIFT) {
            Some(block) => (block, p & (POINTS - 1)),
            None => (&block::EMPTY, 0),
        }
    }

    /// Edge `e`'s block and its number within it.
    #[inline]
    fn edge_place(&self, e: usize) -> (&Block, usize, usize) {
        let e = e as u32;
        let b = self.first_edge.partition_point(|&first| first <= e) - 1;
        (&self.blocks[b], b, (e - self.first_edge[b]) as usize)
    }

    /// How many edges.
    pub fn edge_count(&self) -> usize {
        self.first_edge.last().map_or(0, |&total| total as usize)
    }

    /// Every primitive's segments, once each, in canonical order
    /// ([`Primitives::edges`]).
    pub fn edges(&self) -> impl Iterator<Item = Edge> + '_ {
        (self.blocks.iter().enumerate()).flat_map(|(b, block)| block.edges(b << SHIFT))
    }

    /// Edge `e`.
    ///
    /// # Panics
    /// When `e` is not an edge.
    pub fn edge(&self, e: usize) -> Edge {
        let (block, b, local) = self.edge_place(e);
        block.edge(b << SHIFT, local)
    }

    /// The id of the edge joining `a` and `b`, either way round.
    pub fn edge_of(&self, a: u32, b: u32) -> Option<u32> {
        let (low, high) = (a.min(b), a.max(b));
        let (block, local) = self.point(low as usize);
        let first = self.first_edge.get(low as usize >> SHIFT)?;
        Some(first + block.edge_at(local, high)?)
    }

    /// The polygons with `e` as a side (empty for an edge only lines use).
    pub fn edge_faces(&self, e: usize) -> &[u32] {
        let (block, _, local) = self.edge_place(e);
        block.edge_faces.row(local)
    }

    /// The lines with a segment on `e`, once each.
    pub fn edge_lines(&self, e: usize) -> &[u32] {
        let (block, _, local) = self.edge_place(e);
        block.edge_lines.row(local)
    }

    /// The polygons with the edge between points `a` and `b` as a side, at
    /// the cost of finding it from its points; empty without such an edge.
    pub(crate) fn faces_between(&self, a: u32, b: u32) -> &[u32] {
        let (low, high) = (a.min(b), a.max(b));
        let (block, local) = self.point(low as usize);
        match block.edge_at(local, high) {
            Some(edge) => block.edge_faces.row(edge as usize),
            None => &[],
        }
    }

    /// Primitive `p`'s edges, in vertex order: the edge leaving each vertex
    /// (a closed primitive's last to its first; an open one has one fewer
    /// than its vertices).
    pub fn primitive_edges(&self, p: usize) -> impl ExactSizeIterator<Item = u32> + '_ {
        let (first, end) = (self.offsets[p] as usize, self.offsets[p + 1] as usize);
        let count = end - first;
        let kind = match &self.kinds {
            Kinds::All(kind) => *kind,
            Kinds::Each(kinds) => kinds[p],
        };
        let closed = kind.closed();
        let segments = if closed {
            count
        } else {
            count.saturating_sub(1)
        };
        (0..segments).map(move |k| {
            let (a, b) = (
                self.vertices[first + k],
                self.vertices[first + (k + 1) % count],
            );
            self.edge_of(a, b).expect("a segment is an edge")
        })
    }

    /// The polygons through point `p`, each once.
    pub fn point_faces(&self, p: usize) -> &[u32] {
        let (block, local) = self.point(p);
        block.point_faces.row(local)
    }

    /// The edges at point `p` that polygons use (line segments along a
    /// polygon side included).
    pub fn point_edges(&self, p: usize) -> impl Iterator<Item = u32> + '_ {
        let (block, local) = self.point(p);
        self.edges_to(p, block.around.row(local))
    }

    fn edges_to<'a>(&'a self, p: usize, others: &'a [u32]) -> impl Iterator<Item = u32> + 'a {
        (others.iter()).map(move |&q| self.edge_of(p as u32, q).expect("a neighbour's edge"))
    }

    /// The primitives on edge `e`: polygons, then lines.
    pub fn edge_primitives(&self, e: usize) -> impl Iterator<Item = u32> + '_ {
        let (block, _, local) = self.edge_place(e);
        (block.edge_faces.row(local).iter())
            .chain(block.edge_lines.row(local))
            .copied()
    }

    /// The primitives through point `p`: polygons, then lines.
    pub fn point_primitives(&self, p: usize) -> impl Iterator<Item = u32> + '_ {
        let (block, local) = self.point(p);
        (block.point_faces.row(local).iter())
            .chain(block.point_lines.row(local))
            .copied()
    }

    /// Every edge at point `p`: those polygons use, then those only lines
    /// use.
    pub fn point_every_edge(&self, p: usize) -> impl Iterator<Item = u32> + '_ {
        let (block, local) = self.point(p);
        self.edges_to(p, block.around.row(local))
            .chain(self.edges_to(p, block.around_lines.row(local)))
    }

    /// The points one edge from point `p`, each once, in the order of
    /// [`point_every_edge`](Self::point_every_edge).
    pub fn point_neighbours(&self, p: usize) -> impl Iterator<Item = u32> + '_ {
        let (block, local) = self.point(p);
        (block.around.row(local).iter())
            .chain(block.around_lines.row(local))
            .copied()
    }

    /// Whether edge `e` is a side of exactly one polygon: an edge of the
    /// surface's border.
    pub fn is_boundary(&self, e: usize) -> bool {
        self.edge_faces(e).len() == 1
    }

    /// Every [boundary](Self::is_boundary) edge, ascending. Found once.
    pub fn boundary_edges(&self) -> &[u32] {
        self.boundary.get_or_init(|| {
            let mut boundary = Vec::new();
            for (block, &first) in self.blocks.iter().zip(&self.first_edge) {
                let edges = block.edge_count();
                boundary.extend(
                    (0..edges)
                        .filter(|&e| block.edge_faces.row(e as usize).len() == 1)
                        .map(|e| first + e),
                );
            }
            boundary
        })
    }

    /// Walks this index's edges in order against `base`'s: `each` gets runs
    /// of `base`'s numbers for edges both have, and each edge only this one
    /// has. Blocks they share cost nothing per edge.
    pub fn compare_edges(&self, base: &Index, mut each: impl FnMut(Result<Range<u32>, Edge>)) {
        for (b, block) in self.blocks.iter().enumerate() {
            let first = base.first_edge.get(b).copied().unwrap_or(0);
            let was = base.blocks.get(b);
            if was.is_some_and(|was| Arc::ptr_eq(was, block)) {
                each(Ok(first..first + block.edge_count()));
                continue;
            }
            for edge in block.edges(b << SHIFT) {
                let [low, high] = edge.points;
                let found = was.and_then(|was| was.edge_at(low as usize & (POINTS - 1), high));
                each(found.map(|e| first + e..first + e + 1).ok_or(edge));
            }
        }
    }

    /// The bytes it holds, for memory accounting: those of a block an edit
    /// shares with another index count in both.
    pub fn bytes(&self) -> usize {
        self.boundary.get().map_or(0, |b| b.capacity() * 4)
            + self.first_edge.capacity() * 4
            + self.blocks.iter().map(|b| b.bytes()).sum::<usize>()
    }
}

/// The whole index of `t`: each block made from the corners on its points.
pub(super) fn build(t: &Primitives) -> Index {
    let vertex_count = t.vertices.len();
    let mut primitive_of = vec![0u32; vertex_count];
    let offsets = &t.offsets;
    parallel::chunks(&mut primitive_of, VERTICES, |first, out| {
        let mut f = offsets.partition_point(|&o| o as usize <= first) - 1;
        for (i, primitive) in out.iter_mut().enumerate() {
            while offsets[f + 1] as usize <= first + i {
                f += 1;
            }
            *primitive = f as u32;
        }
    });
    // Each point's corners, in order.
    let counts = atomics(t.points);
    parallel::each(vertex_count, VERTICES, |c| {
        counts[t.vertices[c] as usize].fetch_add(1, Relaxed);
    });
    let mut starts = Vec::with_capacity(t.points + 1);
    let mut total = 0u32;
    starts.push(0);
    for count in counts {
        total += count.into_inner();
        starts.push(total);
    }
    let cursor: Vec<AtomicU32> = starts[..t.points]
        .iter()
        .map(|&s| AtomicU32::new(s))
        .collect();
    let slots = atomics(vertex_count);
    parallel::each(vertex_count, VERTICES, |c| {
        let at = cursor[t.vertices[c] as usize].fetch_add(1, Relaxed);
        slots[at as usize].store(c as u32, Relaxed);
    });
    let blocks = parallel::map(t.points.div_ceil(POINTS), 1, |range| {
        let mut corners = Corners::default();
        range
            .map(|b| {
                let points = b << SHIFT..((b + 1) << SHIFT).min(t.points);
                Arc::new(Block::build(points, &mut corners, |p, corners| {
                    for slot in &slots[starts[p] as usize..starts[p + 1] as usize] {
                        let c = slot.load(Relaxed) as usize;
                        corners.add(t, primitive_of[c] as usize, c);
                    }
                }))
            })
            .collect::<Vec<_>>()
    });
    Index::of(t, blocks.into_iter().flatten().collect())
}

/// `len` counters of zero.
fn atomics(len: usize) -> Vec<AtomicU32> {
    (0..len).map(|_| AtomicU32::new(0)).collect()
}

/// `index` over `t`, a table with the rows of `index`'s table `base` but for
/// `changed` (ascending; rows of `t`, of `base` or of both, whose corners
/// differ, and the rest keep their numbers): the blocks of the points those
/// rows name are made again, the others shared.
pub(super) fn patched(index: &Index, base: &Primitives, t: &Primitives, changed: &[u32]) -> Index {
    let was = |r: u32| (r as usize) < base.len();
    let is = |r: u32| (r as usize) < t.len();
    // Each new row's corners by their points, and every point a row names.
    let mut named: Vec<(u32, u32, u32)> = Vec::new();
    let mut touched: Vec<u32> = Vec::new();
    for &row in changed {
        if was(row) {
            touched.extend_from_slice(base.primitive(row as usize));
        }
        if is(row) {
            let corners = t.primitive_vertices(row as usize);
            named.extend((corners.start..corners.end).map(|c| (t.vertices[c], row, c as u32)));
        }
    }
    named.sort_unstable();
    touched.extend(named.iter().map(|n| n.0));
    touched.sort_unstable();
    touched.dedup();
    let mut blocks = index.blocks.clone();
    blocks.resize(t.points.div_ceil(POINTS), Arc::default());
    let mut corners = Corners::default();
    let mut at = 0;
    while at < touched.len() {
        let b = touched[at] as usize >> SHIFT;
        let end = at + touched[at..].partition_point(|&p| p as usize >> SHIFT == b);
        let points = b << SHIFT..((b + 1) << SHIFT).min(t.points);
        let inside = &touched[at..end];
        let was = index.blocks.get(b).map(Arc::as_ref);
        blocks[b] = Arc::new(Block::patched(
            was,
            points,
            inside,
            &mut corners,
            |p, corners| {
                // The rows that stay, then the new ones naming it.
                let (old, local) = index.point(p);
                let kept = (old.point_faces.row(local).iter())
                    .chain(old.point_lines.row(local))
                    .filter(|f| changed.binary_search(f).is_err());
                for &f in kept {
                    let f = f as usize;
                    for c in t.primitive_vertices(f) {
                        if t.vertices[c] as usize == p {
                            corners.add(t, f, c);
                        }
                    }
                }
                let new = named.partition_point(|n| (n.0 as usize) < p);
                for &(_, row, c) in named[new..].iter().take_while(|n| n.0 as usize == p) {
                    corners.add(t, row as usize, c as usize);
                }
            },
        ));
        at = end;
    }
    Index::of(t, blocks)
}

/// `index` over `t`, a table of the same rows over more points.
pub(super) fn with_points(index: &Index, t: &Primitives) -> Index {
    let mut blocks = index.blocks.clone();
    blocks.resize(t.points.div_ceil(POINTS), Arc::default());
    Index::of(t, blocks)
}
