//! Primitives: every rule with a passing and a failing case, the limits, the
//! canonical edges, the fingerprint and the derived slots.
use kinogaki_geometry::limits::{MAX_POINTS, MAX_PRIMITIVES, MAX_VERTICES};
use kinogaki_geometry::{
    Basis, CurveData, Edge, ErrorKind, GeometryError, Kind, Knots, Primitives,
};
use std::sync::{Arc, Weak};

/// The table of `prims` over `points`, the first `polygons` polygons, the
/// rest polylines.
fn make(points: usize, prims: &[&[u32]], polygons: usize) -> Result<Primitives, GeometryError> {
    let kinds: Vec<Kind> = (0..prims.len())
        .map(|p| {
            if p < polygons {
                Kind::POLYGON
            } else {
                Kind::POLYLINE
            }
        })
        .collect();
    of(points, prims, &kinds, None)
}

/// The table of `prims` of `kinds` over `points`, with `curves`.
fn of(
    points: usize,
    prims: &[&[u32]],
    kinds: &[Kind],
    curves: Option<CurveData>,
) -> Result<Primitives, GeometryError> {
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
        curves,
    )
}

fn refused(result: Result<Primitives, GeometryError>) -> (Option<usize>, String) {
    let e = result.unwrap_err();
    assert_eq!(e.kind(), ErrorKind::Primitive, "{}", e.message());
    (e.element(), e.message().to_owned())
}

#[test]
fn valid_topologies_are_accepted() {
    let t = make(6, &[&[0, 1, 2], &[0, 2, 3, 4], &[4, 5], &[5, 0, 5]], 2).unwrap();
    assert_eq!(
        (t.len(), t.polygons().iter().collect::<Vec<_>>()),
        (4, vec![0, 1])
    );
    assert_eq!(
        (t.vertex_count(), t.kind(2), t.uniform()),
        (12, Kind::POLYLINE, None)
    );
    assert_eq!(t.primitive(1), &[0, 2, 3, 4]);
    assert_eq!(t.primitive_vertices(2), 7..9);
    assert!(make(0, &[], 0).is_ok());
    assert!(make(3, &[], 0).is_ok(), "points only");
    assert_eq!(Primitives::empty(), make(0, &[], 0).unwrap());
}

#[test]
fn offsets_must_run_from_zero_to_the_vertex_count() {
    let t = |o: Vec<u32>, v: Vec<u32>| Primitives::of_polygons(4, o.into(), v.into());
    assert_eq!(refused(t(vec![], vec![])).1, "the offsets must start at 0");
    assert_eq!(
        refused(t(vec![1, 3], vec![0, 1, 2])).1,
        "the offsets must start at 0"
    );
    assert_eq!(
        refused(t(vec![0, 2], vec![0, 1, 2])).1,
        "the offsets end at 2, not at the vertex count 3"
    );
    assert_eq!(
        refused(t(vec![0, 3, 2, 4], vec![0, 1, 2, 3])),
        (Some(1), "polygon 1 has its offsets decreasing".into())
    );
    assert_eq!(
        refused(t(vec![0, 9, 4], vec![0, 1, 2, 3])),
        (
            Some(0),
            "polygon 0 has its offsets past the vertices".into()
        )
    );
}

#[test]
fn every_primitive_has_one_kind() {
    let kinds = [Kind::POLYGON, Kind::POLYGON];
    assert_eq!(
        refused(of(4, &[&[0, 1, 2]], &kinds, None)),
        (None, "2 kinds for 1 primitives".into())
    );
}

#[test]
fn primitives_have_enough_vertices() {
    assert!(make(4, &[&[0, 1, 2]], 1).is_ok());
    assert_eq!(
        refused(make(4, &[&[0, 1, 2], &[0, 1]], 2)),
        (Some(1), "polygon 1 has fewer than three vertices".into())
    );
    assert!(make(4, &[&[0, 1]], 0).is_ok());
    assert_eq!(
        refused(make(4, &[&[0, 1], &[2]], 0)),
        (Some(1), "polyline 1 has fewer than two vertices".into())
    );
}

