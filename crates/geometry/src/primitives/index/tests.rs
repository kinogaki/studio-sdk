//! The parallel bucket build equals the serial sort build table for table,
//! and the edge count both; lines' edges and adjacency in any primitive
//! order; degenerate primitives refused.
use super::{Adjacency, Index, bucket, count, sparse};
use crate::primitives::{Kind, Primitives};

fn rows(a: &Adjacency) -> (&[u32], &[u32]) {
    (&a.offsets, &a.entries)
}

fn assert_same(t: &Primitives) {
    let parallel: Index = bucket::build(t).finish(t);
    let serial: Index = sparse::edges(t).finish(t);
    let pairs = |i: &Index| i.edges.iter().map(|e| e.points).collect::<Vec<_>>();
    assert_eq!(pairs(&parallel), pairs(&serial));
    assert_eq!(
        (bucket::count(t), count(t)),
        (serial.edges.len(), serial.edges.len())
    );
    assert_eq!(rows(&parallel.edge_faces), rows(&serial.edge_faces));
    assert_eq!(
        rows(&parallel.primitive_edges),
        rows(&serial.primitive_edges)
    );
    assert_eq!(rows(&parallel.point_edges), rows(&serial.point_edges));
    assert_eq!(rows(&parallel.point_faces), rows(&serial.point_faces));
    assert_eq!(rows(&parallel.edge_lines), rows(&serial.edge_lines));
    assert_eq!(rows(&parallel.point_lines), rows(&serial.point_lines));
    assert_eq!(
        rows(&parallel.point_line_edges),
        rows(&serial.point_line_edges)
    );
}

fn table(
    points: usize,
    prims: &[&[u32]],
    kinds: &[Kind],
) -> Result<Primitives, crate::GeometryError> {
    let mut offsets = vec![0];
    let mut vertices = Vec::new();
    for p in prims {
        vertices.extend_from_slice(p);
        offsets.push(vertices.len() as u32);
    }
    Primitives::new(
        points,
        offsets.into(),
        vertices.into(),
        kinds.to_vec().into(),
        None,
    )
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
            assert_same(&Primitives::of_polygons(points, offsets.into(), vertices.into()).unwrap());
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
    assert_same(&Primitives::of_polygons(14, offsets.into(), vertices.into()).unwrap());
    assert_same(&Primitives::of_polygons(0, vec![0].into(), Vec::new().into()).unwrap());
}

#[test]
fn polyline_validation_rejects_degenerate_curves() {
    use Kind as K;
    let (line, polygon) = (K::POLYLINE, K::POLYGON);
    assert!(table(4, &[&[0, 1]], &[line]).is_ok());
    assert!(
        table(4, &[&[0, 1, 0]], &[line]).is_ok(),
        "a revisit is allowed"
    );
    assert!(table(4, &[&[0]], &[line]).is_err(), "one vertex");
    assert!(
        table(4, &[&[0, 1, 1, 2]], &[line]).is_err(),
        "consecutive repeat"
    );
    assert!(
        table(4, &[&[0, 1]], &[polygon]).is_err(),
        "a two-vertex polygon"
    );
    let missing = table(4, &[&[0, 1, 2], &[2, 9]], &[polygon, line]).unwrap_err();
    assert_eq!(missing.message(), "polyline 1 references a missing point");
    assert!(
        table(4, &[&[0, 1, 2]], &[polygon, polygon]).is_err(),
        "more kinds than primitives"
    );
}

#[test]
fn lines_before_and_between_polygons_split_their_edges_alike() {
    use Kind as K;
    // A polyline along the quad's side and off its corner, the quad, a
    // closed polyline-free ring as a second quad, in mixed order.
    let prims: [&[u32]; 4] = [&[3, 2, 6], &[0, 1, 2, 3], &[6, 7], &[4, 5, 7, 6]];
    let kinds = [K::POLYLINE, K::POLYGON, K::POLYLINE, K::POLYGON];
    let t = table(8, &prims, &kinds).unwrap();
    assert_same(&t);
    let index = t.index();
    let side = t.edge_of(2, 3).unwrap() as usize;
    assert_eq!(
        (&index.edge_faces()[side], &index.edge_lines()[side]),
        (&[1u32][..], &[0u32][..])
    );
    // The polyline's two segments, the quad's four sides.
    assert_eq!(index.primitive_edges(0).len(), 2);
    assert_eq!(index.primitive_edges(1).len(), 4);
    let off = t.edge_of(2, 6).unwrap() as usize;
    assert!(index.edge_faces()[off].is_empty());
    assert_eq!(t.polygons().iter().collect::<Vec<_>>(), [1, 3]);
    assert_eq!(t.edge_count(), t.edges().len());
}
