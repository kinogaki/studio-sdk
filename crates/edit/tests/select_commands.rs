//! The selection commands' rules (`select::{similar, by_normal,
//! flood_by_angle, hard_edges, border, rim, loops, shortest_path}`) and the
//! shortest-way search under them, on analytic cubes and planes.
use kinogaki_edit::select::{self, Ways};
use kinogaki_geometry::{Attributes, Domain, Elements, Geometry, Primitives, Selection};

/// A unit cube about the origin, outward quads.
fn cube() -> Geometry {
    let s = 0.5;
    let positions: Vec<[f64; 3]> = vec![
        [-s, -s, -s],
        [s, -s, -s],
        [s, -s, s],
        [-s, -s, s],
        [-s, s, -s],
        [s, s, -s],
        [s, s, s],
        [-s, s, s],
    ];
    let corners = vec![
        0, 1, 2, 3, 4, 7, 6, 5, 0, 4, 5, 1, 2, 6, 7, 3, 3, 7, 4, 0, 1, 5, 6, 2,
    ];
    let offsets: Vec<u32> = (0..=6).map(|f| 4 * f).collect();
    let table = Primitives::of_polygons(8, offsets.into(), corners.into()).unwrap();
    Geometry::new(positions.into(), table, Attributes::new()).unwrap()
}

/// A square of side 1 on the ground (Y up) in `n` × `n` quads facing up.
fn plane(n: u32) -> Geometry {
    let step = 1.0 / f64::from(n);
    let positions: Vec<[f64; 3]> = (0..=n)
        .flat_map(|j| {
            (0..=n).map(move |i| [f64::from(i) * step - 0.5, 0.0, f64::from(j) * step - 0.5])
        })
        .collect();
    let at = |i: u32, j: u32| j * (n + 1) + i;
    let corners: Vec<u32> = (0..n)
        .flat_map(|j| {
            (0..n).flat_map(move |i| [at(i, j), at(i, j + 1), at(i + 1, j + 1), at(i + 1, j)])
        })
        .collect();
    let offsets: Vec<u32> = (0..=n * n).map(|f| 4 * f).collect();
    let points = positions.len();
    let table = Primitives::of_polygons(points, offsets.into(), corners.into()).unwrap();
    Geometry::new(positions.into(), table, Attributes::new()).unwrap()
}

fn on(value: &Geometry, domain: Domain, members: &[u32]) -> Selection {
    let count = value.count(domain).unwrap();
    let members = Elements::from_indices(count, members.iter().copied()).unwrap();
    Selection::on(value, domain, members, None).unwrap()
}

fn faces(value: &Geometry, members: &[u32]) -> Selection {
    on(value, Domain::Primitive, members)
}

fn points(value: &Geometry, members: &[u32]) -> Selection {
    on(value, Domain::Point, members)
}

fn edges(value: &Geometry, pairs: &[(u32, u32)]) -> Selection {
    let table = value.primitives();
    let found: Vec<u32> = (pairs.iter())
        .map(|&(a, b)| table.edge_of(a, b).expect("an edge"))
        .collect();
    on(value, Domain::Edge, &found)
}

fn at(value: &Geometry, p: u32) -> [f64; 3] {
    value.positions()[p as usize]
}

/// The point of `plane` at `x`, `z`.
fn point_at(plane: &Geometry, x: f64, z: f64) -> u32 {
    (0..plane.positions().len() as u32)
        .find(|&p| {
            let a = at(plane, p);
            (a[0] - x).abs() < 1e-9 && (a[2] - z).abs() < 1e-9
        })
        .unwrap()
}

fn found(s: &Selection) -> Vec<u32> {
    s.members().iter().collect()
}

#[test]
fn similar_faces_have_as_many_corners_and_about_the_area() {
    let cube = cube();
    let like = select::similar(&faces(&cube, &[0]), &cube).unwrap();
    assert_eq!(found(&like).len(), 6);
}

#[test]
fn similar_points_have_as_many_edges() {
    let plane = plane(2);
    // The middle of a 2 × 2 plane is the only point with four edges.
    let middle = point_at(&plane, 0.0, 0.0);
    let like = select::similar(&points(&plane, &[middle]), &plane).unwrap();
    assert_eq!(found(&like), [middle]);
}

#[test]
fn by_normal_finds_what_faces_the_same_way_and_no_other() {
    let plane = plane(2);
    let up = select::by_normal(&faces(&plane, &[0]), &plane).unwrap();
    assert_eq!(found(&up).len(), 4);
    let cube = cube();
    assert_eq!(
        found(&select::by_normal(&faces(&cube, &[0]), &cube).unwrap()),
        [0]
    );
}

