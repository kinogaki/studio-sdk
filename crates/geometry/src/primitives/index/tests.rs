//! The index equals its definition, worked out the plain way from the rows:
//! whole, and patched after any edit a table writer makes. Lines' edges and
//! adjacency in any primitive order; degenerate primitives refused.
use crate::primitives::{Kind, Primitives};
use std::collections::{BTreeMap, BTreeSet};

/// Every row of `t`'s index against the definition.
fn assert_same(t: &Primitives) {
    let index = t.index();
    let mut edges: BTreeMap<[u32; 2], (BTreeSet<u32>, BTreeSet<u32>)> = BTreeMap::new();
    let mut faces: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    let mut lines: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    let mut around: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    let mut around_lines: BTreeMap<u32, BTreeSet<u32>> = BTreeMap::new();
    for p in 0..t.len() {
        let row = t.primitive(p);
        let kind = t.kind(p);
        let polygon = kind.is_polygon();
        let segments = if kind.closed() {
            row.len()
        } else {
            row.len() - 1
        };
        for &q in row {
            (if polygon { &mut faces } else { &mut lines })
                .entry(q)
                .or_default()
                .insert(p as u32);
        }
        for k in 0..segments {
            let (a, b) = (row[k], row[(k + 1) % row.len()]);
            let edge = edges.entry([a.min(b), a.max(b)]).or_default();
            (if polygon { &mut edge.0 } else { &mut edge.1 }).insert(p as u32);
        }
    }
    for (&[a, b], (polygons, _)) in &edges {
        let rows = if polygons.is_empty() {
            &mut around_lines
        } else {
            &mut around
        };
        rows.entry(a).or_default().insert(b);
        rows.entry(b).or_default().insert(a);
    }
    let got: Vec<[u32; 2]> = index.edges().map(|e| e.points).collect();
    assert_eq!(got, edges.keys().copied().collect::<Vec<_>>());
    assert_eq!(index.edge_count(), edges.len());
    for (e, (&[a, b], (polygons, on))) in edges.iter().enumerate() {
        assert_eq!(index.edge(e).points, [a, b]);
        assert_eq!(index.edge_of(b, a), Some(e as u32));
        assert_eq!(
            index.edge_faces(e),
            polygons.iter().copied().collect::<Vec<_>>()
        );
        assert_eq!(index.edge_lines(e), on.iter().copied().collect::<Vec<_>>());
        assert_eq!(index.faces_between(a, b), index.edge_faces(e));
    }
    let list = |rows: &BTreeMap<u32, BTreeSet<u32>>, p: u32| -> Vec<u32> {
        rows.get(&p).into_iter().flatten().copied().collect()
    };
    for p in 0..t.points() as u32 {
        assert_eq!(index.point_faces(p as usize), list(&faces, p));
        let through: Vec<u32> = index.point_primitives(p as usize).collect();
        let want: Vec<u32> = list(&faces, p).into_iter().chain(list(&lines, p)).collect();
        assert_eq!(through, want);
        let neighbours: Vec<u32> = index.point_neighbours(p as usize).collect();
        let want: Vec<u32> = list(&around, p)
            .into_iter()
            .chain(list(&around_lines, p))
            .collect();
        assert_eq!(neighbours, want);
        let at: Vec<u32> = index.point_every_edge(p as usize).collect();
        assert_eq!(
            at,
            want.iter()
                .map(|&q| index.edge_of(p, q).unwrap())
                .collect::<Vec<_>>()
        );
    }
    for p in 0..t.len() {
        let row = t.primitive(p);
        let closed = t.kind(p).closed();
        let segments = if closed { row.len() } else { row.len() - 1 };
        let want: Vec<u32> = (0..segments)
            .map(|k| index.edge_of(row[k], row[(k + 1) % row.len()]).unwrap())
            .collect();
        assert_eq!(index.primitive_edges(p).collect::<Vec<_>>(), want);
    }
    let boundary: Vec<u32> = (edges.values().enumerate())
        .filter(|(_, (polygons, _))| polygons.len() == 1)
        .map(|(e, _)| e as u32)
        .collect();
    assert_eq!(index.boundary_edges(), boundary);
}

