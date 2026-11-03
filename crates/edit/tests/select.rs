//! The selection rules over a polygons value: validity, its elements,
//! the same selection as another domain, and connected pieces.
use kinogaki_edit::select::{self, Coverage, Expand, Mode};
use kinogaki_geometry::{
    Attribute, Attributes, Domain, Elements, ErrorKind, Geometry, Kind, Meaning, Primitives,
    Selection, Text, names,
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
fn value() -> Geometry {
    let positions = vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [2.0, 0.5, 0.0],
        [-1.0, 2.0, 0.0],
        [-2.0, 3.0, 1.0],
    ];
    let kinds = vec![Kind::POLYGON, Kind::POLYGON, Kind::POLYLINE];
    let topology = Primitives::new(
        7,
        vec![0, 4, 7, 10].into(),
        vec![0, 1, 2, 3, 1, 4, 2, 3, 5, 6].into(),
        kinds.into(),
        None,
    )
    .unwrap();
    Geometry::new(positions.into(), topology, Attributes::new()).unwrap()
}

fn on(p: &Geometry, domain: Domain, ids: &[u32]) -> Selection {
    let count = p.count(domain).unwrap();
    let members = Elements::from_indices(count, ids.iter().copied()).unwrap();
    Selection::on(p, domain, members, None).unwrap()
}

/// A selection of `ids` of `count` points made on another value.
fn stale(count: usize, ids: &[u32]) -> Selection {
    let members = Elements::from_indices(count, ids.iter().copied()).unwrap();
    let s = Selection::new(Domain::Point, members, None).unwrap();
    s.restored(Some([7; 32]), None).unwrap()
}

fn all(p: &Geometry, domain: Domain) -> Selection {
    Selection::on(p, domain, Elements::all(p.count(domain).unwrap()), None).unwrap()
}

fn ids(s: &Selection) -> Vec<u32> {
    s.members().iter().collect()
}

/// The edge between points `a` and `b`.
fn edge(p: &Geometry, a: u32, b: u32) -> u32 {
    p.primitives().index().edge_of(a, b).unwrap()
}

#[test]
fn a_selection_is_valid_on_its_own_topology_and_within_its_elements() {
    let p = value();
    on(&p, Domain::Point, &[0, 6]).validate(&p).unwrap();
    // Made on no particular topology: valid while it is a set of the
    // domain's elements.
    let edges = p.count(Domain::Edge).unwrap();
    let one = Elements::from_indices(edges, [1]).unwrap();
    Selection::new(Domain::Edge, one, None)
        .unwrap()
        .validate(&p)
        .unwrap();
    let e = stale(7, &[0]).validate(&p).unwrap_err();
    assert_eq!((e.kind, e.diagnostic.element), (ErrorKind::Invalid, None));
    // A set of another count is refused.
    let wide = Selection::none(Domain::Primitive, 9);
    let e = wide.validate(&p).unwrap_err();
    assert_eq!(
        e.message(),
        "a selection of 9 primitive elements on a value of 3"
    );
    let e = Selection::none(Domain::Object, 1).validate(&p).unwrap_err();
    assert_eq!(e.message(), "the value has no object domain");
}

#[test]
fn resolved_lists_the_members_and_spells_out_all() {
    let p = value();
    let some = on(&p, Domain::Point, &[2, 5]);
    assert_eq!(select::resolve(&some, &p).unwrap().as_slice(), &[2, 5]);
    let every = all(&p, Domain::Primitive);
    assert_eq!(select::resolve(&every, &p).unwrap().as_slice(), &[0, 1, 2]);
    assert_eq!(
        select::resolve(&all(&p, Domain::Detail), &p)
            .unwrap()
            .as_slice(),
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
    assert_eq!(faces.made_on(), Some(p.primitives().fingerprint()));
}

#[test]
fn all_stays_all_and_the_same_domain_is_the_selection_itself() {
    let p = value();
    let every = select::convert(&all(&p, Domain::Point), &p, Domain::Edge).unwrap();
    assert!(every.members().is_all() && every.domain() == Domain::Edge);
    let s = on(&p, Domain::Edge, &[0]);
    assert_eq!(select::convert(&s, &p, Domain::Edge).unwrap(), s);
    let e = select::convert(&s, &p, Domain::Instance).unwrap_err();
    assert_eq!(e.kind, ErrorKind::Invalid);
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
    assert!(select::convert(&stale(7, &[0]), &p, Domain::Edge).is_err());
}

/// Two quads apart (0 1 2 3, 4 5 6 7) and a lone triangle (8 9 10), the
/// triangle named like the first quad when `named`.
fn islands(named: bool) -> Geometry {
    let positions: Vec<[f64; 3]> = (0..11).map(|i| [f64::from(i), 0.0, 0.0]).collect();
    let topology = Primitives::of_polygons(
        11,
        vec![0, 4, 8, 11].into(),
        vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 10].into(),
    )
    .unwrap();
    let mut attributes = Attributes::new();
    if named {
        let text = Text::from_strs(["leaf", " ", "leaf"]);
        attributes = attributes
            .with(Domain::Primitive, names::NAME, Attribute::text(text))
            .unwrap();
    }
    Geometry::new(positions.into(), topology, attributes).unwrap()
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
    assert_eq!(grown.made_on(), Some(p.primitives().fingerprint()));
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
        grown.len(),
        7,
        "the quad's four edges and the triangle's three"
    );
    // As picked, and a selection of another value, stay as they are.
    assert_eq!(select::expand(&first, &named, &Expand::None), first);
    let stale = stale(11, &[0]);
    assert_eq!(select::expand(&stale, &named, &Expand::Connected), stale);
    assert_eq!(Mode::from(Domain::Edge).expand, Expand::None);
}

