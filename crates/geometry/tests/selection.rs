//! A selection over a polygons value: validity, its elements, the same
//! selection as another domain, its box and its counts.
use kinogaki_geometry::{
    Attributes, Bounds, Domain, ErrorKind, Polygons, Selection, SelectionCounts, Topology, Vec3,
};

/// A unit quad (0 1 2 3) and a triangle (1 4 2) sharing the edge 1–2, and
/// a polyline 3 → 5 → 6 off the quad's corner.
///
/// ```text
///   3 ─── 2         6
///   │     │ ╲      /
///   │  0  │ 1 4   5
///   0 ─── 1 ╱     (polyline 3–5–6)
/// ```
fn value() -> Polygons {
    let positions = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [2.0, 0.5, 0.0],
        [-1.0, 2.0, 0.0],
        [-2.0, 3.0, 1.0],
    ];
    let topology = Topology::new(
        7,
        vec![0, 4, 7, 10].into(),
        vec![0, 1, 2, 3, 1, 4, 2, 3, 5, 6].into(),
        2,
    )
    .unwrap();
    Polygons::new(positions.into(), topology, Attributes::new()).unwrap()
}

fn on(p: &Polygons, domain: Domain, ids: &[u32]) -> Selection {
    Selection::new(domain, ids.to_vec(), None, Some(p.topology().fingerprint())).unwrap()
}

fn ids(s: &Selection) -> Vec<u32> {
    s.indices().to_vec()
}

/// The edge between points `a` and `b`.
fn edge(p: &Polygons, a: u32, b: u32) -> u32 {
    p.topology().index().edge_of(a, b).unwrap()
}

#[test]
fn a_selection_is_valid_on_its_own_topology_and_within_its_elements() {
    let p = value();
    on(&p, Domain::Point, &[0, 6]).validate(&p).unwrap();
    // Made on no particular topology: valid while its indices are.
    Selection::new(Domain::Edge, vec![1], None, None)
        .unwrap()
        .validate(&p)
        .unwrap();
    let stale = Selection::new(Domain::Point, vec![0], None, Some([7; 32])).unwrap();
    let e = stale.validate(&p).unwrap_err();
    assert_eq!((e.kind(), e.element()), (ErrorKind::Selection, None));
    // The first index past the end is the one named.
    let past = on(&p, Domain::Primitive, &[1, 3, 9]);
    let e = past.validate(&p).unwrap_err();
    assert_eq!((e.kind(), e.element()), (ErrorKind::Selection, Some(3)));
    let e = Selection::none(Domain::Object).validate(&p).unwrap_err();
    assert_eq!(e.message(), "polygons have no object domain");
}

#[test]
fn resolved_shares_the_indices_and_spells_out_all() {
    let p = value();
    let some = on(&p, Domain::Point, &[2, 5]);
    assert!(some.resolved(&p).unwrap().ptr_eq(some.indices()));
    let all = Selection::all(Domain::Primitive, None);
    assert_eq!(&*all.resolved(&p).unwrap(), &[0, 1, 2]);
    assert_eq!(
        &*Selection::all(Domain::Detail, None).resolved(&p).unwrap(),
        &[0]
    );
}

#[test]
fn polygons_become_their_points_edges_and_vertices() {
    let p = value();
    let triangle = on(&p, Domain::Primitive, &[1]);
    assert_eq!(
        ids(&triangle.converted(&p, Domain::Point).unwrap()),
        [1, 2, 4]
    );
    assert_eq!(
        ids(&triangle.converted(&p, Domain::Vertex).unwrap()),
        [4, 5, 6]
    );
    let mut edges = vec![edge(&p, 1, 4), edge(&p, 4, 2), edge(&p, 2, 1)];
    edges.sort_unstable();
    assert_eq!(ids(&triangle.converted(&p, Domain::Edge).unwrap()), edges);
    // The polyline's points; its last vertex starts no segment.
    let polyline = on(&p, Domain::Primitive, &[2]);
    assert_eq!(
        ids(&polyline.converted(&p, Domain::Point).unwrap()),
        [3, 5, 6]
    );
    let mut segments = vec![edge(&p, 3, 5), edge(&p, 5, 6)];
    segments.sort_unstable();
    assert_eq!(
        ids(&polyline.converted(&p, Domain::Edge).unwrap()),
        segments
    );
    assert_eq!(ids(&triangle.converted(&p, Domain::Detail).unwrap()), [0]);
}

