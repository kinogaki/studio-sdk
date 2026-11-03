//! Canonical connectivity built in parallel blocks.
//!
//! Segments are counting-sorted by their low point: atomic counters size
//! and fill each point's row, then every row is sorted by (high point,
//! vertex). Vertex ids are unique, so the sorted rows, and every table
//! built from them, are identical to a serial build whatever the thread
//! timing. Point adjacency rows are filled the same way and sorted
//! ascending, which is the order a serial pass over edges and faces gives.
use super::{Adjacency, Edge, NO_EDGE, Primitives, Tables, segment};
use crate::parallel;
use std::ops::Range;
use std::sync::atomic::{AtomicU32, AtomicU64, Ordering::Relaxed};

/// Vertices or points per parallel block.
const BLOCK: usize = 16_384;

pub(super) fn build(t: &Primitives) -> Tables {
    let points = t.points;
    let vertex_count = t.vertices.len();
    let prim_of = prim_of_vertices(t);
    // The segment leaving vertex `c`, as (low point, high point).
    let low_high = |c: usize| segment(t, prim_of[c] as usize, c);

    // Segments per low point, then (high << 32 | vertex) placed in rows.
    let counts = atomics(points);
    each(vertex_count, |c| {
        if let Some((low, _)) = low_high(c) {
            counts[low as usize].fetch_add(1, Relaxed);
        }
    });
    let starts = prefix(counts.into_iter().map(AtomicU32::into_inner));
    let occurrence_count = starts[points] as usize;
    let cursor: Vec<AtomicU32> = starts[..points]
        .iter()
        .map(|&s| AtomicU32::new(s))
        .collect();
    let slots: Vec<AtomicU64> = (0..occurrence_count).map(|_| AtomicU64::new(0)).collect();
    each(vertex_count, |c| {
        if let Some((low, high)) = low_high(c) {
            let at = cursor[low as usize].fetch_add(1, Relaxed) as usize;
            slots[at].store(u64::from(high) << 32 | c as u64, Relaxed);
        }
    });
    drop(cursor);
    let mut occurrences: Vec<u64> = slots.into_iter().map(AtomicU64::into_inner).collect();

    // Sort each row; count its distinct high points (its edges).
    let mut row_edges = vec![0u32; points];
    {
        let blocks: Vec<_> = split_rows(&mut occurrences, &starts)
            .into_iter()
            .zip(row_edges.chunks_mut(BLOCK))
            .collect();
        parallel::each_block(blocks, |(rows, counts)| {
            for (row, count) in rows.into_iter().zip(counts.iter_mut()) {
                row.sort_unstable();
                *count = (0..row.len())
                    .filter(|&i| i == 0 || row[i] >> 32 != row[i - 1] >> 32)
                    .count() as u32;
            }
        });
    }
    let first_edge = prefix(row_edges.into_iter());
    let edge_count = first_edge[points] as usize;

    // Edges, and where each edge's occurrences start.
    let mut edges = vec![Edge::default(); edge_count];
    let mut offsets = vec![0u32; edge_count + 1];
    offsets[edge_count] = occurrence_count as u32;
    {
        let ranges = || point_blocks(points).map(|r| edge_range(&first_edge, &r));
        let blocks: Vec<_> = split(&mut edges, ranges())
            .into_iter()
            .zip(split(&mut offsets[..edge_count], ranges()))
            .zip(point_blocks(points))
            .collect();
        parallel::each_block(blocks, |((edges, offsets), range)| {
            let mut e = 0;
            for p in range {
                let row = starts[p] as usize..starts[p + 1] as usize;
                for at in row.clone() {
                    let high = (occurrences[at] >> 32) as u32;
                    if at == row.start || high != (occurrences[at - 1] >> 32) as u32 {
                        edges[e] = Edge {
                            points: [p as u32, high],
                        };
                        offsets[e] = at as u32;
                        e += 1;
                    }
                }
            }
        });
    }

    // Occurrence vertices and primitives, and each vertex's edge.
    let mut edge_vertices = vec![0u32; occurrence_count];
    parallel::chunks(&mut edge_vertices, BLOCK, |first, out| {
        for (i, vertex) in out.iter_mut().enumerate() {
            *vertex = occurrences[first + i] as u32;
        }
    });
    drop(occurrences);
    let mut edge_prims = vec![0u32; occurrence_count];
    parallel::chunks(&mut edge_prims, BLOCK, |first, out| {
        for (i, prim) in out.iter_mut().enumerate() {
            *prim = prim_of[edge_vertices[first + i] as usize];
        }
    });
    let mut vertex_edges = vec![0u32; vertex_count];
    parallel::chunks(&mut vertex_edges, BLOCK, |first, out| {
        for (i, edge) in out.iter_mut().enumerate() {
            *edge = match low_high(first + i) {
                Some((low, high)) => {
                    let row =
                        first_edge[low as usize] as usize..first_edge[low as usize + 1] as usize;
                    let at = edges[row.clone()].partition_point(|e| e.points[1] < high);
                    (row.start + at) as u32
                }
                None => NO_EDGE,
            };
        }
    });
    Tables {
        edges,
        offsets,
        prims: edge_prims,
        vertices: edge_vertices,
        vertex_edges,
        prim_of,
    }
}

/// The primitive of every vertex.
fn prim_of_vertices(t: &Primitives) -> Vec<u32> {
    let mut prim_of = vec![0u32; t.vertices.len()];
    let offsets = &t.offsets;
    parallel::chunks(&mut prim_of, BLOCK, |first, out| {
        let mut f = offsets.partition_point(|&o| o as usize <= first) - 1;
        for (i, prim) in out.iter_mut().enumerate() {
            while offsets[f + 1] as usize <= first + i {
                f += 1;
            }
            *prim = f as u32;
        }
    });
    prim_of
}