/// `value()` with point ids `ids` and, when `insert`, a new point and a
/// triangle on it placed before everything else (an insert upstream).
fn with_ids(insert: bool) -> Geometry {
    let base = value();
    let ids: Vec<i64> = (0..7).map(|i| 100 + i).collect();
    let column = |ids: Vec<i64>| Attribute::tuples(ids, Meaning::None).unwrap();
    if !insert {
        let attributes = Attributes::new()
            .with(Domain::Point, names::ID, column(ids))
            .unwrap();
        return base.with_attributes(attributes).unwrap();
    }
    // Point 0 is new (id 7); the old points move up by one.
    let mut positions = vec![[5.0, 5.0, 0.0]];
    positions.extend(base.positions().iter().copied());
    let t = base.primitives();
    let mut offsets = vec![0u32, 3];
    offsets.extend(t.offsets().as_slice()[1..].iter().map(|o| o + 3));
    let mut vertices = vec![0u32, 1, 2];
    vertices.extend(t.vertices().iter().map(|v| v + 1));
    // The new triangle first: nothing else is renumbered but the points.
    let kinds = vec![Kind::POLYGON, Kind::POLYGON, Kind::POLYGON, Kind::POLYLINE];
    let topology = Primitives::new(8, offsets.into(), vertices.into(), kinds.into(), None).unwrap();
    let mut shifted = vec![7];
    shifted.extend(ids);
    let attributes = Attributes::new()
        .with(Domain::Point, names::ID, column(shifted))
        .unwrap();
    Geometry::new(positions.into(), topology, attributes).unwrap()
}

#[test]
fn a_selection_with_ids_is_found_again_after_an_insert_upstream() {
    let before = with_ids(false);
    let members = Elements::from_indices(7, [2, 5]).unwrap();
    let s = Selection::on(&before, Domain::Point, members, Some(5)).unwrap();
    assert_eq!(s.ids().map(|ids| ids.to_vec()), Some(vec![102, 105]));
    // On its own value it is itself.
    assert_eq!(select::reresolve(&s, &before), Some(s.clone()));
    let after = with_ids(true);
    assert!(s.validate(&after).is_err());
    let found = select::reresolve(&s, &after).unwrap();
    assert_eq!((ids(&found), found.active()), (vec![3, 6], Some(6)));
    assert_eq!(found.made_on(), Some(after.primitives().fingerprint()));
    // Without ids it can't be found again.
    let plain = on(&value(), Domain::Point, &[2]);
    let moved = after.with_attributes(Attributes::new()).unwrap();
    assert_eq!(select::reresolve(&plain, &moved), None);
}

/// `n` × `m` quads with point `(i, j)` at `j * (n + 1) + i`.
fn sheet(n: u32, m: u32) -> Geometry {
    let at = |i: u32, j: u32| j * (n + 1) + i;
    let mut corners = Vec::new();
    for j in 0..m {
        for i in 0..n {
            corners.extend([at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j + 1)]);
        }
    }
    let offsets: Vec<u32> = (0..=n * m).map(|f| 4 * f).collect();
    let points = ((n + 1) * (m + 1)) as usize;
    let table = Primitives::of_polygons(points, offsets.into(), corners.into()).unwrap();
    Geometry::new(vec![[0.0; 3]; points].into(), table, Attributes::new()).unwrap()
}

