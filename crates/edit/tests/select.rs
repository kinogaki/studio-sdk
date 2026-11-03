//! The selection rules over a polygons value: validity, its elements,
//! the same selection as another domain, and connected pieces.
use kinogaki_edit::select::{self, Coverage, Expand, Mode};
use kinogaki_geometry::{
    Attribute, Attributes, Domain, ErrorKind, Polygons, Selection, Text, Topology, names,
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
    assert_eq!(e.message(), "the value has no object domain");
}

#[test]
fn resolved_shares_the_indices_and_spells_out_all() {
    let p = value();
    let some = on(&p, Domain::Point, &[2, 5]);
    assert!(select::resolve(&some, &p).unwrap().ptr_eq(some.indices()));
    let all = Selection::all(Domain::Primitive, None);
    assert_eq!(&*select::resolve(&all, &p).unwrap(), &[0, 1, 2]);
    assert_eq!(
        &*select::resolve(&Selection::all(Domain::Detail, None), &p).unwrap(),
        &[0]
    );
}

#[test]
fn polygons_become_their_points_edges_and_vertices() {
    let p = value();
    let triangle = on(&p, Domain::Primitive, &[1]);
    assert_eq!(
        ids(&select::convert(&triangle, &p, Domain::Point).unwrap()),
        [1, 2, 4]
    );
    assert_eq!(
        ids(&select::convert(&triangle, &p, Domain::Vertex).unwrap()),
        [4, 5, 6]
    );
    let mut edges = vec![edge(&p, 1, 4), edge(&p, 4, 2), edge(&p, 2, 1)];
    edges.sort_unstable();
    assert_eq!(
        ids(&select::convert(&triangle, &p, Domain::Edge).unwrap()),
        edges
    );
    // The polyline's points; its last vertex starts no segment.
    let polyline = on(&p, Domain::Primitive, &[2]);
    assert_eq!(
        ids(&select::convert(&polyline, &p, Domain::Point).unwrap()),
        [3, 5, 6]
    );
    let mut segments = vec![edge(&p, 3, 5), edge(&p, 5, 6)];
    segments.sort_unstable();
    assert_eq!(
        ids(&select::convert(&polyline, &p, Domain::Edge).unwrap()),
        segments
    );
    assert_eq!(
        ids(&select::convert(&triangle, &p, Domain::Detail).unwrap()),
        [0]
    );
}

#[test]
fn points_become_whole_polygons_and_edges_by_the_standard_rule() {
    let p = value();
    // The shared edge's points and the triangle's tip: the triangle alone
    // is whole; edges need both their points.
    let points = on(&p, Domain::Point, &[1, 2, 4]);
    let faces = select::convert(&points, &p, Domain::Primitive).unwrap();
    assert_eq!(ids(&faces), [1]);
    let mut edges = vec![edge(&p, 1, 2), edge(&p, 1, 4), edge(&p, 2, 4)];
    edges.sort_unstable();
    assert_eq!(
        ids(&select::convert(&points, &p, Domain::Edge).unwrap()),
        edges
    );
    // Edges to points: any edge end.
    let shared = on(&p, Domain::Edge, &[edge(&p, 1, 2)]);
    assert_eq!(
        ids(&select::convert(&shared, &p, Domain::Point).unwrap()),
        [1, 2]
    );
    // A primitive converts to its points.
    let piece = on(&p, Domain::Primitive, &[0]);
    assert_eq!(
        ids(&select::convert(&piece, &p, Domain::Point).unwrap()),
        [0, 1, 2, 3]
    );
    // Every result is made on the value's topology.
    assert_eq!(faces.topology(), Some(p.topology().fingerprint()));
}

#[test]
fn all_stays_all_and_the_same_domain_is_the_selection_itself() {
    let p = value();
    let all = select::convert(&Selection::all(Domain::Point, None), &p, Domain::Edge).unwrap();
    assert!(all.is_all() && all.domain() == Domain::Edge);
    let s = on(&p, Domain::Edge, &[0]);
    assert!(
        select::convert(&s, &p, Domain::Edge)
            .unwrap()
            .indices()
            .ptr_eq(s.indices())
    );
    let e = select::convert(&s, &p, Domain::Instance).unwrap_err();
    assert_eq!(e.kind(), ErrorKind::Selection);
}

#[test]
fn coverage_chooses_between_any_and_whole() {
    let p = value();
    let points = on(&p, Domain::Point, &[1, 2]);
    // Any: both polygons through the shared edge's points.
    let any = select::convert_by(&points, &p, Domain::Primitive, Coverage::Any).unwrap();
    assert_eq!(ids(&any), [0, 1]);
    // Whole: neither holds only those points.
    let whole = select::convert_by(&points, &p, Domain::Primitive, Coverage::Whole).unwrap();
    assert!(whole.is_empty());
    // A stale selection converts to nothing.
    let stale = Selection::new(Domain::Point, vec![0], None, Some([7; 32])).unwrap();
    assert!(select::convert(&stale, &p, Domain::Edge).is_err());
}

/// Two quads apart (0 1 2 3, 4 5 6 7) and a lone triangle (8 9 10), the
/// triangle named like the first quad when `named`.
fn islands(named: bool) -> Polygons {
    let positions: Vec<[f64; 3]> = (0..11).map(|i| [f64::from(i), 0.0, 0.0]).collect();
    let topology = Topology::new(
        11,
        vec![0, 4, 8, 11].into(),
        vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10].into(),
        3,
    )
    .unwrap();
    let mut attributes = Attributes::new();
    if named {
        let text = Text::from_strs(["leaf", " ", "leaf"]);
        attributes = attributes
            .with(Domain::Primitive, names::NAME, Attribute::text(text))
            .unwrap();
    }
    Polygons::new(positions.into(), topology, attributes).unwrap()
}

#[test]
fn connected_grows_a_pick_to_its_pieces_and_a_name_keeps_one_together() {
    let p = islands(false);
    let pieces = Mode {
        domain: Domain::Primitive,
        expand: Expand::Connected,
    };
    let pick = on(&p, Domain::Primitive, &[1]);
    assert_eq!(ids(&select::expand(&pick, &p, &pieces.expand)), [1]);
    // A point of the first quad: its four points.
    let point = on(&p, Domain::Point, &[2]);
    let grown = select::expand(&point, &p, &Expand::Connected);
    assert_eq!(ids(&grown), [0, 1, 2, 3]);
    assert_eq!(grown.topology(), Some(p.topology().fingerprint()));
    // Named alike, the first quad and the triangle are one piece; a blank
    // name joins nothing.
    let named = islands(true);
    let first = on(&named, Domain::Primitive, &[0]);
    assert_eq!(
        ids(&select::expand(&first, &named, &Expand::Connected)),
        [0, 2]
    );
    let edges =
        select::convert(&on(&named, Domain::Primitive, &[2]), &named, Domain::Edge).unwrap();
    let grown = select::expand(&edges, &named, &Expand::Connected);
    assert_eq!(
        grown.len(0),
        7,
        "the quad's four edges and the triangle's three"
    );
    // As picked, and a selection of another value, stay as they are.
    assert_eq!(select::expand(&first, &named, &Expand::None), first);
    let stale = Selection::new(Domain::Primitive, vec![0], None, Some([7; 32])).unwrap();
    assert_eq!(select::expand(&stale, &named, &Expand::Connected), stale);
    assert_eq!(Mode::from(Domain::Edge).expand, Expand::None);
}
