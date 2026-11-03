//! Shading normals: the cusp angle, sharp edges, authored normals first,
//! the cache following N and sharp, and the refresh after a drag matching
//! a whole build.
use kinogaki_geometry::{
    Attribute, Attributes, Domain, Elements, Meaning, Polygons, Primitives, names,
};
use std::f64::consts::{FRAC_PI_2, FRAC_PI_6, PI};
use std::sync::Arc;

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

/// Two triangles folded `angle` apart across the side 0–1.
fn folded(angle: f64) -> Polygons {
    value(
        &[
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, -angle.cos(), angle.sin()],
        ],
        &[&[0, 1, 2], &[1, 0, 3]],
    )
}

/// `v` with every edge in edge group `name` (`all`), or none of them.
fn edges(v: &Polygons, name: &str, all: bool) -> Polygons {
    let count = v.count(Domain::Edge).unwrap();
    let group = if all {
        Elements::from_indices(count, 0..count as u32).unwrap()
    } else {
        Elements::none(count)
    };
    let attributes = v
        .attributes()
        .with_group(Domain::Edge, name, group)
        .unwrap();
    v.with_attributes(attributes).unwrap()
}

fn sharp(v: &Polygons, all: bool) -> Polygons {
    edges(v, names::SHARP, all)
}

fn close(a: [f32; 3], b: [f64; 3]) {
    let d = (0..3)
        .map(|k| (a[k] as f64 - b[k]).powi(2))
        .sum::<f64>()
        .sqrt();
    assert!(d < 1e-6, "{a:?} != {b:?}");
}

#[test]
fn a_smooth_fold_averages_by_angle_and_a_fold_past_the_cusp_stays_hard() {
    let smooth = folded(FRAC_PI_6).shading_normals();
    close(
        smooth.vertices()[0],
        [0.0, (PI / 12.0).sin(), (PI / 12.0).cos()],
    );
    close(smooth.vertices()[2], [0.0, 0.0, 1.0]);
    let hard = folded(FRAC_PI_2).shading_normals();
    close(hard.vertices()[0], [0.0, 0.0, 1.0]);
    // The second triangle's corner at point 0 (vertex 4).
    close(hard.vertices()[4], [0.0, 1.0, 0.0]);
    close(hard.primitives()[1], [0.0, 1.0, 0.0]);
}

#[test]
fn sharp_edges_split_fans_and_authored_normals_come_first() {
    let split = sharp(&folded(FRAC_PI_6), true).shading_normals();
    close(split.vertices()[0], [0.0, 0.0, 1.0]);
    // A sharp group with no members walks fans by the cusp alone.
    let walked = sharp(&folded(FRAC_PI_6), false).shading_normals();
    assert_eq!(
        walked.vertices(),
        folded(FRAC_PI_6).shading_normals().vertices()
    );
    // Authored N on vertices overrides sharp edges and the cusp.
    let marked = sharp(&folded(FRAC_PI_6), true);
    let n = Attribute::tuples(vec![[1.0f32, 0.0, 0.0]; 6], Meaning::Normal).unwrap();
    let authored = marked
        .with_attributes(
            marked
                .attributes()
                .with(Domain::Vertex, names::N, n)
                .unwrap(),
        )
        .unwrap();
    assert!(
        authored
            .shading_normals()
            .vertices()
            .iter()
            .all(|&n| n == [1.0, 0.0, 0.0])
    );
    // Vertex N before Point N; a zero-length N takes the polygon's normal.
    let point = Attribute::tuples(vec![[0.0f32, 1.0, 0.0]; 4], Meaning::Normal).unwrap();
    let both = authored
        .with_attributes(
            authored
                .attributes()
                .with(Domain::Point, names::N, point.clone())
                .unwrap(),
        )
        .unwrap();
    assert!(
        both.shading_normals()
            .vertices()
            .iter()
            .all(|&n| n == [1.0, 0.0, 0.0])
    );
    let zero = Attribute::tuples(vec![[0.0f32; 3]; 4], Meaning::Normal).unwrap();
    let flat = folded(FRAC_PI_2);
    let flat = flat
        .with_attributes(
            flat.attributes()
                .with(Domain::Point, names::N, zero)
                .unwrap(),
        )
        .unwrap();
    close(flat.shading_normals().vertices()[4], [0.0, 1.0, 0.0]);
}