/// `rows` adjacency rows filled from `pairs(item)` (row, value) for every
/// item, each row ascending.
pub(super) fn adjacency<const N: usize>(
    rows: usize,
    items: usize,
    pairs: impl Fn(usize) -> [(u32, u32); N] + Sync,
) -> Adjacency {
    let counts = atomics(rows);
    each(items, |i| {
        for (row, _) in pairs(i) {
            counts[row as usize].fetch_add(1, Relaxed);
        }
    });
    let offsets = prefix(counts.into_iter().map(AtomicU32::into_inner));
    let cursor: Vec<AtomicU32> = offsets[..rows].iter().map(|&s| AtomicU32::new(s)).collect();
    let slots = atomics(offsets[rows] as usize);
    each(items, |i| {
        for (row, value) in pairs(i) {
            let at = cursor[row as usize].fetch_add(1, Relaxed) as usize;
            slots[at].store(value, Relaxed);
        }
    });
    drop(cursor);
    let mut entries: Vec<u32> = slots.into_iter().map(AtomicU32::into_inner).collect();
    let blocks = split_rows(&mut entries, &offsets);
    parallel::each_block(blocks, |rows| {
        for row in rows {
            row.sort_unstable();
        }
    });
    Adjacency { offsets, entries }
}

fn atomics(len: usize) -> Vec<AtomicU32> {
    (0..len).map(|_| AtomicU32::new(0)).collect()
}

/// Running sums starting at zero, with the total last.
fn prefix(counts: impl Iterator<Item = u32>) -> Vec<u32> {
    let mut sums = Vec::with_capacity(counts.size_hint().0 + 1);
    let mut total = 0u32;
    sums.push(0);
    for count in counts {
        total += count;
        sums.push(total);
    }
    sums
}

/// `work(i)` for every `i` in `0..len`, in parallel blocks.
#[track_caller]
fn each(len: usize, work: impl Fn(usize) + Sync) {
    parallel::each(len, BLOCK, work);
}

/// Consecutive point ranges of [`BLOCK`] points.
fn point_blocks(points: usize) -> impl Iterator<Item = Range<usize>> {
    (0..points)
        .step_by(BLOCK)
        .map(move |start| start..(start + BLOCK).min(points))
}

fn edge_range(first_edge: &[u32], points: &Range<usize>) -> Range<usize> {
    first_edge[points.start] as usize..first_edge[points.end] as usize
}

/// `values` cut into the consecutive `ranges` that tile it.
fn split<T>(mut values: &mut [T], ranges: impl Iterator<Item = Range<usize>>) -> Vec<&mut [T]> {
    let mut out = Vec::new();
    let mut at = 0;
    for range in ranges {
        debug_assert_eq!(range.start, at);
        let (head, tail) = std::mem::take(&mut values).split_at_mut(range.end - at);
        out.push(head);
        values = tail;
        at = range.end;
    }
    out
}

/// Per block of points, that block's rows (row `p` is
/// `offsets[p]..offsets[p + 1]`).
fn split_rows<'a, T>(values: &'a mut [T], offsets: &[u32]) -> Vec<Vec<&'a mut [T]>> {
    let points = offsets.len() - 1;
    let ranges = point_blocks(points).map(|r| offsets[r.start] as usize..offsets[r.end] as usize);
    split(values, ranges)
        .into_iter()
        .zip(point_blocks(points))
        .map(|(block, range)| {
            let base = offsets[range.start] as usize;
            split(
                block,
                range.map(|p| offsets[p] as usize - base..offsets[p + 1] as usize - base),
            )
        })
        .collect()
}

/// How many unique edges: the segments' high points counted per low
/// point and deduplicated row by row, in parallel, with none of the
/// build's other tables.
pub(super) fn count(t: &Primitives) -> usize {
    let points = t.points;
    let prim_of = prim_of_vertices(t);
    let low_high = |c: usize| segment(t, prim_of[c] as usize, c);
    let counts = atomics(points);
    each(t.vertices.len(), |c| {
        if let Some((low, _)) = low_high(c) {
            counts[low as usize].fetch_add(1, Relaxed);
        }
    });
    let starts = prefix(counts.into_iter().map(AtomicU32::into_inner));
    let cursor: Vec<AtomicU32> = starts[..points]
        .iter()
        .map(|&s| AtomicU32::new(s))
        .collect();
    let slots = atomics(starts[points] as usize);
    each(t.vertices.len(), |c| {
        if let Some((low, high)) = low_high(c) {
            let at = cursor[low as usize].fetch_add(1, Relaxed) as usize;
            slots[at].store(high, Relaxed);
        }
    });
    drop(cursor);
    let mut highs: Vec<u32> = slots.into_iter().map(AtomicU32::into_inner).collect();
    let mut row_edges = vec![0u32; points];
    let blocks: Vec<_> = split_rows(&mut highs, &starts)
        .into_iter()
        .zip(row_edges.chunks_mut(BLOCK))
        .collect();
    parallel::each_block(blocks, |(rows, counts)| {
        for (row, count) in rows.into_iter().zip(counts.iter_mut()) {
            row.sort_unstable();
            *count = (0..row.len())
                .filter(|&i| i == 0 || row[i] != row[i - 1])
                .count() as u32;
        }
    });
    row_edges.iter().map(|&n| n as usize).sum()
}
