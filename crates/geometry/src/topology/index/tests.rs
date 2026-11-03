//! The parallel bucket build equals the serial sort build table for table;
//! polylines' edges and adjacency; degenerate primitives refused.
use super::{Adjacency, Index, bucket, sparse};
use crate::topology::Topology;

fn rows(a: &Adjacency) -> (&[u32], &[u32]) {
    (&a.offsets, &a.entries)
}

fn assert_same(topology: &Topology) {
    let parallel: Index = bucket::build(topology).finish(topology);
    let serial: Index = sparse::edges(topology).finish(topology);
    let pairs = |i: &Index| i.edges.iter().map(|e| e.points).collect::<Vec<_>>();
    assert_eq!(pairs(&parallel), pairs(&serial));
    assert_eq!(rows(&parallel.edge_faces), rows(&serial.edge_faces));
    assert_eq!(rows(&parallel.face_edges), rows(&serial.face_edges));
    assert_eq!(rows(&parallel.point_edges), rows(&serial.point_edges));
    assert_eq!(rows(&parallel.point_faces), rows(&serial.point_faces));
    assert_eq!(rows(&parallel.edge_polylines), rows(&serial.edge_polylines));
    assert_eq!(rows(&parallel.polyline_edges), rows(&serial.polyline_edges));
    assert_eq!(
        rows(&parallel.point_polylines),
        rows(&serial.point_polylines)
    );
    assert_eq!(
        rows(&parallel.point_curve_edges),
        rows(&serial.point_curve_edges)
    );
}

/// A deterministic shuffle so point ids do not follow face order.
fn shuffled(n: usize, seed: u64) -> Vec<u32> {
    let mut ids: Vec<u32> = (0..n as u32).collect();
    let mut state = seed;
    for i in (1..n).rev() {
        state = state
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ids.swap(i, (state >> 33) as usize % (i + 1));
    }
    ids
}

fn grid(n: u32, ids: &[u32]) -> (Vec<u32>, Vec<u32>) {
    let mut offsets = vec![0];
    let mut vertices = Vec::new();
    for y in 0..n {
        for x in 0..n {
            let p = |x: u32, y: u32| ids[(y * (n + 1) + x) as usize];
            if (x + y) % 7 == 0 {
                // Two triangles instead of a quad.
                vertices.extend([p(x, y), p(x + 1, y), p(x + 1, y + 1)]);
                offsets.push(vertices.len() as u32);
                vertices.extend([p(x, y), p(x + 1, y + 1), p(x, y + 1)]);
            } else {
                vertices.extend([p(x, y), p(x + 1, y), p(x + 1, y + 1), p(x, y + 1)]);
            }
            offsets.push(vertices.len() as u32);
        }
    }
    (offsets, vertices)
}

#[test]
fn parallel_build_matches_serial_on_large_grids() {
    for (n, seed) in [(3, 0), (64, 1), (200, 2)] {
        let points = ((n + 1) * (n + 1)) as usize;
        for ids in [(0..points as u32).collect(), shuffled(points, seed)] {
            let (offsets, vertices) = grid(n, &ids);
            let polygons = offsets.len() - 1;
            assert_same(&Topology::new(points, offsets.into(), vertices.into(), polygons).unwrap());
        }
    }
}

#[test]
fn parallel_build_matches_serial_on_irregular_topology() {
    // Non-manifold fins, an n-gon, opposite windings, unused points.
    let faces: Vec<Vec<u32>> = vec![
        vec![0, 1, 2],
        vec![1, 0, 3],
        vec![0, 1, 4],
        vec![2, 1, 5, 6, 7, 8],
        vec![8, 7, 9],
        vec![9, 7, 8, 10],
    ];
    let mut offsets = vec![0];
    let mut vertices = Vec::new();
    for f in faces {
        vertices.extend(f);
        offsets.push(vertices.len() as u32);
    }
    assert_same(&Topology::new(14, offsets.into(), vertices.into(), 6).unwrap());
    assert_same(&Topology::new(0, vec![0].into(), Vec::new().into(), 0).unwrap());
}

#[test]
fn polyline_validation_rejects_degenerate_curves() {
    let make = |prims: &[&[u32]], polygons| {
        let mut offsets = vec![0];
        let mut vertices = Vec::new();
        for p in prims {
            vertices.extend_from_slice(p);
            offsets.push(vertices.len() as u32);
        }
        Topology::new(4, offsets.into(), vertices.into(), polygons)
    };
    assert!(make(&[&[0, 1]], 0).is_ok());
    assert!(make(&[&[0, 1, 0]], 0).is_ok(), "a revisit is allowed");
    assert!(make(&[&[0]], 0).is_err(), "one vertex");
    assert!(make(&[&[0, 1, 1, 2]], 0).is_err(), "consecutive repeat");
    assert!(make(&[&[0, 1]], 1).is_err(), "a two-vertex polygon");
    assert!(make(&[&[0, 1, 2], &[2, 9]], 1).is_err(), "missing point");
    assert!(
        make(&[&[0, 1, 2]], 2).is_err(),
        "more polygons than primitives"
    );
}
