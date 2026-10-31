//! Topology: every rule with a passing and a failing case, the limits, the
//! canonical edges, the fingerprint and the derived slots.
use kinogaki_geometry::limits::{MAX_POINTS, MAX_PRIMITIVES, MAX_VERTICES};
use kinogaki_geometry::{Edge, ErrorKind, GeometryError, Topology};
use std::sync::{Arc, Weak};

/// The topology of `prims` over `points`, the first `polygons` polygons.
fn make(points: usize, prims: &[&[u32]], polygons: usize) -> Result<Topology, GeometryError> {
    let mut offsets = vec![0];
    let mut vertices = Vec::new();
    for p in prims {
        vertices.extend_from_slice(p);
        offsets.push(vertices.len() as u32);
    }
    Topology::new(points, offsets.into(), vertices.into(), polygons)
}

fn refused(result: Result<Topology, GeometryError>) -> (Option<usize>, String) {
    let e = result.unwrap_err();
    assert_eq!(e.kind(), ErrorKind::Topology, "{}", e.message());
    (e.element(), e.message().to_owned())
}

#[test]
fn valid_topologies_are_accepted() {
    let t = make(6, &[&[0, 1, 2], &[0, 2, 3, 4], &[4, 5], &[5, 0, 5]], 2).unwrap();
    assert_eq!((t.primitives(), t.polygons(), t.polylines()), (4, 2, 2..4));
    assert_eq!((t.vertex_count(), t.polygon_vertices()), (12, 7));
    assert_eq!(t.primitive(1), &[0, 2, 3, 4]);
    assert_eq!(t.primitive_vertices(2), 7..9);
    assert!(t.is_polyline(3) && !t.is_polyline(1));
    assert!(make(0, &[], 0).is_ok());
    assert!(make(3, &[], 0).is_ok(), "points only");
    assert_eq!(Topology::empty(), make(0, &[], 0).unwrap());
}

#[test]
fn offsets_must_run_from_zero_to_the_vertex_count() {
    let t = |o: Vec<u32>, v: Vec<u32>| Topology::new(4, o.into(), v.into(), 0);
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
        (Some(1), "polyline 1 has its offsets decreasing".into())
    );
    assert_eq!(
        refused(t(vec![0, 9, 4], vec![0, 1, 2, 3])),
        (
            Some(0),
            "polyline 0 has its offsets past the vertices".into()
        )
    );
}

#[test]
fn polygons_are_at_most_the_primitives() {
    assert_eq!(
        refused(make(4, &[&[0, 1, 2]], 2)),
        (None, "2 polygons, but only 1 primitives".into())
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
    let e = Topology::new(n as usize + 2, offsets.into(), vertices.into(), n as usize).unwrap_err();
    assert_eq!(
        (e.element(), e.message()),
        (Some(70_000), "polygon 70000 repeats a point")
    );
}

#[test]
fn one_past_each_limit_is_refused() {
    let limit = |r: Result<Topology, GeometryError>| r.unwrap_err().kind();
    assert!(Topology::new(MAX_POINTS, vec![0].into(), Vec::new().into(), 0).is_ok());
    assert_eq!(
        limit(Topology::new(
            MAX_POINTS + 1,
            vec![0].into(),
            Vec::new().into(),
            0
        )),
        ErrorKind::Limit
    );
    assert_eq!(
        limit(Topology::new(
            4,
            vec![0; MAX_PRIMITIVES + 2].into(),
            Vec::new().into(),
            0
        )),
        ErrorKind::Limit
    );
    assert_eq!(
        limit(Topology::new(
            4,
            vec![0].into(),
            vec![0; MAX_VERTICES + 1].into(),
            0
        )),
        ErrorKind::Limit
    );
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
fn random(seed: u64, points: u32, polygons: usize, polylines: usize) -> Topology {
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
fn naive_edges(t: &Topology) -> Vec<Edge> {
    let mut edges = Vec::new();
    for p in 0..t.primitives() {
        let ids = t.primitive(p);
        let n = ids.len();
        let segments = if t.is_polyline(p) { n - 1 } else { n };
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
        for p in 0..t.primitives() {
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