#[test]
fn flooding_by_angle_stops_at_a_cubes_edges() {
    let cube = cube();
    let one = select::flood_by_angle(&faces(&cube, &[0]), &cube).unwrap();
    assert_eq!(found(&one), [0]);
    let plane = plane(3);
    let all = select::flood_by_angle(&faces(&plane, &[4]), &plane).unwrap();
    assert_eq!(found(&all).len(), 9);
}

#[test]
fn hard_edges_are_every_edge_of_a_cube_and_none_of_a_plane() {
    let cube = cube();
    let hard = select::hard_edges(&edges(&cube, &[(0, 4)]), &cube).unwrap();
    assert_eq!(hard.len(), 12);
    let plane = plane(2);
    let none = select::hard_edges(&edges(&plane, &[(0, 1)]), &plane).unwrap();
    assert_eq!(none.len(), 0);
}

#[test]
fn a_border_is_the_open_rim_a_selection_touches() {
    let plane = plane(2);
    assert_eq!(select::border(None, &plane).unwrap().len(), 8);
    let corner = points(&plane, &[0]);
    assert_eq!(select::border(Some(&corner), &plane).unwrap().len(), 8);
    let inside = points(&plane, &[point_at(&plane, 0.0, 0.0)]);
    let touches = select::border(Some(&inside), &plane).unwrap();
    assert_eq!(touches.len(), 0, "touches none");
    assert_eq!(select::border(None, &cube()).unwrap().len(), 0, "closed");
}

#[test]
fn a_rim_is_a_faces_outline_and_the_outer_points() {
    let two = plane(2);
    let all = faces(&two, &[0, 1, 2, 3]);
    assert_eq!(select::rim(&all, &two).unwrap().len(), 8);
    let plane = plane(4);
    let inner: Vec<u32> = (0..25u32)
        .filter(|&p| at(&plane, p).iter().all(|c| c.abs() < 0.3))
        .collect();
    let rim = select::rim(&points(&plane, &inner), &plane).unwrap();
    assert_eq!(rim.len(), 8, "all but the middle");
}

#[test]
fn two_neighbouring_faces_make_their_loop() {
    let cube = cube();
    let top = cube.primitives().edge_of(0, 1).unwrap();
    let two: Vec<u32> = cube.primitives().index().edge_faces(top as usize).to_vec();
    let around = select::loops(&faces(&cube, &two), &cube).unwrap();
    assert_eq!(around.len(), 4, "a band round the cube");
}

#[test]
fn two_neighbouring_points_make_their_edge_loop() {
    let plane = plane(4);
    let row: Vec<u32> = (0..25u32)
        .filter(|&p| at(&plane, p)[2].abs() < 1e-9)
        .take(2)
        .collect();
    let around = select::loops(&points(&plane, &row), &plane).unwrap();
    assert_eq!(around.len(), 5, "across the plane");
}

#[test]
fn the_shortest_way_between_points_edges_and_faces() {
    let plane = plane(4);
    let (a, b) = (point_at(&plane, -0.5, -0.5), point_at(&plane, 0.5, -0.5));
    let way = select::shortest_path(&points(&plane, &[a, b]), &plane).unwrap();
    assert_eq!(way.len(), 5, "along the side");
    let side = edges(&plane, &[(a, point_at(&plane, -0.25, -0.5))]);
    let other = plane
        .primitives()
        .edge_of(b, point_at(&plane, 0.25, -0.5))
        .unwrap();
    let ends = on(&plane, Domain::Edge, &[found(&side)[0], other]);
    let along = select::shortest_path(&ends, &plane).unwrap();
    assert_eq!(along.len(), 4, "the side's four edges");
    let across = select::shortest_path(&faces(&plane, &[0, 3]), &plane).unwrap();
    assert_eq!(across.len(), 4, "the row of faces between");
    let one = faces(&plane, &[0]);
    assert!(select::shortest_path(&one, &plane).is_err(), "needs two");
}

#[test]
fn ways_reach_no_further_than_their_limit_and_lead_back_to_a_start() {
    // A line of points 0 – 1 – 2 – 3, one apart.
    let around = |p: u32, next: &mut dyn FnMut(u32, f64)| {
        for q in [p.wrapping_sub(1), p + 1] {
            if q < 4 {
                next(q, 1.0);
            }
        }
    };
    let near = Ways::search(&[0], 2.5, |_| false, around);
    let mut reached: Vec<(u32, f64)> = near.distances().collect();
    reached.sort_by_key(|&(p, _)| p);
    assert_eq!(reached, [(0, 0.0), (1, 1.0), (2, 2.0)]);
    assert_eq!(near.end(), None);
    let to = Ways::search(&[0, 3], f64::INFINITY, |p| p == 1, around);
    assert_eq!(to.end(), Some(1));
    assert_eq!(to.path(1), [1, 0], "from the nearer start");
    assert!(to.path(7).is_empty());
}
