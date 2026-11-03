//! Geometry: positions and attribute rules, counts, and the sharing every
//! edit keeps.
use kinogaki_geometry::limits::MAX_POINTS;
use kinogaki_geometry::{
    Attribute, Attributes, Bounds, Domain, Elements, ErrorKind, Geometry, Kind, Meaning,
    Primitives, Text, Vec3, names,
};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// Two triangles sharing a side, then a polyline: 5 points, 3
/// primitives, 8 vertices, 6 edges.
fn topology() -> Primitives {
    Primitives::new(
        5,
        vec![0, 3, 6, 8].into(),
        vec![0, 1, 2, 0, 2, 3, 3, 4].into(),
        vec![Kind::POLYGON, Kind::POLYGON, Kind::POLYLINE].into(),
        None,
    )
    .unwrap()
}

fn positions() -> Vec<[f64; 3]> {
    vec![
        [0.0, 0.0, 0.0],
        [1.0, 0.0, 0.0],
        [1.0, 1.0, 0.0],
        [0.0, 1.0, 0.0],
        [0.0, 2.0, 1.0],
    ]
}

fn value() -> Geometry {
    Geometry::new(positions().into(), topology(), Attributes::new()).unwrap()
}

#[test]
fn counts_are_the_domains_elements() {
    let p = value();
    let counts = Domain::ALL.map(|c| p.count(c));
    assert_eq!(
        counts,
        [Some(1), None, None, Some(3), Some(6), Some(5), Some(8)]
    );
    assert_eq!(
        p.bounds(),
        Bounds::of([Vec3::ZERO, Vec3::new(1.0, 2.0, 1.0)])
    );
    assert_eq!(Geometry::empty().bounds(), None);
    assert_eq!(Geometry::empty().count(Domain::Point), Some(0));
}

#[test]
fn positions_must_be_finite_and_one_a_point() {
    let with = |change: fn(&mut Vec<[f64; 3]>)| {
        let mut p = positions();
        change(&mut p);
        Geometry::new(p.into(), topology(), Attributes::new())
    };
    assert!(with(|p| p[3][1] = 1e100).is_ok());
    for bad in [f64::NAN, f64::INFINITY, -1.1e100] {
        let e = Geometry::new(
            {
                let mut p = positions();
                p[3][2] = bad;
                p.into()
            },
            topology(),
            Attributes::new(),
        )
        .unwrap_err();
        assert_eq!(
            (e.kind, e.diagnostic.element),
            (ErrorKind::Invalid, Some(3))
        );
        assert_eq!(e.message(), "point 3 is not finite or is beyond 1e100");
    }
    let e = with(|p| {
        p.pop();
    })
    .unwrap_err();
    assert_eq!(e.message(), "4 positions for a table of 5 points");
    let far = value()
        .with_positions(vec![[0.0; 3]; 6].into())
        .unwrap_err();
    assert_eq!(far.message(), "6 positions for a table of 5 points");
}

#[test]
fn the_first_bad_position_is_reported_on_large_values() {
    let mut p = vec![[0.0; 3]; 300_000];
    p[250_000][0] = f64::NAN;
    p[200_001][1] = f64::INFINITY;
    let t = Primitives::of_polygons(300_000, vec![0].into(), Vec::new().into()).unwrap();
    let e = Geometry::new(p.into(), t, Attributes::new()).unwrap_err();
    assert_eq!(e.diagnostic.element, Some(200_001));
}

#[test]
fn the_first_bad_attribute_value_is_reported_on_large_values() {
    let n = 300_000;
    let mut w = vec![0.0; n];
    w[250_000] = f64::NAN;
    w[200_001] = f64::NEG_INFINITY;
    let mut dir = vec![[0.0f32; 3]; n];
    dir[299_999][2] = f32::INFINITY;
    dir[131_072][1] = f32::NAN;
    let refused = [
        (Attribute::tuples(w, Meaning::None), 200_001, 200_001),
        (
            Attribute::tuples(dir, Meaning::None),
            131_072,
            131_072 * 3 + 1,
        ),
    ];
    for (attribute, element, value) in refused {
        let e = attribute.unwrap_err();
        assert_eq!(e.diagnostic.element, Some(element));
        assert_eq!(e.message(), format!("value {value} is not finite"));
    }
}

#[test]
fn one_past_the_point_limit_is_refused() {
    let t = Primitives::of_polygons(MAX_POINTS + 1, vec![0].into(), Vec::new().into());
    assert_eq!(t.unwrap_err().kind, ErrorKind::Invalid);
}