/// A fan bending away from the vertex's own polygon: each polygon within
/// the cusp of the next, the last past the cusp of the first. A corner of
/// the first averages its own polygon and the next alone, with an empty
/// sharp group as without one.
#[test]
fn one_rule_with_or_without_an_empty_group() {
    let ring = |k: usize, z: f64| {
        let t = k as f64 * PI / 3.0;
        [t.cos(), t.sin(), z]
    };
    let v = value(
        &[
            [0.0; 3],
            ring(0, 0.0),
            ring(1, 0.0),
            ring(2, 1.0),
            ring(3, 2.5),
        ],
        &[&[0, 1, 2], &[0, 2, 3], &[0, 3, 4]],
    );
    let faces = v.shading_normals().primitives().to_vec();
    let dot = |a: [f32; 3], b: [f32; 3]| (0..3).map(|k| a[k] * b[k]).sum::<f32>();
    assert!(dot(faces[0], faces[1]) > 0.5 && dot(faces[1], faces[2]) > 0.5);
    assert!(
        dot(faces[0], faces[2]) < 0.5,
        "the last past the first's cusp"
    );
    let plain = v.shading_normals();
    let grouped = sharp(&v, false).shading_normals();
    assert_eq!(
        plain.vertices(),
        grouped.vertices(),
        "an empty group changes nothing"
    );
    // The first polygon's corner at the centre: its own and the next, by
    // their angles there, not the last.
    let points = [ring(0, 0.0), ring(1, 0.0), ring(2, 1.0)];
    let angle = |a: [f64; 3], b: [f64; 3]| {
        let d = |x: [f64; 3], y: [f64; 3]| (0..3).map(|k| x[k] * y[k]).sum::<f64>();
        (d(a, b) / (d(a, a) * d(b, b)).sqrt()).acos()
    };
    let weights = [angle(points[0], points[1]), angle(points[1], points[2])];
    let mean: [f64; 3] = std::array::from_fn(|k| {
        weights[0] * f64::from(faces[0][k]) + weights[1] * f64::from(faces[1][k])
    });
    let length = mean.iter().map(|c| c * c).sum::<f64>().sqrt();
    close(plain.vertices()[0], mean.map(|c| c / length));
}

#[test]
fn fans_never_join_across_an_edge_of_three_polygons() {
    let v = value(
        &[
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [-1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
        ],
        &[&[0, 1, 2], &[0, 3, 4], &[1, 0, 5], &[0, 1, 3]],
    );
    let normals = sharp(&v, false).shading_normals();
    // Edge 0–1 has three polygons: point 1's corner of the first keeps its own.
    close(normals.vertices()[1], [0.0, 0.0, 1.0]);
    assert!(
        normals
            .vertices()
            .iter()
            .all(|n| n.iter().all(|c| c.is_finite()))
    );
}

#[test]
fn normals_are_kept_and_follow_n_and_sharp_without_new_positions() {
    let v = folded(FRAC_PI_6);
    let n = v.shading_normals();
    assert!(Arc::ptr_eq(&n, &v.shading_normals()));
    // The same positions with sharp edges: other normals, not the kept ones.
    let marked = sharp(&v, true);
    assert!(marked.positions().ptr_eq(v.positions()));
    assert_ne!(marked.shading_normals().vertices(), n.vertices());
    assert!(Arc::ptr_eq(&n, &v.shading_normals()), "both kept");
    // A group of a few edges of many (held as a list) is kept by identity
    // too.
    let g = grid(10);
    let one = Elements::from_indices(g.count(Domain::Edge).unwrap(), [0]).unwrap();
    let attributes = g.attributes().with_group(Domain::Edge, names::SHARP, one);
    let few = g.with_attributes(attributes.unwrap()).unwrap();
    assert!(Arc::ptr_eq(&few.shading_normals(), &few.shading_normals()));
}

/// A bumpy grid of `n` × `n` quads.
fn grid(n: usize) -> Polygons {
    let side = n + 1;
    let positions: Vec<[f64; 3]> = (0..side * side)
        .map(|i| {
            let (x, z) = ((i % side) as f64, (i / side) as f64);
            [x, (x * 0.7).sin() * (z * 0.4).cos(), z]
        })
        .collect();
    let rings: Vec<Vec<u32>> = (0..n * n)
        .map(|q| {
            let (x, z) = (q % n, q / n);
            let at = |dx: usize, dz: usize| ((z + dz) * side + x + dx) as u32;
            vec![at(0, 0), at(0, 1), at(1, 1), at(1, 0)]
        })
        .collect();
    let refs: Vec<&[u32]> = rings.iter().map(Vec::as_slice).collect();
    value(&positions, &refs)
}

fn next(state: &mut u64) -> u64 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    *state
}

