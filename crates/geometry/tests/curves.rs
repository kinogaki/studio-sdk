//! Curves: each basis's rules, knots and weights, attributes, and the
//! zero-copy view of a polygons value's polylines.
use kinogaki_geometry::limits::MAX_DEGREE;
use kinogaki_geometry::{
    Attribute, Attributes, Basis, Column, Curves, CurvesBuilder, Domain, ErrorKind, Group, Meaning,
    Polygons, Text, Topology, names,
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
        (c.curves(), c.degree(0), c.count(Domain::Vertex)),
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
        "curve 0 has degree 26, not 1 to 25"
    );
    assert_eq!(
        refused(one(Basis::BSpline, 4).degree(0)),
        "curve 0 has degree 0, not 1 to 25"
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
        .with(
            Domain::Point,
            names::WIDTH,
            Attribute::tuples(vec![0.1f32; 4], Meaning::None).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Primitive,
            names::CD,
            Attribute::tuples(vec![[1.0f32; 3]], Meaning::Color).unwrap(),
        )
        .unwrap();
    let c = one(Basis::BSpline, 4).attributes(widths).build().unwrap();
    assert_eq!(
        c.attributes()
            .tuples::<f32>(Domain::Point, names::WIDTH)
            .map(|w| w.len()),
        Some(4)
    );
    let wrong = Attributes::new()
        .with(
            Domain::Primitive,
            names::WIDTH,
            Attribute::tuples(vec![0.1f32; 2], Meaning::None).unwrap(),
        )
        .unwrap();
    assert_eq!(
        c.with_attributes(wrong).unwrap_err().message(),
        "primitive attribute width has 2 values for 1 elements"
    );
    let edges = Attributes::new()
        .with(
            Domain::Edge,
            "e",
            Attribute::tuples(vec![0.0], Meaning::None).unwrap(),
        )
        .unwrap();
    assert_eq!(
        c.with_attributes(edges).unwrap_err().message(),
        "curves have no edge domain"
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
        .with(
            Domain::Point,
            names::WIDTH,
            Attribute::tuples(vec![0.5f32; 7], Meaning::None).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Vertex,
            "t",
            Attribute::tuples((0..9).map(f64::from).collect::<Vec<_>>(), Meaning::None).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Primitive,
            names::NAME,
            Attribute::text(Text::from_strs(["quad", "a", "b"])),
        )
        .unwrap()
        .with(
            Domain::Edge,
            "crease",
            Attribute::tuples(vec![0.0; 7], Meaning::None).unwrap(),
        )
        .unwrap()
        .with_group(
            Domain::Primitive,
            names::HIDDEN,
            Group::from_indices(3, [2]).unwrap(),
        )
        .unwrap();
    let polygons = Polygons::new(positions.into(), t, attributes).unwrap();
    let curves = Curves::from_polylines(&polygons);
    assert_eq!((curves.curves(), curves.basis()), (2, Basis::Linear));
    assert!(curves.positions().ptr_eq(polygons.positions()));
    let points = curves.points().unwrap();
    // A slice of the polygons' own vertex buffer.
    assert!(
        polygons
            .topology()
            .vertices()
            .as_ptr_range()
            .contains(&points.as_ptr())
    );
    assert_eq!(&**points, &[0, 4, 5, 5, 6]);
    assert_eq!(&**curves.offsets(), &[0, 3, 5]);
    let a = curves.attributes();
    assert!(std::ptr::eq(
        a.get(Domain::Point, names::WIDTH).unwrap(),
        polygons
            .attributes()
            .get(Domain::Point, names::WIDTH)
            .unwrap()
    ));
    let t = a.tuples::<f64>(Domain::Vertex, "t").unwrap();
    assert_eq!(t, &[4.0, 5.0, 6.0, 7.0, 8.0]);
    let shared = polygons
        .attributes()
        .tuples::<f64>(Domain::Vertex, "t")
        .unwrap();
    assert!(shared.as_ptr_range().contains(&t.as_ptr()));
    let names = a.text(Domain::Primitive, names::NAME).unwrap();
    assert_eq!((names.get(0), names.get(1)), (Some("a"), Some("b")));
    assert!(a.get(Domain::Edge, "crease").is_none());
    let hidden = a.group(Domain::Primitive, names::HIDDEN).unwrap();
    assert_eq!(hidden.iter().collect::<Vec<_>>(), [1]);
    assert_eq!(Curves::from_polylines(&Polygons::empty()).curves(), 0);
}

#[test]
fn bases_name_themselves() {
    for b in Basis::ALL {
        assert_eq!(Basis::from_name(b.name()), Some(b));
    }
}

mod display {
    use kinogaki_geometry::{Basis, Curves, Group};
    use std::sync::Arc;

    /// The points a display samples each span with.
    const SAMPLES: usize = 16;

    fn curve(basis: Basis, points: Vec<[f64; 3]>, degree: u8, periodic: bool) -> Curves {
        let n = points.len() as u32;
        let mut builder = Curves::builder(basis, points.into(), vec![0, n].into()).degree(degree);
        if periodic {
            builder = builder.periodic(Group::from_indices(1, [0]).unwrap());
        }
        builder.build().unwrap()
    }

    fn close(
        a: impl Into<kinogaki_geometry::Vec3>,
        b: impl Into<kinogaki_geometry::Vec3>,
        tolerance: f64,
    ) -> bool {
        let d = a.into() - b.into();
        [d.x, d.y, d.z].iter().all(|c| c.abs() <= tolerance)
    }

    #[test]
    fn a_rational_quadratic_is_exactly_a_circle() {
        let w = std::f64::consts::FRAC_1_SQRT_2;
        let quarter = Curves::builder(
            Basis::Nurbs,
            vec![[1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 1.0, 0.0]].into(),
            vec![0, 3].into(),
        )
        .degree(2)
        .knots(vec![0, 6].into(), vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0].into())
        .weights(vec![1.0, w, 1.0].into())
        .build()
        .unwrap();
        assert_eq!(quarter.domain(0), [0.0, 1.0]);
        for k in 0..=100 {
            let p = quarter.point(0, k as f64 / 100.0);
            assert!((p.x.hypot(p.y) - 1.0).abs() < 1e-12, "{p:?}");
        }
        let display = quarter.display();
        assert_eq!(display.positions().len(), SAMPLES + 1);
        assert!(
            display
                .positions()
                .iter()
                .all(|p| (p[0].hypot(p[1]) - 1.0).abs() < 1e-12)
        );
    }

    #[test]
    fn bezier_ends_on_its_end_points_and_follows_bernstein() {
        let p = vec![
            [0.0, 0.0, 0.0],
            [1.0, 2.0, 0.0],
            [3.0, 2.0, 0.0],
            [4.0, 0.0, 0.0],
        ];
        let cubic = curve(Basis::Bezier, p.clone(), 3, false);
        assert_eq!(cubic.domain(0), [0.0, 1.0]);
        assert_eq!(cubic.point(0, 0.0), p[0].into());
        assert_eq!(cubic.point(0, 1.0), p[3].into());
        // At t = ½: (p0 + 3 p1 + 3 p2 + p3) / 8.
        let half: [f64; 3] =
            std::array::from_fn(|c| (p[0][c] + 3.0 * p[1][c] + 3.0 * p[2][c] + p[3][c]) / 8.0);
        assert!(close(cubic.point(0, 0.5), half, 1e-12));
    }

    #[test]
    fn an_open_bspline_is_clamped_and_a_straight_one_straight() {
        let line: Vec<[f64; 3]> = (0..6).map(|k| [k as f64, 2.0 * k as f64, 0.0]).collect();
        let spline = curve(Basis::BSpline, line.clone(), 3, false);
        assert_eq!(spline.domain(0), [0.0, 3.0]);
        assert!(close(spline.point(0, 0.0), line[0], 1e-12));
        assert!(close(spline.point(0, 3.0), line[5], 1e-12));
        for k in 0..=30 {
            let p = spline.point(0, k as f64 / 10.0);
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
        let open = curve(Basis::CatmullRom, points.clone(), 3, false);
        assert_eq!(open.domain(0), [0.0, 4.0]);
        for (k, p) in points.iter().enumerate() {
            assert!(close(open.point(0, k as f64), *p, 1e-12), "point {k}");
        }
        let closed = curve(Basis::CatmullRom, points.clone(), 3, true);
        assert_eq!(closed.domain(0), [0.0, 5.0]);
        assert!(close(closed.point(0, 5.0), points[0], 1e-12), "it closes");
    }

    #[test]
    fn periodic_curves_close_and_linear_curves_show_their_points() {
        let square = vec![
            [0.0, 0.0, 0.0],
            [1.0, 0.0, 0.0],
            [1.0, 1.0, 0.0],
            [0.0, 1.0, 0.0],
        ];
        let ring = curve(Basis::BSpline, square.clone(), 3, true);
        assert_eq!(ring.domain(0), [0.0, 4.0]);
        assert!(
            close(ring.point(0, 0.0), ring.point(0, 4.0), 1e-12),
            "the ends meet"
        );
        let display = ring.display();
        assert_eq!(display.positions().len(), 4 * SAMPLES, "no repeated end");
        let segments: Vec<[u32; 2]> = display.segments(0).collect();
        assert_eq!(
            segments.len(),
            display.positions().len(),
            "closing segment included"
        );
        let polyline = curve(Basis::Linear, square.clone(), 1, false);
        let display = polyline.display();
        assert_eq!(display.positions().as_slice(), square.as_slice());
    }

    #[test]
    fn the_display_is_kept_and_follows_moved_points_in_place() {
        let points: Vec<[f64; 3]> = (0..7).map(|k| [k as f64, (k % 2) as f64, 0.0]).collect();
        let first = Curves::builder(Basis::BSpline, points.clone().into(), vec![0, 4, 7].into())
            .degree(2)
            .build()
            .unwrap();
        let shown = first.display();
        assert!(Arc::ptr_eq(&shown, &first.display()), "kept");
        let mut moved = points;
        moved[5][1] += 3.0;
        let after = first.with_positions(moved.into()).unwrap();
        let patched = after.display_after(&first, &[5]);
        assert_eq!(
            *patched,
            *after
                .with_positions(after.positions().clone())
                .unwrap()
                .display()
        );
        assert_eq!(
            patched.positions().len(),
            shown.positions().len(),
            "the layout holds"
        );
    }

    /// Set when dropped: a cache slot that tells whether its column is gone.
    struct Dropped(Arc<std::sync::atomic::AtomicBool>);

    impl Drop for Dropped {
        fn drop(&mut self) {
            self.0.store(true, std::sync::atomic::Ordering::SeqCst);
        }
    }

    /// A flag set once `positions` (and every clone of it) is freed.
    fn watch(
        positions: &kinogaki_geometry::Column<[f64; 3]>,
    ) -> Arc<std::sync::atomic::AtomicBool> {
        let freed = Arc::new(std::sync::atomic::AtomicBool::new(false));
        positions.derived(|_| Dropped(freed.clone()));
        freed
    }

    #[test]
    fn a_display_never_keeps_its_positions_alive() {
        let freed = std::sync::atomic::Ordering::SeqCst;
        let points: Vec<[f64; 3]> = (0..7).map(|k| [k as f64, (k % 2) as f64, 0.0]).collect();
        let first = Curves::builder(Basis::BSpline, points.clone().into(), vec![0, 4, 7].into())
            .degree(2)
            .build()
            .unwrap();
        let shown = first.display();
        let mut moved = points;
        moved[5][1] += 3.0;
        let after = first.with_positions(moved.into()).unwrap();
        let patched = after.display_after(&first, &[5]);
        let (first_freed, after_freed) = (watch(first.positions()), watch(after.positions()));
        drop((first, after));
        assert!(first_freed.load(freed), "display() kept its positions");
        assert!(
            after_freed.load(freed),
            "display_after() kept its positions"
        );
        // The displays themselves are the caller's, still whole.
        assert_eq!(shown.positions().len(), patched.positions().len());
    }
}

#[test]
fn moved_points_are_of_one_structure() {
    let curve = one(Basis::Bezier, 4).degree(3).build().unwrap();
    assert_eq!(curve.moved_points(&curve), Some(Vec::new()));
    let mut at = curve.positions().to_vec();
    at[2][1] = 1.0;
    at[0][1] = -0.0;
    let moved = curve.with_positions(at.into()).unwrap();
    assert_eq!(moved.moved_points(&curve), Some(vec![0, 2]), "bitwise");
    // Another structure over the same points: no move of it.
    let other = one(Basis::Linear, 4).build().unwrap();
    assert_eq!(other.moved_points(&curve), None);
}