#[test]
fn attributes_are_sized_to_their_domain() {
    let p = value();
    let sized = Attributes::new()
        .with(
            Domain::Point,
            names::CD,
            Attribute::tuples(vec![[1.0f32; 3]; 5], Meaning::Color).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Vertex,
            names::UV,
            Attribute::tuples(vec![[0.0f32; 2]; 8], Meaning::TextureCoord).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Primitive,
            names::MATERIAL,
            Attribute::text(Text::from_strs(["/A", "", "/B"])),
        )
        .unwrap()
        .with(
            Domain::Primitive,
            "piece",
            Attribute::tuples::<i64>(vec![0, 0, 1], Meaning::None).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Edge,
            "crease",
            Attribute::tuples(vec![0.0; 6], Meaning::None).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Detail,
            "factor",
            Attribute::tuples(vec![1.0], Meaning::None).unwrap(),
        )
        .unwrap()
        .with_group(
            Domain::Edge,
            names::SHARP,
            Elements::from_indices(6, [1]).unwrap(),
        )
        .unwrap()
        .with_group(Domain::Primitive, names::HIDDEN, Elements::none(3))
        .unwrap();
    let ok = p.with_attributes(sized).unwrap();
    assert!(Arc::ptr_eq(ok.primitives(), p.primitives()) && ok.positions().ptr_eq(p.positions()));
    let wrong = Attributes::new()
        .with(
            Domain::Vertex,
            "w",
            Attribute::tuples(vec![0.0; 7], Meaning::None).unwrap(),
        )
        .unwrap();
    assert_eq!(
        p.with_attributes(wrong).unwrap_err().message(),
        "vertex attribute w has 7 values for 8 elements"
    );
    let group = Attributes::new()
        .with_group(Domain::Edge, names::SHARP, Elements::none(5))
        .unwrap();
    assert_eq!(
        p.with_attributes(group).unwrap_err().message(),
        "edge group sharp has 5 values for 6 elements"
    );
    for domain in [Domain::Object, Domain::Instance] {
        let refused = Attributes::new()
            .with(
                domain,
                "x",
                Attribute::tuples(vec![0.0], Meaning::None).unwrap(),
            )
            .unwrap();
        let e = p.with_attributes(refused).unwrap_err();
        assert_eq!(
            e.message(),
            format!("polygons have no {} domain", domain.name())
        );
    }
}

#[test]
fn with_positions_shares_the_topology_the_attributes_and_their_caches() {
    let attributes = Attributes::new()
        .with(
            Domain::Point,
            "w",
            Attribute::tuples(vec![1.0; 5], Meaning::None).unwrap(),
        )
        .unwrap();
    let a = value().with_attributes(attributes).unwrap();
    let builds = AtomicUsize::new(0);
    let cache = |p: &Geometry| {
        p.primitives().derived(|t| {
            builds.fetch_add(1, Ordering::Relaxed);
            t.len()
        })
    };
    cache(&a);
    let b = a.with_positions(vec![[2.0; 3]; 5].into()).unwrap();
    let c = b.with_positions(positions().into()).unwrap();
    assert!(Arc::ptr_eq(a.primitives(), c.primitives()));
    assert!(Arc::ptr_eq(a.attributes(), c.attributes()));
    assert!(Arc::ptr_eq(&cache(&a), &cache(&c)));
    assert_eq!(builds.load(Ordering::Relaxed), 1);
    assert_eq!(
        a.primitives().index() as *const _,
        c.primitives().index() as *const _
    );
    assert_eq!(c, a, "the same positions again: an equal value");
    assert_eq!(
        b.bounds(),
        Some(Bounds::point(Vec3::new(2.0, 2.0, 2.0))),
        "bounds follow the positions"
    );
}

#[test]
fn with_primitives_reuses_a_checked_topology() {
    let a = value();
    let b = Geometry::with_primitives(
        vec![[3.0; 3]; 5].into(),
        a.primitives().clone(),
        a.attributes().clone(),
    )
    .unwrap();
    assert!(Arc::ptr_eq(a.primitives(), b.primitives()));
    let wrong = Arc::new(
        Attributes::new()
            .with(
                Domain::Primitive,
                "w",
                Attribute::tuples(vec![0.0; 2], Meaning::None).unwrap(),
            )
            .unwrap(),
    );
    assert!(Geometry::with_primitives(positions().into(), a.primitives().clone(), wrong).is_err());
}

#[test]
fn moved_points_are_those_whose_bits_changed() {
    let p = value();
    assert!(p.moved_points(&p).is_empty(), "one column moved nothing");
    assert!(p.ptr_eq(&p.clone()));
    let mut at = positions();
    at[1][2] = 0.5;
    // -0.0 is another position's bytes; the same bits anew are not a move.
    at[3][0] = -0.0;
    let moved = p.with_positions(at.clone().into()).unwrap();
    assert_eq!(moved.moved_points(&p), [1, 3]);
    assert!(!moved.ptr_eq(&p), "other positions");
    let again = p.with_positions(positions().into()).unwrap();
    assert!(
        again.moved_points(&p).is_empty(),
        "equal bits, another column"
    );
    assert!(!again.ptr_eq(&p));
    // Back from -0.0 to 0.0 is a move too, either way round.
    assert_eq!(p.moved_points(&moved), [1, 3]);
}
