//! Polygons as triangles: the display triangulation (exact outlines, the
//! tolerant clip of cut outlines, the fan for damaged ones, n − 2 always),
//! and the cache and its refresh after a drag matching a whole build.
use kinogaki_geometry::{Attributes, Kind, Polygons, Primitives};
use std::sync::Arc;

/// One value of `polygons` (each a ring of point ids) over `positions`.
fn value(positions: &[[f64; 3]], polygons: &[&[u32]]) -> Polygons {
    let mut offsets = vec![0u32];
    let mut vertices = Vec::new();
    for p in polygons {
        vertices.extend_from_slice(p);
        offsets.push(vertices.len() as u32);
    }
    let topology =
        Primitives::of_polygons(positions.len(), offsets.into(), vertices.into()).unwrap();
    Polygons::new(positions.to_vec().into(), topology, Attributes::new()).unwrap()
}

/// Twice the area the triangles of polygon `p` cover, seen along y.
fn area_along_y(v: &Polygons, p: usize) -> f64 {
    let t = v.triangulation();
    t.of(p)
        .map(|i| {
            let [a, b, c] = t.triangles()[i].map(|q| v.positions()[q as usize]);
            let (u, w) = ([b[0] - a[0], b[2] - a[2]], [c[0] - a[0], c[2] - a[2]]);
            (u[0] * w[1] - u[1] * w[0]).abs()
        })
        .sum()
}

#[test]
fn damaged_polygons_show_as_a_fan() {
    let x = |s: f64| [s, 0.0, 0.0];
    for positions in [
        // Collinear.
        vec![x(0.0), x(1.0), x(2.0), x(3.0)],
        // Crossing.
        vec![[0.0; 3], [3.0, 2.0, 0.0], [0.0, 2.0, 0.0], [2.0, 0.0, 0.0]],
        // Collapsed to one place.
        vec![[0.0; 3]; 4],
    ] {
        let v = value(&positions, &[&[0, 1, 2, 3]]);
        let t = v.triangulation();
        assert_eq!(t.corners().as_slice(), [[0, 1, 2], [0, 2, 3]]);
        assert_eq!(t.triangles().as_slice(), [[0, 1, 2], [0, 2, 3]]);
        assert_eq!(t.fans().as_slice(), [0]);
    }
}

#[test]
fn exact_polygons_keep_their_vertex_ids() {
    // A triangle, then one whose first and second points are one place.
    let v = value(
        &[[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0; 3]],
        &[&[0, 1, 2], &[0, 3, 1]],
    );
    let t = v.triangulation();
    assert_eq!(t.fans().as_slice(), [1]);
    assert_eq!(t.corners()[1], [3, 4, 5]);
    assert_eq!(t.triangles()[1], [0, 3, 1]);
    assert_eq!(t.primitives().as_slice(), [0, 1]);
}

#[test]
fn concave_outlines_with_coincident_and_collinear_points_triangulate_exactly() {
    // A box Union's top face: an L with a run of points on one straight
    // side, two of them a rounding step apart (a cut's constructed points).
    let ring = [
        [-1.0, 2.0, -1.0],
        [-1.0, 2.0, 1.0],
        [-0.3, 2.0, 1.0],
        [-0.30000000000000004, 2.0, -0.30000000000000004],
        [-0.3, 2.0, -0.3],
        [-0.30000000000000004, 2.0, -0.7],
        [1.0, 2.0, -0.7],
        [1.0, 2.0, -1.0],
    ];
    let all: Vec<u32> = (0..8).collect();
    let v = value(&ring, &[&all]);
    assert!(v.triangulation().fans().is_empty());
    assert_eq!(v.triangulation().len(), 6);
    // The L: 2 × 2 less the 1.3 × 1.7 corner, twice over.
    let area = area_along_y(&v, 0);
    assert!((area - 2.0 * (4.0 - 1.3 * 1.7)).abs() < 1e-9, "{area}");
    // An L with midpoints on its straight sides.
    let l = [
        (0.0, 0.0),
        (1.0, 0.0),
        (2.0, 0.0),
        (2.0, 1.0),
        (1.0, 1.0),
        (1.0, 2.0),
        (0.0, 2.0),
        (0.0, 1.0),
    ]
    .map(|(x, z)| [x, 0.0, z]);
    let v = value(&l, &[&all]);
    assert!(v.triangulation().fans().is_empty());
    assert!((area_along_y(&v, 0) - 6.0).abs() < 1e-12);
    // A crossing outline is still not a polygon.
    let bowtie = [(0.0, 0.0), (1.0, 1.0), (1.0, 0.0), (0.0, 1.0)].map(|(x, z)| [x, 0.0, z]);
    let v = value(&bowtie, &[&[0, 1, 2, 3]]);
    assert_eq!(v.triangulation().fans().as_slice(), [0]);
}