#[test]
fn vertices_name_existing_points() {
    assert_eq!(
        refused(make(4, &[&[0, 1, 2], &[2, 9]], 1)),
        (Some(1), "polyline 1 references a missing point".into())
    );
    assert_eq!(
        refused(make(3, &[&[0, 1, 3]], 1)),
        (Some(0), "polygon 0 references a missing point".into())
    );
}

#[test]
fn polygons_never_repeat_a_point_and_polylines_never_consecutively() {
    for n in [3usize, 4, 6, 12] {
        let ring: Vec<u32> = (0..n as u32).collect();
        assert!(make(n, &[&ring], 1).is_ok());
        let mut repeated = ring.clone();
        repeated[n - 1] = 0;
        assert_eq!(
            refused(make(n, &[&repeated], 1)),
            (Some(0), "polygon 0 repeats a point".into()),
            "{n}-gon"
        );
    }
    assert!(
        make(4, &[&[0, 1, 0]], 0).is_ok(),
        "a polyline may come back"
    );
    assert_eq!(
        refused(make(4, &[&[0, 1, 1, 2]], 0)),
        (Some(0), "polyline 0 repeats a point consecutively".into())
    );
}

#[test]
fn the_first_fault_in_primitive_order_is_reported_on_large_topologies() {
    // 100k triangles checked in parallel blocks; faults at 70k and 90k.
    let n = 100_000u32;
    let mut vertices: Vec<u32> = (0..n).flat_map(|f| [f, f + 1, f + 2]).collect();
    vertices[90_000 * 3 + 1] = 90_000;
    vertices[70_000 * 3 + 2] = 70_000;
    let offsets: Vec<u32> = (0..=n).map(|f| f * 3).collect();
    let e = Primitives::of_polygons(n as usize + 2, offsets.into(), vertices.into()).unwrap_err();
    assert_eq!(
        (e.element(), e.message()),
        (Some(70_000), "polygon 70000 repeats a point")
    );
}

#[test]
fn one_past_each_limit_is_refused() {
    let limit = |r: Result<Primitives, GeometryError>| r.unwrap_err().kind();
    assert!(Primitives::of_polygons(MAX_POINTS, vec![0].into(), Vec::new().into()).is_ok());
    assert_eq!(
        limit(Primitives::of_polygons(
            MAX_POINTS + 1,
            vec![0].into(),
            Vec::new().into()
        )),
        ErrorKind::Limit
    );
    // Every id fits a u32, with u32::MAX free as "no element"; a table
    // past the primitive or vertex limits would take 16 GB to hold here.
    assert_eq!(MAX_PRIMITIVES, u32::MAX as usize - 1);
    assert_eq!(MAX_VERTICES, MAX_PRIMITIVES);
}

/// A deterministic random source.
struct Lcg(u64);
impl Lcg {
    fn below(&mut self, n: u32) -> u32 {
        self.0 = self
            .0
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        ((self.0 >> 33) % u64::from(n)) as u32
    }
}

/// A random topology: polygons with distinct points, then polylines.
fn random(seed: u64, points: u32, polygons: usize, polylines: usize) -> Primitives {
    let mut rng = Lcg(seed);
    let mut prims: Vec<Vec<u32>> = Vec::new();
    for _ in 0..polygons {
        let n = 3 + rng.below(5);
        let mut p: Vec<u32> = Vec::new();
        while p.len() < n as usize {
            let q = rng.below(points);
            if !p.contains(&q) {
                p.push(q);
            }
        }
        prims.push(p);
    }
    for _ in 0..polylines {
        let n = 2 + rng.below(6);
        let mut p = vec![rng.below(points)];
        while p.len() < n as usize {
            let q = rng.below(points);
            if q != *p.last().unwrap() {
                p.push(q);
            }
        }
        prims.push(p);
    }
    let prims: Vec<&[u32]> = prims.iter().map(|p| p.as_slice()).collect();
    make(points as usize, &prims, polygons).unwrap()
}

