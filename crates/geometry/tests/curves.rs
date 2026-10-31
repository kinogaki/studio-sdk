//! Curves: each basis's rules, knots and weights, attributes, and the
//! zero-copy view of a polygons value's polylines.
use kinogaki_geometry::limits::MAX_DEGREE;
use kinogaki_geometry::{
    Attribute, Attributes, Basis, Class, Column, Curves, CurvesBuilder, ErrorKind, Group, Polygons,
    Text, Topology, TypeInfo, names,
};
use std::sync::Arc;

/// A builder for one curve of `count` control points.
fn one(basis: Basis, count: usize) -> CurvesBuilder {
    let points: Vec<[f64; 3]> = (0..count).map(|i| [i as f64, 0.0, 0.0]).collect();
    Curves::builder(basis, points.into(), vec![0, count as u32].into())
}

fn refused(builder: CurvesBuilder) -> String {
    let e = builder.build().unwrap_err();
    assert_eq!(e.kind(), ErrorKind::Curve, "{}", e.message());
    e.message().to_owned()
}

#[test]
fn linear_curves_are_degree_one_with_two_points() {
    let c = one(Basis::Linear, 2).build().unwrap();
    assert_eq!(
        (c.curves(), c.degree(0), c.count(Class::Vertex)),
        (1, 1, Some(2))
    );
    assert_eq!(
        refused(one(Basis::Linear, 1)),
        "curve 0 has 1 control points, fewer than 2"
    );
    assert_eq!(
        refused(one(Basis::Linear, 3).degree(2)),
        "curve 0 is linear but of degree 2"
    );
}

#[test]
fn bezier_counts_are_whole_segments() {
    for (count, ok) in [(4, true), (7, true), (5, false), (3, false)] {
        assert_eq!(
            one(Basis::Bezier, count).degree(3).build().is_ok(),
            ok,
            "{count}"
        );
    }
    assert_eq!(
        refused(one(Basis::Bezier, 5).degree(3)),
        "curve 0 has 5 control points, not k·degree + 1 for degree 3"
    );
    let closed = |count| {
        one(Basis::Bezier, count)
            .degree(3)
            .periodic(Group::from_indices(1, [0]).unwrap())
    };
    assert!(closed(6).build().is_ok());
    assert_eq!(
        refused(closed(7)),
        "curve 0 has 7 control points, not k·degree for degree 3"
    );
    assert!(one(Basis::Bezier, 3).degree(2).build().is_ok());
}

#[test]
fn bsplines_need_degree_plus_one_points() {
    assert!(one(Basis::BSpline, 4).build().is_ok(), "cubic by default");
    assert_eq!(
        refused(one(Basis::BSpline, 3)),
        "curve 0 has 3 control points, fewer than 4"
    );
    assert!(one(Basis::BSpline, 6).degree(5).build().is_ok());
    assert_eq!(
        refused(one(Basis::BSpline, 20).degree(MAX_DEGREE + 1)),
        "curve 0 has degree 16, not 1 to 15"
    );
    assert_eq!(
        refused(one(Basis::BSpline, 4).degree(0)),
        "curve 0 has degree 0, not 1 to 15"
    );
}

#[test]
fn catmull_rom_is_cubic() {
    assert!(one(Basis::CatmullRom, 2).build().is_ok());
    assert_eq!(
        refused(one(Basis::CatmullRom, 4).degree(2)),
        "curve 0 is Catmull-Rom but of degree 2, not 3"
    );
}