#[test]
fn grow_takes_what_lies_next_to_a_selection_in_every_domain() {
    let p = sheet(3, 3);
    let t = p.primitives().clone();
    // The middle point of the sheet's second row of points.
    let point = on(&p, Domain::Point, &[5]);
    assert_eq!(ids(&select::grow(&point, &p)), [1, 4, 5, 6, 9]);
    // The middle face touches all nine by a point.
    let face = on(&p, Domain::Primitive, &[4]);
    assert_eq!(ids(&select::grow(&face, &p)), (0..9).collect::<Vec<_>>());
    let edge_id = edge(&p, 5, 6);
    let grown = select::grow(&on(&p, Domain::Edge, &[edge_id]), &p);
    let mut want = vec![edge_id];
    for (a, b) in [(1, 5), (4, 5), (5, 9), (2, 6), (6, 7), (6, 10)] {
        want.push(t.edge_of(a, b).unwrap());
    }
    want.sort_unstable();
    assert_eq!(ids(&grown), want);
    // A vertex: the corners on its point and its point's neighbours.
    let vertex = t.primitive_vertices(0).start as u32;
    let grown = select::grow(&on(&p, Domain::Vertex, &[vertex]), &p);
    let points: std::collections::BTreeSet<u32> = (grown.members().iter())
        .map(|v| t.vertices()[v as usize])
        .collect();
    assert_eq!(points.into_iter().collect::<Vec<_>>(), [0, 1, 4]);
    // The selection's active element stays; Detail doesn't grow.
    let members = Elements::from_indices(16, [5]).unwrap();
    let with_active = Selection::on(&p, Domain::Point, members, Some(5)).unwrap();
    assert_eq!(select::grow(&with_active, &p).active(), Some(5));
    let detail = Selection::on(&p, Domain::Detail, Elements::all(1), None).unwrap();
    assert_eq!(select::grow(&detail, &p), detail);
    assert_eq!(select::shrink(&detail, &p), detail);
}

#[test]
fn shrink_is_growing_the_rest_in_every_domain() {
    let p = sheet(4, 3);
    for domain in [
        Domain::Point,
        Domain::Edge,
        Domain::Primitive,
        Domain::Vertex,
    ] {
        let count = p.count(domain).unwrap() as u32;
        // A deterministic scatter of sets, none and all among them.
        for seed in 0..40u32 {
            let members: Vec<u32> = (0..count)
                .filter(|&i| seed == 0 || (i.wrapping_mul(2654435761) >> 7) % 40 < seed)
                .collect();
            let s = on(&p, domain, &members);
            let rest: Vec<u32> = (0..count).filter(|i| !members.contains(i)).collect();
            let grown_rest = select::grow(&on(&p, domain, &rest), &p);
            let want: Vec<u32> = (0..count).filter(|&i| !grown_rest.contains(i)).collect();
            let shrunk = ids(&select::shrink(&s, &p));
            if s.members().is_all() {
                assert_eq!(shrunk, members, "everything stays");
            } else {
                assert_eq!(shrunk, want, "{domain:?}, seed {seed}");
            }
        }
    }
}

#[test]
fn shrink_and_grow_leave_a_stale_selection_alone() {
    let p = sheet(2, 2);
    let s = stale(9, &[1]);
    assert_eq!(select::grow(&s, &p), s);
    assert_eq!(select::shrink(&s, &p), s);
}

#[test]
fn loop_and_ring_grow_an_edge_selection_along_its_walks() {
    let p = sheet(4, 3);
    let at = |i: u32, j: u32| j * 5 + i;
    let row = edge(&p, at(1, 1), at(2, 1));
    let s = on(&p, Domain::Edge, &[row]);
    let along = select::expand(&s, &p, &Expand::Loop);
    let mut want: Vec<u32> = (0..4).map(|i| edge(&p, at(i, 1), at(i + 1, 1))).collect();
    want.sort_unstable();
    assert_eq!(ids(&along), want);
    let across = select::expand(&s, &p, &Expand::Ring);
    let mut want: Vec<u32> = (0..4).map(|j| edge(&p, at(1, j), at(2, j))).collect();
    want.sort_unstable();
    assert_eq!(ids(&across), want);
    // Two edges of two walks: both; another domain stays as it is.
    let column = edge(&p, at(2, 1), at(2, 2));
    let both = select::expand(&on(&p, Domain::Edge, &[row, column]), &p, &Expand::Loop);
    assert_eq!(both.len(), 4 + 3);
    let face = on(&p, Domain::Primitive, &[0]);
    assert_eq!(select::expand(&face, &p, &Expand::Loop), face);
}

#[test]
fn a_boundary_is_a_regions_outline() {
    let p = sheet(3, 3);
    let t = p.primitives().clone();
    // The whole sheet ends at its border.
    let all_faces = all(&p, Domain::Primitive);
    let border = select::boundary(&all_faces, &p).unwrap();
    assert_eq!(border.len(), 12);
    // The middle face and the one beside it: their six outer sides.
    let region = on(&p, Domain::Primitive, &[4, 5]);
    let outline = select::boundary(&region, &p).unwrap();
    assert_eq!(outline.domain(), Domain::Edge);
    assert_eq!(outline.len(), 6);
    assert!(
        !outline.contains(t.edge_of(6, 10).unwrap()),
        "the side between them"
    );
    // Not of another domain, nor of another value.
    assert!(select::boundary(&on(&p, Domain::Edge, &[0]), &p).is_err());
    let other = sheet(1, 1);
    assert!(select::boundary(&region, &other).is_err());
}