/// The edges by a naive sort and dedup of every side and segment.
fn naive_edges(t: &Primitives) -> Vec<Edge> {
    let mut edges = Vec::new();
    for p in 0..t.len() {
        let ids = t.primitive(p);
        let n = ids.len();
        let segments = if t.kind(p).closed() { n } else { n - 1 };
        for i in 0..segments {
            let (a, b) = (ids[i], ids[(i + 1) % n]);
            edges.push(Edge {
                points: [a.min(b), a.max(b)],
            });
        }
    }
    edges.sort();
    edges.dedup();
    edges
}

#[test]
fn edges_are_canonical_on_random_topologies() {
    // Dense and sparse point domains take the two builds.
    for (seed, points, polygons, polylines) in [
        (1, 20, 30, 0),
        (2, 50, 40, 25),
        (3, 2000, 3000, 500),
        (4, 100_000, 40, 10),
        (5, 30_000, 40_000, 2_000),
    ] {
        let t = random(seed, points, polygons, polylines);
        assert_eq!(t.edges(), naive_edges(&t), "seed {seed}");
        for (e, edge) in t.edges().iter().enumerate() {
            let [a, b] = edge.points;
            assert!(a < b);
            assert_eq!(t.edge_of(b, a), Some(e as u32));
        }
        // Each primitive's edges are its sides or segments, in order.
        let index = t.index();
        for p in 0..t.len() {
            let ids = t.primitive(p);
            for (i, &e) in index.primitive_edges(p).iter().enumerate() {
                let next = ids[(i + 1) % ids.len()];
                assert_eq!(t.edge_of(ids[i], next), Some(e));
            }
        }
    }
    assert_eq!(make(4, &[&[0, 1, 2]], 1).unwrap().edge_of(0, 3), None);
}

#[test]
fn the_fingerprint_follows_every_input() {
    let base = || make(5, &[&[0, 1, 2], &[2, 3, 4]], 1).unwrap();
    assert_eq!(base().fingerprint(), base().fingerprint());
    let others = [
        make(6, &[&[0, 1, 2], &[2, 3, 4]], 1).unwrap(),
        make(5, &[&[0, 1, 2], &[2, 3, 4]], 2).unwrap(),
        make(5, &[&[0, 1, 3], &[2, 3, 4]], 1).unwrap(),
        make(5, &[&[0, 1, 2, 3], &[3, 4]], 1).unwrap(),
    ];
    for other in others {
        assert_ne!(base().fingerprint(), other.fingerprint());
    }
}

#[test]
fn a_topology_cache_is_built_once_and_dropped_with_it() {
    struct Cache;
    let t = make(4, &[&[0, 1, 2, 3]], 1).unwrap();
    let mut builds = 0;
    let first = t.derived(|_| {
        builds += 1;
        Cache
    });
    let again = t.derived(|_| {
        builds += 1;
        Cache
    });
    assert!(Arc::ptr_eq(&first, &again) && builds == 1);
    let weak: Weak<Cache> = Arc::downgrade(&first);
    drop((first, again));
    assert!(weak.upgrade().is_some(), "kept by the topology");
    drop(t);
    assert!(weak.upgrade().is_none());
}

#[test]
fn kinds_hold_a_basis_and_closed_and_any_order_is_kept() {
    for basis in Basis::ALL {
        for closed in [false, true] {
            let k = Kind::curve(basis, closed);
            assert_eq!(
                (k.basis(), k.closed(), Kind::from_byte(k.byte())),
                (basis, closed, Some(k))
            );
        }
    }
    assert_eq!(Kind::from_byte(5), None);
    // Lines before polygons: nothing is renumbered, polygons() names them.
    let t = of(
        6,
        &[&[4, 5], &[0, 1, 2], &[5, 0, 4], &[0, 2, 3]],
        &[Kind::POLYLINE, Kind::POLYGON, Kind::POLYLINE, Kind::POLYGON],
        None,
    )
    .unwrap();
    assert_eq!(t.polygons().iter().collect::<Vec<_>>(), [1, 3]);
    assert_eq!(t.edges(), naive_edges(&t).as_slice());
    // All polygons: the set is every primitive, held as such.
    let quads = make(4, &[&[0, 1, 2, 3]], 1).unwrap();
    assert!(quads.polygons().is_all() && quads.uniform() == Some(Kind::POLYGON));
}