fn table(points: usize, prims: &[&[u32]], kinds: &[Kind]) -> Result<Primitives, crate::Error> {
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
fn the_index_is_its_definition_on_grids() {
    for (n, seed) in [(3, 0), (64, 1), (200, 2)] {
        let points = ((n + 1) * (n + 1)) as usize;
        for ids in [(0..points as u32).collect(), shuffled(points, seed)] {
            let (offsets, vertices) = grid(n, &ids);
            assert_same(&Primitives::of_polygons(points, offsets.into(), vertices.into()).unwrap());
        }
    }
}

#[test]
fn the_index_is_its_definition_on_irregular_topology() {
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
        (index.edge_faces(side), index.edge_lines(side)),
        (&[1u32][..], &[0u32][..])
    );
    // The polyline's two segments, the quad's four sides.
    assert_eq!(index.primitive_edges(0).len(), 2);
    assert_eq!(index.primitive_edges(1).len(), 4);
    let off = t.edge_of(2, 6).unwrap() as usize;
    assert!(index.edge_faces(off).is_empty());
    assert_eq!(t.polygons().iter().collect::<Vec<_>>(), [1, 3]);
    assert_eq!(t.edge_count(), t.edges().count());
}

/// A random table's row: a polygon or a polyline over `points`.
fn row(next: &mut impl FnMut(usize) -> usize, points: usize) -> (Kind, Vec<u32>) {
    let kind = if next(4) == 0 {
        Kind::POLYLINE
    } else {
        Kind::POLYGON
    };
    let mut corners = Vec::new();
    while corners.len() < 3 + next(3) {
        let p = next(points) as u32;
        if !corners.contains(&p) {
            corners.push(p);
        }
    }
    (kind, corners)
}

/// However rows are rewritten, added and dropped from the end, over more
/// points or not, a patched index is the whole one.
#[test]
fn a_patched_index_is_the_whole_one() {
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = move |below: usize| {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        (seed % below.max(1) as u64) as usize
    };
    // Big enough to span several blocks, its rows few enough to check.
    let mut points = 3000;
    let mut offsets = vec![0u32];
    let mut vertices = Vec::new();
    let mut kinds = Vec::new();
    for _ in 0..1500 {
        let (kind, corners) = row(&mut next, points);
        vertices.extend(corners);
        offsets.push(vertices.len() as u32);
        kinds.push(kind);
    }
    let mut table =
        Primitives::new(points, offsets.into(), vertices.into(), kinds.into(), None).unwrap();
    table.index();
    for _ in 0..40 {
        points += next(50);
        let mut writer = table.rewrite(points);
        let (mut at, keep) = (0, table.len() - next(3));
        for _ in 0..next(6) {
            let to = at + next(keep - at);
            writer.keep(at..to);
            if to < keep {
                let (kind, corners) = row(&mut next, points);
                writer.write(kind, &corners).unwrap();
                at = to + 1;
            } else {
                at = to;
            }
        }
        writer.keep(at..keep);
        for _ in 0..next(5) {
            let (kind, corners) = row(&mut next, points);
            writer.write(kind, &corners).unwrap();
        }
        let edited = writer.finish(None).unwrap();
        assert!(edited.index.get().is_some(), "patched, not left to build");
        assert_same(&edited);
        table = edited;
    }
}

/// A table with a row taken from the middle renumbers the rows after it: its
/// index is built when asked, as what it is.
#[test]
fn a_renumbered_table_builds_its_index_when_asked() {
    let t = table(
        5,
        &[&[0, 1, 2], &[1, 2, 3], &[2, 3, 4]],
        &[Kind::POLYGON; 3],
    )
    .unwrap();
    t.index();
    let mut writer = t.rewrite(5);
    writer.keep(0..1);
    writer.keep(2..3);
    let edited = writer.finish(None).unwrap();
    assert!(edited.index.get().is_none());
    assert_same(&edited);
}

#[test]
fn more_points_share_the_index_and_answer_for_the_new_ones() {
    let t = table(5, &[&[0, 1, 2], &[1, 2, 3]], &[Kind::POLYGON; 2]).unwrap();
    t.index();
    let more = t.with_points(5000).unwrap();
    assert_same(&more);
    assert_ne!(more.fingerprint(), t.fingerprint());
}
