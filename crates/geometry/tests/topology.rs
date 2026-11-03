//! Topology questions over the table's one index: what touches what,
//! neighbours, edge loops and rings, boundaries and connected components,
//! on a grid, a closed band, a pole, a non-manifold fin and polylines.
use kinogaki_geometry::{Attributes, Geometry, Index, Kind, Primitives, Walk};
use std::sync::Arc;

/// `n` × `m` quads: point `(i, j)` is `j * (n + 1) + i`.
fn grid(n: u32, m: u32) -> Primitives {
    let at = |i: u32, j: u32| j * (n + 1) + i;
    let mut corners = Vec::new();
    for j in 0..m {
        for i in 0..n {
            corners.extend([at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    let offsets: Vec<u32> = (0..=n * m).map(|f| 4 * f).collect();
    Primitives::of_polygons(((n + 1) * (m + 1)) as usize, offsets.into(), corners.into()).unwrap()
}

/// A band of `around` quads closed in `i`, `m` high: point `(i, j)` is
/// `j * around + i`.
fn band(around: u32, m: u32) -> Primitives {
    let at = |i: u32, j: u32| j * around + (i % around);
    let mut corners = Vec::new();
    for j in 0..m {
        for i in 0..around {
            corners.extend([at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    let offsets: Vec<u32> = (0..=around * m).map(|f| 4 * f).collect();
    Primitives::of_polygons((around * (m + 1)) as usize, offsets.into(), corners.into()).unwrap()
}

/// The walk's edges as point pairs.
fn pairs(t: &Primitives, walk: &Walk) -> Vec<[u32; 2]> {
    walk.edges
        .iter()
        .map(|&e| t.edge(e as usize).points)
        .collect()
}

fn edge(t: &Primitives, a: u32, b: u32) -> u32 {
    t.edge_of(a, b).expect("an edge")
}

/// Every element's rows agree with the table's own rows, both ways.
fn rows_agree(t: &Primitives, index: &Index) {
    for p in 0..t.points() {
        let through: Vec<u32> = index.point_primitives(p).collect();
        let want: Vec<u32> = (0..t.len() as u32)
            .filter(|&f| t.primitive(f as usize).contains(&(p as u32)))
            .collect();
        let mut sorted = through.clone();
        sorted.sort_unstable();
        assert_eq!(sorted, want, "the primitives through {p}");
        for e in index.point_every_edge(p) {
            assert!(t.edge(e as usize).points.contains(&(p as u32)));
        }
        let mut near: Vec<u32> = index.point_neighbours(p).collect();
        near.sort_unstable();
        let mut want: Vec<u32> = t
            .edges()
            .filter(|e| e.points.contains(&(p as u32)))
            .map(|e| e.points[usize::from(e.points[0] == p as u32)])
            .collect();
        want.sort_unstable();
        assert_eq!(near, want, "the neighbours of {p}");
    }
    for e in 0..t.edge_count() {
        for f in index.edge_primitives(e) {
            assert!(
                index
                    .primitive_edges(f as usize)
                    .any(|side| side == e as u32)
            );
        }
    }
}

#[test]
fn what_touches_what_agrees_with_the_table_on_a_mixed_value() {
    // Two quads, a polyline leaving a corner along an edge of its own and
    // along a quad's side, and a loose point 8.
    let kinds = vec![Kind::POLYGON, Kind::POLYGON, Kind::POLYLINE];
    let t = Primitives::new(
        9,
        vec![0, 4, 8, 11].into(),
        vec![0, 1, 4, 3, 1, 2, 5, 4, 1, 0, 7].into(),
        kinds.into(),
        None,
    )
    .unwrap();
    let index = t.index();
    rows_agree(&t, index);
    assert_eq!(index.point_primitives(0).collect::<Vec<_>>(), [0, 2]);
    assert_eq!(index.point_neighbours(7).collect::<Vec<_>>(), [0]);
    assert_eq!(index.point_primitives(8).count(), 0);
    assert_eq!(index.point_neighbours(8).count(), 0);
    let tail = edge(&t, 0, 7);
    assert_eq!(
        index.edge_primitives(tail as usize).collect::<Vec<_>>(),
        [2]
    );
    assert!(
        !index.is_boundary(tail as usize),
        "a line's edge no surface's"
    );
    rows_agree(&grid(3, 2), grid(3, 2).index());
}

#[test]
fn an_edge_loop_crosses_regular_points_and_stops_at_the_border() {
    let t = grid(4, 3);
    let index = t.index();
    // The second row of points runs straight from border to border.
    let at = |i: u32, j: u32| j * 5 + i;
    let walk = index.edge_loop(edge(&t, at(1, 1), at(2, 1)));
    assert!(!walk.closed);
    let want: Vec<[u32; 2]> = (0..4).map(|i| [at(i, 1), at(i + 1, 1)]).collect();
    assert_eq!(pairs(&t, &walk), want);
    // Any edge of it gives the same loop.
    let other = index.edge_loop(edge(&t, at(3, 1), at(4, 1)));
    assert_eq!(other, walk);
    // A column.
    let walk = index.edge_loop(edge(&t, at(2, 1), at(2, 2)));
    assert_eq!(walk.edges.len(), 3);
    assert!(!walk.closed);
}

#[test]
fn a_border_edge_loop_goes_round_the_border() {
    let t = grid(4, 3);
    let index = t.index();
    let walk = index.edge_loop(edge(&t, 0, 1));
    assert!(walk.closed);
    assert_eq!(walk.edges.len(), 2 * (4 + 3));
    let mut sorted = walk.edges.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, index.boundary_edges());
}

#[test]
fn a_loop_round_a_closed_band_closes_and_a_loop_along_it_stops_at_its_rims() {
    let t = band(6, 3);
    let index = t.index();
    let around = index.edge_loop(edge(&t, 6, 7));
    assert!(around.closed);
    assert_eq!(around.edges.len(), 6);
    let along = index.edge_loop(edge(&t, 7, 13));
    assert!(!along.closed);
    assert_eq!(along.edges.len(), 3);
    // A rim is a boundary loop of its own.
    let rim = index.edge_loop(edge(&t, 0, 1));
    assert!(rim.closed);
    assert_eq!(rim.edges.len(), 6);
    assert_eq!(index.boundary_edges().len(), 12);
}

#[test]
fn a_ring_crosses_quads_and_closes_round_a_band() {
    let t = band(6, 3);
    let index = t.index();
    // The rungs between the rows 0 and 1, all the way round.
    let ring = index.edge_ring(edge(&t, 2, 8));
    assert!(ring.closed);
    let mut rungs = pairs(&t, &ring);
    rungs.sort_unstable();
    assert_eq!(rungs, (0..6).map(|i| [i, i + 6]).collect::<Vec<_>>());
    // Across the band it stops at both rims.
    let across = index.edge_ring(edge(&t, 7, 8));
    assert!(!across.closed);
    assert_eq!(
        pairs(&t, &across),
        [[1, 2], [7, 8], [13, 14], [19, 20]],
        "from one rim to the other, in order"
    );
}

#[test]
fn walks_stop_where_the_surface_stops_being_regular() {
    // A quad strip (0 1 4 3)(1 2 5 4) then a triangle on 2–5.
    let t = Primitives::of_polygons(
        7,
        vec![0, 4, 8, 11].into(),
        vec![0, 1, 4, 3, 1, 2, 5, 4, 2, 6, 5].into(),
    )
    .unwrap();
    let index = t.index();
    let ring = index.edge_ring(edge(&t, 1, 4));
    assert_eq!(
        pairs(&t, &ring),
        [[0, 3], [1, 4], [2, 5]],
        "the triangle has no opposite side"
    );

    // A pole: five quads round a point. A spoke stops at it.
    let mut corners = Vec::new();
    for i in 0..5u32 {
        corners.extend([0, 1 + 2 * i, 2 + 2 * i, 1 + (2 * i + 2) % 10]);
    }
    let offsets: Vec<u32> = (0..=5).map(|f| 4 * f).collect();
    let pole = Primitives::of_polygons(11, offsets.into(), corners.into()).unwrap();
    let walk = pole.index().edge_loop(edge(&pole, 0, 1));
    assert_eq!(walk.edges, [edge(&pole, 0, 1)]);
    assert!(!walk.closed);

    // A fin: three quads on one edge. It has no way on, in loops or rings.
    let fin = Primitives::of_polygons(
        8,
        vec![0, 4, 8, 12].into(),
        vec![0, 1, 2, 3, 0, 1, 4, 5, 0, 1, 6, 7].into(),
    )
    .unwrap();
    let spine = edge(&fin, 0, 1);
    assert_eq!(fin.index().edge_loop(spine).edges, [spine]);
    assert_eq!(fin.index().edge_ring(spine).edges, [spine]);
    assert!(!fin.index().is_boundary(spine as usize));
}

#[test]
fn a_loop_follows_a_polyline_and_stops_at_its_branches() {
    // 0–1–2–3 open, closed 4–5–6, and 7–8 meeting the first at point 2.
    let kinds = vec![Kind::POLYLINE; 3];
    let branched = Primitives::new(
        9,
        vec![0, 4, 8, 10].into(),
        vec![0, 1, 2, 3, 4, 5, 6, 4, 2, 8].into(),
        kinds.into(),
        None,
    )
    .unwrap();
    let index = branched.index();
    let up_to_branch = index.edge_loop(edge(&branched, 0, 1));
    assert_eq!(pairs(&branched, &up_to_branch), [[0, 1], [1, 2]]);
    let round = index.edge_loop(edge(&branched, 4, 5));
    assert!(round.closed, "a line coming back to its start: {round:?}");
    assert_eq!(round.edges.len(), 3);
    assert!(index.edge_ring(edge(&branched, 4, 5)).edges.len() == 1);
    assert!(index.boundary_edges().is_empty());
}

#[test]
fn components_number_pieces_by_their_smallest_point_and_are_kept_on_the_table() {
    let kinds = vec![Kind::POLYGON, Kind::POLYLINE, Kind::POLYGON];
    // The triangle 4 5 6, a line 6–7 on it, the triangle 0 1 2; 3 and 8
    // are loose.
    let t = Primitives::new(
        9,
        vec![0, 3, 5, 8].into(),
        vec![4, 5, 6, 6, 7, 0, 1, 2].into(),
        kinds.into(),
        None,
    )
    .unwrap();
    let pieces = t.components();
    assert_eq!(pieces.count(), 4);
    assert_eq!(pieces.of_points(), [0, 0, 0, 1, 2, 2, 2, 2, 3]);
    let joined = pieces.joined([(3, 8), (0, 7)]);
    assert_eq!(joined.count(), 2);
    assert_eq!(joined.of_points(), [0, 0, 0, 1, 0, 0, 0, 0, 1]);
    assert_eq!(Primitives::empty().components().count(), 0);

    // Asked again, on a clone or a points-only edit: the same answer, not
    // a second one.
    let value = Geometry::new(vec![[0.0; 3]; 9].into(), t, Attributes::new()).unwrap();
    let moved = value.with_positions(vec![[1.0; 3]; 9].into()).unwrap();
    assert!(Arc::ptr_eq(
        &value.primitives().components(),
        &moved.primitives().components()
    ));
    let boundary: *const [u32] = value.primitives().index().boundary_edges();
    let again: *const [u32] = moved.primitives().index().boundary_edges();
    assert_eq!(boundary, again, "the boundary is found once");
}
