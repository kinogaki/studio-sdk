//! The canonical [`Index`] of a topology: its edges and the adjacency
//! between points, edges and primitives, the same for every kernel and
//! for Studio's display.
//!
//! Both builds list segments: the side leaving each polygon vertex
//! (wrapping to the first) and the segment leaving each polyline vertex
//! but the last. Normal surfaces are counting-sorted by low point in
//! parallel ([`bucket`]); very sparse point domains sort their few
//! segments serially ([`sparse`]). Both give identical tables, and their
//! shared [`Tables::finish`] splits each edge's occurrences into polygon
//! sides and polyline segments.
mod bucket;
mod sparse;
#[cfg(test)]
mod tests;

use super::{Edge, Topology};
use std::ops;

pub(super) fn build(topology: &Topology) -> Index {
    if topology.points > topology.vertices.len().saturating_mul(4) {
        sparse::edges(topology).finish(topology)
    } else {
        bucket::build(topology).finish(topology)
    }
}

/// A vertex that starts no segment: a polyline's last.
const NO_EDGE: u32 = u32::MAX;

/// The segment leaving vertex `c` of primitive `prim`, as (low, high)
/// points.
fn segment(topology: &Topology, prim: usize, c: usize) -> Option<(u32, u32)> {
    let next = if c + 1 < topology.offsets[prim + 1] as usize {
        c + 1
    } else if prim < topology.polygons {
        topology.offsets[prim] as usize
    } else {
        return None;
    };
    let (a, b) = (topology.vertices[c], topology.vertices[next]);
    Some((a.min(b), a.max(b)))
}

/// Rows of ids, one row per point, edge or primitive, stored contiguously.
/// A table with no entries at all stores no rows: every row is empty
/// (the polyline tables of a value without polylines).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Adjacency {
    offsets: Vec<u32>,
    entries: Vec<u32>,
}

impl ops::Index<usize> for Adjacency {
    type Output = [u32];
    #[inline]
    fn index(&self, i: usize) -> &[u32] {
        self.row(i)
    }
}

impl Adjacency {
    /// Rows that are all empty.
    fn none() -> Self {
        Self {
            offsets: Vec::new(),
            entries: Vec::new(),
        }
    }

    /// This table, storing nothing when it has no entries.
    fn compact(self) -> Self {
        if self.entries.is_empty() {
            Self::none()
        } else {
            self
        }
    }

    /// Row `i` (empty in a table storing no rows).
    #[inline]
    pub(crate) fn row(&self, i: usize) -> &[u32] {
        if self.offsets.is_empty() {
            return &[];
        }
        &self.entries[self.offsets[i] as usize..self.offsets[i + 1] as usize]
    }

    /// The rows stored (none for a table without entries).
    pub(crate) fn rows(&self) -> usize {
        self.offsets.len().saturating_sub(1)
    }

    /// Drops repeated entries from each sorted row.
    fn dedup_rows(&mut self) {
        let mut write = 0;
        let mut start = 0;
        for row in 0..self.rows() {
            let end = self.offsets[row + 1] as usize;
            for read in start..end {
                if read == start || self.entries[read] != self.entries[read - 1] {
                    self.entries[write] = self.entries[read];
                    write += 1;
                }
            }
            start = end;
            self.offsets[row + 1] = write as u32;
        }
        self.entries.truncate(write);
    }

    fn bytes(&self) -> usize {
        4 * (self.offsets.capacity() + self.entries.capacity())
    }
}

/// A topology's edges and adjacency ([`Topology::index`]). Rows are
/// ascending unless said otherwise. Polygon ids are primitive ids
/// `0..polygons`; a polyline row `k` is primitive `polygons + k`.
#[derive(Debug)]
pub struct Index {
    edges: Vec<Edge>,
    edge_faces: Adjacency,
    edge_vertices: Vec<u32>,
    edge_polylines: Adjacency,
    face_edges: Adjacency,
    polyline_edges: Adjacency,
    point_edges: Adjacency,
    point_curve_edges: Adjacency,
    point_faces: Adjacency,
    point_polylines: Adjacency,
    polygons: usize,
}