#[test]
fn nurbs_need_knots_and_positive_weights() {
    let knots = |values: Vec<f64>| {
        (
            Column::new(vec![0, values.len() as u32]),
            Column::new(values),
        )
    };
    let clamped = || knots(vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]);
    let nurbs = |(o, v): (Column<u32>, Column<f64>)| one(Basis::Nurbs, 4).knots(o, v);
    assert!(nurbs(clamped()).build().is_ok());
    assert!(
        nurbs(clamped())
            .weights(vec![1.0, 0.5, 0.5, 1.0].into())
            .build()
            .is_ok()
    );
    assert_eq!(refused(one(Basis::Nurbs, 4)), "NURBS curves need knots");
    assert_eq!(
        refused(nurbs(knots(vec![0.0; 7]))),
        "curve 0 has 7 knots, not 8 (count + degree + 1)"
    );
    assert_eq!(
        refused(nurbs(knots(vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.5, 1.0, 1.0]))),
        "curve 0's knots decrease or are not finite"
    );
    assert_eq!(
        refused(nurbs(knots(vec![0.0; 8]))),
        "curve 0's knots span no interval"
    );
    let e = nurbs(clamped())
        .weights(vec![1.0, 0.0, 1.0, 1.0].into())
        .build()
        .unwrap_err();
    assert_eq!(
        (e.element(), e.message()),
        (Some(1), "vertex 1's weight 0 is not above 0")
    );
    assert_eq!(
        refused(nurbs(clamped()).weights(vec![1.0; 3].into())),
        "3 weights for 4 vertices"
    );
    let (o, v) = clamped();
    assert_eq!(
        refused(one(Basis::BSpline, 4).knots(o, v)),
        "bspline curves take no knots"
    );
    assert_eq!(
        refused(one(Basis::BSpline, 4).weights(vec![1.0; 4].into())),
        "bspline curves take no weights"
    );
}

#[test]
fn offsets_points_degrees_and_periodic_cover_the_curves() {
    let p: Column<[f64; 3]> = vec![[0.0; 3], [1.0, 0.0, 0.0], [2.0, 0.0, 0.0]].into();
    let build = |offsets: Vec<u32>| Curves::builder(Basis::Linear, p.clone(), offsets.into());
    assert_eq!(refused(build(vec![1, 3])), "the offsets must start at 0");
    assert_eq!(
        refused(build(vec![0, 2])),
        "the offsets end at 2, not at the vertex count 3"
    );
    let shared = build(vec![0, 2, 4])
        .points(vec![0, 1, 1, 2].into())
        .build()
        .unwrap();
    assert_eq!((shared.curves(), shared.point_of(2)), (2, 1));
    let e = build(vec![0, 2])
        .points(vec![0, 3].into())
        .build()
        .unwrap_err();
    assert_eq!(
        (e.element(), e.message()),
        (Some(1), "vertex 1 names point 3, which is missing")
    );
    assert_eq!(
        refused(build(vec![0, 3]).degrees(vec![1, 1].into())),
        "2 degrees for 1 curves"
    );
    assert_eq!(
        refused(build(vec![0, 3]).periodic(Group::none(2))),
        "the periodic group is over 2 curves, not 1"
    );
    let far = Curves::builder(
        Basis::Linear,
        vec![[f64::NAN; 3]; 2].into(),
        vec![0, 2].into(),
    );
    assert_eq!(far.build().unwrap_err().kind(), ErrorKind::Position);
    assert_eq!(Curves::empty().curves(), 0);
}

#[test]
fn attributes_are_sized_to_points_vertices_and_curves() {
    let widths = Attributes::new()
        .with(Class::Point, names::WIDTH, Attribute::float(vec![0.1; 4]))
        .unwrap()
        .with(
            Class::Primitive,
            names::CD,
            Attribute::vector(vec![[1.0; 3]], TypeInfo::Color),
        )
        .unwrap();
    let c = one(Basis::BSpline, 4).attributes(widths).build().unwrap();
    assert_eq!(
        c.attributes()
            .float(Class::Point, names::WIDTH)
            .map(|w| w.len()),
        Some(4)
    );
    let wrong = Attributes::new()
        .with(
            Class::Primitive,
            names::WIDTH,
            Attribute::float(vec![0.1; 2]),
        )
        .unwrap();
    assert_eq!(
        c.with_attributes(wrong).unwrap_err().message(),
        "primitive attribute width has 2 values for 1 elements"
    );
    let edges = Attributes::new()
        .with(Class::Edge, "e", Attribute::float(vec![0.0]))
        .unwrap();
    assert_eq!(
        c.with_attributes(edges).unwrap_err().message(),
        "curves have no edge class"
    );
    let moved = c.with_positions(vec![[1.0; 3]; 4].into()).unwrap();
    assert!(Arc::ptr_eq(moved.attributes(), c.attributes()) && moved.degrees().ptr_eq(c.degrees()));
    assert!(c.with_positions(vec![[1.0; 3]; 3].into()).is_err());
}