/// The normal of triangle `t` at `positions`.
fn triangle_normal(positions: &[[f64; 3]], t: [u32; 3]) -> [f64; 3] {
    let [a, b, c] = t.map(|p| positions[p as usize]);
    let (u, w) = (
        [b[0] - a[0], b[1] - a[1], b[2] - a[2]],
        [c[0] - a[0], c[1] - a[1], c[2] - a[2]],
    );
    [
        u[1] * w[2] - u[2] * w[1],
        u[2] * w[0] - u[0] * w[2],
        u[0] * w[1] - u[1] * w[0],
    ]
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

#[test]
fn triangles_are_n_minus_two_and_wind_with_the_polygon() {
    // A concave pentagon (an arrow) in the xy plane, wound both ways.
    let arrow = [
        [0.0, 0.0, 0.0],
        [2.0, 0.0, 0.0],
        [2.0, 2.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 2.0, 0.0],
    ];
    for (ring, z) in [([0u32, 1, 2, 3, 4], 1.0), ([4, 3, 2, 1, 0], -1.0)] {
        let v = value(&arrow, &[&ring]);
        let t = v.triangulation();
        assert_eq!(t.len(), 3);
        assert!(t.fans().is_empty());
        assert!(
            t.triangles()
                .iter()
                .all(|&points| dot(triangle_normal(&arrow, points), [0.0, 0.0, z]) > 0.0)
        );
        // Corners are global vertex ids, read back to the same points.
        for (k, corners) in t.corners().iter().enumerate() {
            assert_eq!(
                t.triangles()[k],
                corners.map(|c| v.primitives().vertices()[c as usize])
            );
        }
    }
    // A polyline has no triangles.
    let line = vec![Kind::POLYLINE].into();
    let topology = Primitives::new(2, vec![0, 2].into(), vec![0, 1].into(), line, None).unwrap();
    let line = Polygons::new(
        vec![[0.0; 3], [1.0, 0.0, 0.0]].into(),
        topology,
        Attributes::new(),
    )
    .unwrap();
    assert!(line.triangulation().is_empty());
    assert_eq!(line.triangulation().of(0), 0..0);
}

#[test]
fn a_skew_outline_that_collapses_in_every_axis_plane_clips_in_its_own_plane() {
    // A cube with its (1, 1, 1) corner dissolved: the six points around it.
    let ring = [
        [1.0, 0.0, 0.0],
        [1.0, 0.0, 1.0],
        [0.0, 0.0, 1.0],
        [0.0, 1.0, 1.0],
        [0.0, 1.0, 0.0],
        [1.0, 1.0, 0.0],
    ];
    let v = value(&ring, &[&[0, 1, 2, 3, 4, 5]]);
    assert_eq!(v.triangulation().len(), 4);
    assert!(v.triangulation().fans().is_empty());
}

/// A grid of `n` × `n` quads in the xz plane, every fifth one dented
/// (concave), as one value.
fn grid(n: usize) -> Polygons {
    let side = n + 1;
    let mut positions: Vec<[f64; 3]> = (0..side * side)
        .map(|i| [(i % side) as f64, 0.0, (i / side) as f64])
        .collect();
    let mut rings = Vec::new();
    for z in 0..n {
        for x in 0..n {
            let at = |dx: usize, dz: usize| ((z + dz) * side + x + dx) as u32;
            let mut ring = vec![at(0, 0), at(0, 1), at(1, 1), at(1, 0)];
            if (z * n + x).is_multiple_of(5) {
                // A dent: a point inside the quad's side, pulled in.
                positions.push([x as f64 + 0.5, 0.0, z as f64 + 0.4]);
                ring.push(positions.len() as u32 - 1);
            }
            rings.push(ring);
        }
    }
    let refs: Vec<&[u32]> = rings.iter().map(Vec::as_slice).collect();
    value(&positions, &refs)
}

/// A small deterministic generator.
fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

#[test]
fn the_triangulation_is_kept_and_its_refresh_after_a_drag_matches_a_whole_build() {
    let v = grid(120);
    let t = v.triangulation();
    assert!(Arc::ptr_eq(&t, &v.triangulation()), "kept on the positions");
    let polygons = v.primitives().polygons().len();
    assert_eq!(t.len(), v.primitives().vertex_count() - 2 * polygons);
    for p in [0, 1, 5, polygons - 1] {
        let n = v.primitives().primitive(p).len();
        assert_eq!(t.of(p).len(), n - 2);
        assert!(t.of(p).all(|i| t.primitives()[i] == p as u32));
    }
    // Every polygon clips: no fans.
    assert!(t.fans().is_empty());
    let mut state = 0x9e37_79b9_7f4a_7c15;
    let mut before = v;
    let mut fanned = 0;
    for round in 0..20 {
        let mut positions = before.positions().to_vec();
        let mut moved = Vec::new();
        for _ in 0..1 + next(&mut state) % 40 {
            let p = (next(&mut state) % positions.len() as u64) as usize;
            // Large moves fold some polygons, so fans come and go.
            let d = (next(&mut state) % 2000) as f64 / 1000.0 - 1.0;
            positions[p][(round % 2) * 2] += d * 1.5;
            moved.push(p as u32);
        }
        let after = before.with_positions(positions.clone().into()).unwrap();
        before.triangulation();
        let refreshed = after.triangulation_after(&before, &moved);
        // The same positions in a column of their own: built whole.
        let fresh = after
            .with_positions(positions.into())
            .unwrap()
            .triangulation();
        fanned += fresh.fans().len();
        assert_eq!(*refreshed, *fresh, "round {round}");
        before = after;
    }
    assert!(fanned > 0, "the drags folded some polygons");
}

#[test]
fn a_drag_that_flips_no_diagonal_keeps_the_triangle_columns() {
    let before = grid(8);
    let t = before.triangulation();
    let mut positions = before.positions().to_vec();
    // A small lift of one interior point: every polygon keeps its triangles.
    positions[10][1] += 0.01;
    let after = before.with_positions(positions.into()).unwrap();
    let moved = after.triangulation_after(&before, &[10]);
    assert!(moved.triangles().ptr_eq(t.triangles()));
    assert!(moved.corners().ptr_eq(t.corners()));
    // Folding a polygon changes its triangles, and so the column.
    let mut positions = after.positions().to_vec();
    positions[10][0] += 3.0;
    let folded = after.with_positions(positions.into()).unwrap();
    let refolded = folded.triangulation_after(&after, &[10]);
    assert!(!refolded.triangles().ptr_eq(t.triangles()));
}