impl Index {
    /// Every polygon side and polyline segment, once, in canonical order
    /// ([`Topology::edges`]). Disjoint point ranges keep their own edges'
    /// order when concatenated.
    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    /// The id of the edge joining `a` and `b`, either way round.
    pub fn edge_of(&self, a: u32, b: u32) -> Option<u32> {
        let key = Edge {
            points: [a.min(b), a.max(b)],
        };
        self.edges.binary_search(&key).ok().map(|e| e as u32)
    }

    /// Per edge, the polygons with it as a side (empty for an edge only
    /// polylines use).
    pub fn edge_faces(&self) -> &Adjacency {
        &self.edge_faces
    }

    /// Edge `e`'s side occurrences: the vertex starting each side, aligned
    /// with [`edge_faces`](Self::edge_faces)' row `e`.
    #[inline]
    pub fn edge_vertices(&self, e: usize) -> &[u32] {
        let rows = &self.edge_faces.offsets;
        &self.edge_vertices[rows[e] as usize..rows[e + 1] as usize]
    }

    /// Per edge, the polylines with a segment on it (primitive ids).
    pub fn edge_polylines(&self) -> &Adjacency {
        &self.edge_polylines
    }

    /// Per polygon, the edge leaving each of its vertices, in vertex order.
    pub fn face_edges(&self) -> &Adjacency {
        &self.face_edges
    }

    /// Per polyline, the edge of each segment, in order: one fewer than
    /// its vertices.
    pub(crate) fn polyline_edges(&self) -> &Adjacency {
        &self.polyline_edges
    }

    /// Per point, the edges polygons use (polyline segments along a
    /// polygon side included).
    pub fn point_edges(&self) -> &Adjacency {
        &self.point_edges
    }

    /// Per point, the polygons through it, each once.
    pub fn point_faces(&self) -> &Adjacency {
        &self.point_faces
    }

    /// Primitive `p`'s edges: a polygon's sides or a polyline's segments.
    pub fn primitive_edges(&self, p: usize) -> &[u32] {
        if p < self.polygons {
            &self.face_edges[p]
        } else {
            &self.polyline_edges[p - self.polygons]
        }
    }

    /// The primitives on edge `e`: polygons, then polylines.
    pub fn edge_primitives(&self, e: usize) -> impl Iterator<Item = u32> + '_ {
        self.edge_faces[e]
            .iter()
            .chain(&self.edge_polylines[e])
            .copied()
    }

    /// The bytes it holds, for memory accounting.
    pub fn bytes(&self) -> usize {
        self.edges.capacity() * 8
            + self.edge_vertices.capacity() * 4
            + self.edge_faces.bytes()
            + self.edge_polylines.bytes()
            + self.face_edges.bytes()
            + self.polyline_edges.bytes()
            + self.point_edges.bytes()
            + self.point_curve_edges.bytes()
            + self.point_faces.bytes()
            + self.point_polylines.bytes()
    }
}

/// Edges and their segment occurrences, before the polygon/polyline split.
struct Tables {
    edges: Vec<Edge>,
    /// Edge `e`'s occurrences are `offsets[e]..offsets[e + 1]`, ascending
    /// by vertex.
    offsets: Vec<u32>,
    /// The primitive and starting vertex of each occurrence.
    prims: Vec<u32>,
    vertices: Vec<u32>,
    /// The edge each vertex starts, or [`NO_EDGE`].
    vertex_edges: Vec<u32>,
    /// The primitive of each vertex.
    prim_of: Vec<u32>,
}