#[test]
fn from_polylines_shares_the_buffers() {
    // A quad, then two polylines: 0-4-5 and 5-6.
    let t = Topology::new(
        7,
        vec![0, 4, 7, 9].into(),
        vec![0, 1, 2, 3, 0, 4, 5, 5, 6].into(),
        1,
    )
    .unwrap();
    let positions: Vec<[f64; 3]> = (0..7).map(|i| [i as f64, 0.0, 0.0]).collect();
    let attributes = Attributes::new()
        .with(Class::Point, names::WIDTH, Attribute::float(vec![0.5; 7]))
        .unwrap()
        .with(
            Class::Vertex,
            "t",
            Attribute::float((0..9).map(f64::from).collect::<Vec<_>>()),
        )
        .unwrap()
        .with(
            Class::Primitive,
            names::NAME,
            Attribute::text(Text::from_strs(["quad", "a", "b"])),
        )
        .unwrap()
        .with(Class::Piece, "piece", Attribute::integer(vec![7, 8, 9]))
        .unwrap()
        .with(Class::Edge, "crease", Attribute::float(vec![0.0; 7]))
        .unwrap()
        .with_group(
            Class::Primitive,
            names::HIDDEN,
            Group::from_indices(3, [2]).unwrap(),
        )
        .unwrap();
    let polygons = Polygons::new(positions.into(), t, attributes).unwrap();
    let curves = Curves::from_polylines(&polygons);
    assert_eq!((curves.curves(), curves.basis()), (2, Basis::Linear));
    assert!(curves.positions().ptr_eq(polygons.positions()));
    let points = curves.points().unwrap();
    assert!(points.shares_buffer(polygons.topology().vertices()));
    assert_eq!(&**points, &[0, 4, 5, 5, 6]);
    assert_eq!(&**curves.offsets(), &[0, 3, 5]);
    let a = curves.attributes();
    assert!(Arc::ptr_eq(
        a.get_shared(Class::Point, names::WIDTH).unwrap(),
        polygons
            .attributes()
            .get_shared(Class::Point, names::WIDTH)
            .unwrap()
    ));
    let t = a.float(Class::Vertex, "t").unwrap();
    assert_eq!(&**t, &[4.0, 5.0, 6.0, 7.0, 8.0]);
    assert!(t.shares_buffer(polygons.attributes().float(Class::Vertex, "t").unwrap()));
    let names = a.text(Class::Primitive, names::NAME).unwrap();
    assert_eq!((names.get(0), names.get(1)), (Some("a"), Some("b")));
    assert_eq!(
        &**a.integer(Class::Primitive, "piece").unwrap(),
        &[8, 9],
        "Piece read as Primitive"
    );
    assert!(a.get(Class::Edge, "crease").is_none());
    let hidden = a.group(Class::Primitive, names::HIDDEN).unwrap();
    assert_eq!(hidden.iter().collect::<Vec<_>>(), [1]);
    assert_eq!(Curves::from_polylines(&Polygons::empty()).curves(), 0);
}

#[test]
fn bases_name_themselves() {
    for b in Basis::ALL {
        assert_eq!(Basis::from_name(b.name()), Some(b));
    }
}