#[test]
fn points_become_whole_polygons_and_edges_by_the_standard_rule() {
    let p = value();
    // The shared edge's points and the triangle's tip: the triangle alone
    // is whole; edges need both their points.
    let points = on(&p, Domain::Point, &[1, 2, 4]);
    let faces = points.converted(&p, Domain::Primitive).unwrap();
    assert_eq!(ids(&faces), [1]);
    let mut edges = vec![edge(&p, 1, 2), edge(&p, 1, 4), edge(&p, 2, 4)];
    edges.sort_unstable();
    assert_eq!(ids(&points.converted(&p, Domain::Edge).unwrap()), edges);
    // Edges to points: any edge end.
    let shared = on(&p, Domain::Edge, &[edge(&p, 1, 2)]);
    assert_eq!(ids(&shared.converted(&p, Domain::Point).unwrap()), [1, 2]);
    // A primitive converts to its points.
    let piece = on(&p, Domain::Primitive, &[0]);
    assert_eq!(
        ids(&piece.converted(&p, Domain::Point).unwrap()),
        [0, 1, 2, 3]
    );
    // Every result is made on the value's topology.
    assert_eq!(faces.topology(), Some(p.topology().fingerprint()));
}

#[test]
fn all_stays_all_and_the_same_domain_is_the_selection_itself() {
    let p = value();
    let all = Selection::all(Domain::Point, None)
        .converted(&p, Domain::Edge)
        .unwrap();
    assert!(all.is_all() && all.domain() == Domain::Edge);
    let s = on(&p, Domain::Edge, &[0]);
    assert!(
        s.converted(&p, Domain::Edge)
            .unwrap()
            .indices()
            .ptr_eq(s.indices())
    );
    let e = s.converted(&p, Domain::Instance).unwrap_err();
    assert_eq!(e.kind(), ErrorKind::Selection);
}

#[test]
fn bounds_hold_the_touched_points() {
    let p = value();
    let triangle = on(&p, Domain::Primitive, &[1]);
    assert_eq!(
        triangle.bounds(&p),
        Bounds::of([Vec3::X, Vec3::new(2.0, 1.0, 0.0)])
    );
    let polyline_end = on(&p, Domain::Vertex, &[9]);
    assert_eq!(
        polyline_end.bounds(&p),
        Some(Bounds::point(Vec3::new(-2.0, 3.0, 1.0)))
    );
    assert_eq!(Selection::none(Domain::Point).bounds(&p), None);
    let stale = Selection::new(Domain::Point, vec![0], None, Some([7; 32])).unwrap();
    assert_eq!(stale.bounds(&p), None);
}

#[test]
fn counts_hold_the_selection_and_what_its_points_cover() {
    let p = value();
    let count = |s: &Selection| {
        let c = s.counts(&p);
        (c.points, c.edges, c.polygons)
    };
    // The triangle: its 3 points, its 3 edges, itself.
    assert_eq!(count(&on(&p, Domain::Primitive, &[1])), (3, 3, 1));
    // A polyline is no polygon: its points and segments only.
    assert_eq!(count(&on(&p, Domain::Primitive, &[2])), (3, 2, 0));
    assert_eq!(count(&on(&p, Domain::Primitive, &[0, 1])), (5, 6, 2));
    // Two points of one edge: that edge, no polygon.
    assert_eq!(count(&on(&p, Domain::Point, &[1, 2])), (2, 1, 0));
    let edges = p.topology().edges().len();
    assert_eq!(count(&Selection::all(Domain::Edge, None)), (7, edges, 2));
    assert_eq!(count(&Selection::all(Domain::Detail, None)), (7, edges, 2));
    let stale = Selection::new(Domain::Point, vec![0], None, Some([7; 32])).unwrap();
    assert_eq!(stale.counts(&p), SelectionCounts::default());
    // An empty selection.
    assert_eq!(count(&Selection::none(Domain::Point)), (0, 0, 0));
}