impl Tables {
    fn finish(self, topology: &Topology) -> Index {
        let polygons = topology.polygons;
        let polylines = topology.primitives() - polygons;
        let polygon_vertices = topology.polygon_vertices();
        let edge_count = self.edges.len();
        // Polygon vertices precede polyline vertices, so each edge's polygon
        // sides come first among its occurrences.
        let (edge_faces, edge_vertices, edge_polylines) = if polylines == 0 {
            let faces = Adjacency {
                offsets: self.offsets,
                entries: self.prims,
            };
            (faces, self.vertices, Adjacency::none())
        } else {
            let (faces, vertices, lines) =
                split(&self.offsets, &self.prims, &self.vertices, polygon_vertices);
            (faces, vertices, lines.compact())
        };
        let mut vertex_edges = self.vertex_edges;
        let polyline_offsets: Vec<u32> = topology.offsets[polygons..]
            .iter()
            .enumerate()
            // Each earlier polyline's last vertex starts no segment.
            .map(|(k, &o)| o - polygon_vertices as u32 - k as u32)
            .collect();
        let polyline_entries: Vec<u32> = vertex_edges[polygon_vertices..]
            .iter()
            .copied()
            .filter(|&e| e != NO_EDGE)
            .collect();
        vertex_edges.truncate(polygon_vertices);
        let (edges, prim_of) = (self.edges, self.prim_of);
        // Point rows hold the surface's edges; edges only polylines use are
        // listed apart, so polygon tools see the valence a surface has.
        let curve_only = |e: usize| edge_faces.offsets[e] == edge_faces.offsets[e + 1];
        let point_edges = if polylines == 0 {
            bucket::adjacency(topology.points, edge_count, |e| {
                edges[e].points.map(|p| (p, e as u32))
            })
        } else {
            let surface: Vec<u32> = (0..edge_count as u32)
                .filter(|&e| !curve_only(e as usize))
                .collect();
            bucket::adjacency(topology.points, surface.len(), |i| {
                let e = surface[i];
                edges[e as usize].points.map(|p| (p, e))
            })
        };
        let curve_edges: Vec<u32> = (0..edge_count as u32)
            .filter(|&e| curve_only(e as usize))
            .collect();
        let point_curve_edges = bucket::adjacency(topology.points, curve_edges.len(), |i| {
            let e = curve_edges[i];
            edges[e as usize].points.map(|p| (p, e))
        })
        .compact();
        let point_faces = bucket::adjacency(topology.points, polygon_vertices, |c| {
            [(topology.vertices[c], prim_of[c])]
        });
        let mut point_polylines = bucket::adjacency(
            topology.points,
            topology.vertices.len() - polygon_vertices,
            |c| {
                let c = polygon_vertices + c;
                [(topology.vertices[c], prim_of[c])]
            },
        );
        point_polylines.dedup_rows();
        Index {
            edges,
            edge_vertices,
            edge_faces,
            edge_polylines,
            face_edges: Adjacency {
                offsets: topology.offsets[..=polygons].to_vec(),
                entries: vertex_edges,
            },
            polyline_edges: Adjacency {
                offsets: polyline_offsets,
                entries: polyline_entries,
            }
            .compact(),
            point_edges,
            point_curve_edges,
            point_faces,
            point_polylines: point_polylines.compact(),
            polygons,
        }
    }
}

/// Each edge's occurrences split at `polygon_vertices` into polygon sides
/// (faces and vertices) and polyline segments (primitive ids, once each).
fn split(
    offsets: &[u32],
    prims: &[u32],
    vertices: &[u32],
    polygon_vertices: usize,
) -> (Adjacency, Vec<u32>, Adjacency) {
    let mut faces = Adjacency {
        offsets: Vec::with_capacity(offsets.len()),
        entries: Vec::with_capacity(prims.len()),
    };
    let mut face_vertices = Vec::with_capacity(prims.len());
    let mut lines = Adjacency {
        offsets: Vec::with_capacity(offsets.len()),
        entries: Vec::new(),
    };
    faces.offsets.push(0);
    lines.offsets.push(0);
    for w in offsets.windows(2) {
        let row = w[0] as usize..w[1] as usize;
        let sides =
            row.start + vertices[row.clone()].partition_point(|&c| (c as usize) < polygon_vertices);
        faces.entries.extend_from_slice(&prims[row.start..sides]);
        face_vertices.extend_from_slice(&vertices[row.start..sides]);
        let first = lines.entries.len();
        for &p in &prims[sides..row.end] {
            // A polyline crossing the same edge twice is listed once.
            if !lines.entries[first..].contains(&p) {
                lines.entries.push(p);
            }
        }
        faces.offsets.push(faces.entries.len() as u32);
        lines.offsets.push(lines.entries.len() as u32);
    }
    (faces, face_vertices, lines)
}