#[test]
fn the_refresh_after_a_drag_matches_a_whole_build() {
    for with_sharp in [false, true] {
        let mut before = grid(100);
        if with_sharp {
            let edges = before.count(Domain::Edge).unwrap();
            let group = Elements::from_indices(edges, (0..edges as u32).step_by(7)).unwrap();
            let smooth = Elements::from_indices(edges, (3..edges as u32).step_by(5)).unwrap();
            let attributes = before.attributes();
            let attributes = attributes
                .with_group(Domain::Edge, names::SHARP, group)
                .unwrap();
            let attributes = attributes
                .with_group(Domain::Edge, names::SMOOTH, smooth)
                .unwrap();
            before = before.with_attributes(attributes).unwrap();
        }
        let mut state = 0x2545_f491_4f6c_dd1d;
        for round in 0..15 {
            let mut positions = before.positions().to_vec();
            let mut moved = Vec::new();
            for _ in 0..1 + next(&mut state) % 30 {
                let p = (next(&mut state) % positions.len() as u64) as usize;
                positions[p][1] += (next(&mut state) % 1000) as f64 / 500.0 - 1.0;
                moved.push(p as u32);
            }
            let after = before.with_positions(positions.clone().into()).unwrap();
            before.shading_normals();
            let refreshed = after.shading_normals_after(&before, &moved);
            // The same positions in a column of their own: built whole.
            let fresh = after
                .with_positions(positions.into())
                .unwrap()
                .shading_normals();
            assert_eq!(*refreshed, *fresh, "round {round}, sharp {with_sharp}");
            before = after;
        }
    }
}

#[test]
fn smooth_edges_join_past_the_cusp_and_sharp_edges_win() {
    // A right-angle fold joined by smooth edges averages both faces.
    let soft = edges(&folded(FRAC_PI_2), names::SMOOTH, true).shading_normals();
    let half = std::f64::consts::FRAC_1_SQRT_2;
    close(soft.vertices()[0], [0.0, half, half]);
    // An edge both sharp and smooth is sharp.
    let both = sharp(&edges(&folded(FRAC_PI_2), names::SMOOTH, true), true).shading_normals();
    close(both.vertices()[0], [0.0, 0.0, 1.0]);
    // A smooth group with no members leaves the cusp rule.
    let none = edges(&folded(FRAC_PI_2), names::SMOOTH, false).shading_normals();
    close(none.vertices()[0], [0.0, 0.0, 1.0]);
    // Only an edge of exactly two polygons joins: 0–1 has three.
    let v = value(
        &[
            [0.0; 3],
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [-1.0, 0.0, 0.0],
            [0.0, -1.0, 0.0],
        ],
        &[&[0, 1, 2], &[0, 3, 4], &[1, 0, 5], &[0, 1, 3]],
    );
    close(
        edges(&v, names::SMOOTH, true).shading_normals().vertices()[1],
        [0.0, 0.0, 1.0],
    );
    // The kept normals follow the smooth group.
    let plain = folded(FRAC_PI_2);
    let joined = edges(&plain, names::SMOOTH, true);
    assert!(joined.positions().ptr_eq(plain.positions()));
    assert_ne!(
        joined.shading_normals().vertices(),
        plain.shading_normals().vertices()
    );
}

#[test]
fn a_primitive_normal_is_each_primitives_own() {
    // A polygon's corners read the Primitive `N` at its own index.
    let v = folded(FRAC_PI_2);
    let ups = vec![[0.0f32, 0.0, 1.0], [1.0, 0.0, 0.0]];
    let ups = Attribute::tuples(ups, Meaning::Normal).unwrap();
    let a = v
        .attributes()
        .with(Domain::Primitive, names::N, ups)
        .unwrap();
    let n = v.with_attributes(a).unwrap().shading_normals();
    for corner in 0..3 {
        close(n.vertices()[corner], [0.0, 0.0, 1.0]);
        close(n.vertices()[3 + corner], [1.0, 0.0, 0.0]);
    }
    // One value for two primitives is refused, not read.
    let one = Attribute::tuples(vec![[0.0f32, 0.0, 1.0]], Meaning::Normal).unwrap();
    let short = v
        .attributes()
        .with(Domain::Primitive, names::N, one)
        .unwrap();
    assert!(v.with_attributes(short).is_err());
}
