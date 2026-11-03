//! Curve primitives evaluated ([`Geometry::curve`]): each basis's
//! parameters and spans, exact points, and closed curves meeting.
use kinogaki_geometry::{
    Attributes, Basis, Column, CurveData, Geometry, Kind, Knots, Primitives, Vec3,
};

/// One curve of `kind` and `degree` through `points`, with NURBS `knots`
/// and `weights` when given.
fn curve(
    kind: Kind,
    degree: u8,
    points: Vec<[f64; 3]>,
    knots: Vec<f64>,
    weights: Option<Vec<f64>>,
) -> Geometry {
    let n = points.len() as u32;
    let knots = Knots::new(vec![0, knots.len() as u32].into(), knots.into());
    let data = CurveData::new(vec![degree].into(), knots, weights.map(Column::new));
    let vertices: Vec<u32> = (0..n).collect();
    let table = Primitives::new(
        n as usize,
        vec![0, n].into(),
        vertices.into(),
        vec![kind].into(),
        Some(data),
    )
    .unwrap();
    Geometry::new(points.into(), table, Attributes::new()).unwrap()
}

fn open(basis: Basis, degree: u8, points: Vec<[f64; 3]>) -> Geometry {
    curve(Kind::curve(basis, false), degree, points, Vec::new(), None)
}

fn close(a: Vec3, b: impl Into<Vec3>, tolerance: f64) -> bool {
    let d = a - b.into();
    [d.x, d.y, d.z].iter().all(|c| c.abs() <= tolerance)
}

#[test]
fn a_rational_quadratic_is_exactly_a_circle() {
    let w = std::f64::consts::FRAC_1_SQRT_2;
    let quarter = curve(
        Kind::curve(Basis::Nurbs, false),
        2,
        vec![[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]],
        vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        Some(vec![1.0, w, 1.0]),
    );
    let c = quarter.curve(0);
    assert_eq!((c.domain(), c.spans().count()), ([0.0, 1.0], 1));
    for k in 0..=100 {
        let p = c.point(k as f64 / 100.0);
        assert!((p.x.hypot(p.y) - 1.0).abs() < 1e-12, "{p:?}");
    }
}

#[test]
fn bezier_ends_on_its_end_points_and_follows_bernstein() {
    let p = vec![
        [0.0, 0.0, 0.0],
        [1.0, 2.0, 0.0],
        [3.0, 2.0, 0.0],
        [4.0, 0.0, 0.0],
        [5.0, -2.0, 0.0],
        [6.0, -2.0, 0.0],
        [7.0, 0.0, 0.0],
    ];
    let value = open(Basis::Bezier, 3, p.clone());
    let cubic = value.curve(0);
    assert_eq!((cubic.domain(), cubic.spans().count()), ([0.0, 2.0], 2));
    assert_eq!(cubic.point(0.0), p[0].into());
    assert_eq!(cubic.point(1.0), p[3].into());
    assert_eq!(cubic.point(2.0), p[6].into());
    // At t = ½: (p0 + 3 p1 + 3 p2 + p3) / 8.
    let half: [f64; 3] =
        std::array::from_fn(|c| (p[0][c] + 3.0 * p[1][c] + 3.0 * p[2][c] + p[3][c]) / 8.0);
    assert!(close(cubic.point(0.5), half, 1e-12));
    // A span read alone agrees with the whole curve.
    let second = cubic.spans().nth(1).unwrap();
    assert!(close(cubic.on(second, 1.25), cubic.point(1.25), 1e-12));
}

#[test]
fn an_open_bspline_is_clamped_and_a_straight_one_straight() {
    let line: Vec<[f64; 3]> = (0..6).map(|k| [k as f64, 2.0 * k as f64, 0.0]).collect();
    let value = open(Basis::BSpline, 3, line.clone());
    let spline = value.curve(0);
    assert_eq!(spline.domain(), [0.0, 3.0]);
    assert!(close(spline.point(0.0), line[0], 1e-12));
    assert!(close(spline.point(3.0), line[5], 1e-12));
    for k in 0..=30 {
        let p = spline.point(k as f64 / 10.0);
        assert!((p.y - 2.0 * p.x).abs() < 1e-12, "on the line: {p:?}");
    }
}

#[test]
fn catmull_rom_passes_through_every_point() {
    let points = vec![
        [0.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [2.0, -1.0, 0.0],
        [3.0, 0.5, 1.0],
        [4.0, 0.0, 0.0],
    ];
    let value = open(Basis::CatmullRom, 3, points.clone());
    let c = value.curve(0);
    assert_eq!(c.domain(), [0.0, 4.0]);
    for (k, p) in points.iter().enumerate() {
        assert!(close(c.point(k as f64), *p, 1e-12), "point {k}");
    }
    let ring = curve(
        Kind::curve(Basis::CatmullRom, true),
        3,
        points.clone(),
        Vec::new(),
        None,
    );
    let c = ring.curve(0);
    assert_eq!(c.domain(), [0.0, 5.0]);
    assert!(close(c.point(5.0), points[0], 1e-12), "it closes");
}

#[test]
fn closed_curves_meet_and_linear_primitives_are_their_segments() {
    let square = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
    ];
    let ring = curve(
        Kind::curve(Basis::BSpline, true),
        3,
        square.clone(),
        Vec::new(),
        None,
    );
    let c = ring.curve(0);
    assert_eq!((c.domain(), c.spans().count()), ([0.0, 4.0], 4));
    assert!(close(c.point(0.0), c.point(4.0), 1e-12), "the ends meet");
    // A polygon is its sides, closing.
    let quad = Primitives::of_polygons(4, vec![0, 4].into(), vec![0, 1, 2, 3].into()).unwrap();
    let value = Geometry::new(square.clone().into(), quad, Attributes::new()).unwrap();
    let sides = value.curve(0);
    assert_eq!((sides.degree(), sides.spans().count()), (1, 4));
    assert_eq!(sides.point(3.5).to_array(), [0.0, 0.5, 0.0]);
}