/// One cubic curve over points 0..count, of `basis`, closed or open, with
/// `knots` and `weights`.
fn curve(
    basis: Basis,
    closed: bool,
    count: u32,
    degree: u8,
    knots: Vec<f64>,
    weights: Option<Vec<f64>>,
) -> Result<Primitives, GeometryError> {
    let ids: Vec<u32> = (0..count).collect();
    let k = knots.len() as u32;
    let data = CurveData::new(
        vec![degree].into(),
        Knots::new(vec![0, k].into(), knots.into()),
        weights.map(Into::into),
    );
    of(
        count as usize,
        &[&ids],
        &[Kind::curve(basis, closed)],
        Some(data),
    )
}

#[test]
fn curves_follow_their_basis_rules() {
    let open = [0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    assert!(curve(Basis::Nurbs, false, 4, 3, open.to_vec(), None).is_ok());
    assert!(curve(Basis::BSpline, true, 4, 3, vec![], None).is_ok());
    assert!(curve(Basis::Bezier, false, 4, 3, vec![], None).is_ok());
    let message = |r: Result<Primitives, GeometryError>| {
        let e = r.unwrap_err();
        assert_eq!((e.kind(), e.element()), (ErrorKind::Curve, Some(0)));
        e.message().to_owned()
    };
    assert_eq!(
        message(curve(Basis::Bezier, false, 5, 3, vec![], None)),
        "curve 0 has 5 control points, not k·degree + 1 for degree 3"
    );
    assert_eq!(
        message(curve(Basis::Nurbs, false, 4, 3, open[1..].to_vec(), None)),
        "curve 0 has 7 knots, not 8 (count + degree + 1)"
    );
    assert_eq!(
        message(curve(Basis::Nurbs, false, 4, 3, vec![0.0; 8], None)),
        "curve 0 has knots that span no interval"
    );
    let weights = Some(vec![1.0, 0.0, 1.0, 1.0]);
    assert_eq!(
        message(curve(Basis::Nurbs, false, 4, 3, open.to_vec(), weights)),
        "curve 0 has a weight not above 0 at its vertex 1"
    );
    assert_eq!(
        message(curve(Basis::BSpline, false, 4, 3, open.to_vec(), None)),
        "curve 0 is bspline but has knots"
    );
    // Curve data exactly when a curve is there.
    let quad = of(4, &[&[0, 1, 2, 3]], &[Kind::POLYGON], None).unwrap();
    let extra = CurveData::new(
        vec![1].into(),
        Knots::new(vec![0, 0].into(), vec![].into()),
        None,
    );
    assert!(of(4, &[&[0, 1, 2, 3]], &[Kind::POLYGON], Some(extra)).is_err());
    assert!(
        of(
            4,
            &[&[0, 1, 2, 3]],
            &[Kind::curve(Basis::BSpline, false)],
            None
        )
        .is_err()
    );
    assert_eq!(quad.curves(), None);
}

#[test]
fn the_nurbs_knot_rule_is_the_breps_too() {
    use kinogaki_geometry::brep::NurbsCurve;
    // Unclamped knots: a geometry curve takes them, a B-rep curve (clamped
    // by its rule's parameter) doesn't; their counts are checked alike.
    let unclamped: Vec<f64> = (0..8).map(f64::from).collect();
    let points: Vec<[f64; 3]> = (0..4).map(|i| [f64::from(i), 0.0, 0.0]).collect();
    assert!(curve(Basis::Nurbs, false, 4, 3, unclamped.clone(), None).is_ok());
    assert!(NurbsCurve::new(3, unclamped, points.clone(), None).is_err());
    let short = vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0];
    assert!(curve(Basis::Nurbs, false, 4, 3, short.clone(), None).is_err());
    assert!(NurbsCurve::new(3, short, points, None).is_err());
}

#[test]
fn the_edge_count_needs_no_index() {
    let t = random(9, 400, 300, 40);
    let count = t.edge_count();
    assert_eq!(count, naive_edges(&t).len());
    assert_eq!(count, t.edges().len());
}
