//! When most point ids are unused, sorting the few segments avoids
//! scanning point-sized bucket arrays. The order is the bucket build's.
use super::{Edge, NO_EDGE, Tables, Topology, segment};

pub(super) fn edges(topology: &Topology) -> Tables {
    let vertex_count = topology.vertices.len();
    let mut occurrences = Vec::with_capacity(vertex_count);
    let mut prim_of = Vec::with_capacity(vertex_count);
    for prim in 0..topology.primitives() {
        for c in topology.primitive_vertices(prim) {
            prim_of.push(prim as u32);
            if let Some((low, high)) = segment(topology, prim, c) {
                occurrences.push((low, high, prim as u32, c as u32));
            }
        }
    }
    occurrences.sort_unstable();
    let mut tables = Tables {
        edges: Vec::new(),
        offsets: Vec::new(),
        prims: Vec::with_capacity(occurrences.len()),
        vertices: Vec::with_capacity(occurrences.len()),
        vertex_edges: vec![NO_EDGE; vertex_count],
        prim_of,
    };
    tables.offsets.push(0);
    let mut previous = None;
    for (low, high, prim, vertex) in occurrences {
        if previous != Some([low, high]) {
            if !tables.edges.is_empty() {
                tables.offsets.push(tables.prims.len() as u32);
            }
            tables.edges.push(Edge {
                points: [low, high],
            });
            previous = Some([low, high]);
        }
        tables.prims.push(prim);
        tables.vertices.push(vertex);
        tables.vertex_edges[vertex as usize] = (tables.edges.len() - 1) as u32;
    }
    if !tables.edges.is_empty() {
        tables.offsets.push(tables.prims.len() as u32);
    }
    tables
}
