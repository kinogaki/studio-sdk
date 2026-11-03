//! Selection, Detail, DisplayQuality and Tolerances: their rules.
use kinogaki_geometry::{Class, Detail, DisplayQuality, ErrorKind, Selection, Tolerances};

#[test]
fn selections_are_sorted_distinct_and_hold_their_active_element() {
    let s = Selection::new(Class::Primitive, vec![5, 1, 5, 3], Some(5), None).unwrap();
    assert_eq!(&**s.indices(), &[1, 3, 5]);
    assert_eq!(
        (s.class(), s.active(), s.len(10), s.is_all()),
        (Class::Primitive, Some(5), 3, false)
    );
    let e = Selection::new(Class::Point, vec![1, 2], Some(4), None).unwrap_err();
    assert_eq!((e.kind(), e.element()), (ErrorKind::Selection, Some(4)));
    assert_eq!(e.message(), "the active point 4 is not selected");
    let all = Selection::all(Class::Edge, None);
    assert!(all.contains(1_000) && all.len(7) == 7 && !all.is_empty());
    assert!(Selection::none(Class::Point).is_empty());
}

#[test]
fn render_detail_is_in_range() {
    assert_eq!(Detail::default(), Detail::Viewport(DisplayQuality::Medium));
    for q in DisplayQuality::ALL {
        assert!(Detail::Viewport(q).check().is_ok());
    }
    assert!(Detail::render(0.001, 90.0).is_ok());
    for (chord, angle) in [
        (0.0, 5.0),
        (-1.0, 5.0),
        (f64::NAN, 5.0),
        (0.1, 0.0),
        (0.1, 91.0),
    ] {
        assert_eq!(
            Detail::render(chord, angle).unwrap_err().kind(),
            ErrorKind::Range
        );
    }
    let direct = Detail::Render {
        chord: 0.1,
        angle: 200.0,
    };
    assert!(direct.check().is_err());
}

#[test]
fn tolerances_are_in_their_ranges() {
    let d = Tolerances::default();
    assert_eq!(
        (
            d.positional_mm(),
            d.fit_mm(),
            d.tangent_degrees(),
            d.curvature(),
            d.millimeters_per_unit()
        ),
        (0.01, 0.001, 0.1, 0.005, 1.0)
    );
    let ok = |p, f, t, k, u| Tolerances::new(p, f, t, k, u).is_ok();
    assert!(ok(1e-5, 1e-8, 0.001, 1e-4, 1.0) && ok(1.0, 10.0, 5.0, 0.1, 1000.0));
    let message = |p, f, t, k, u| {
        Tolerances::new(p, f, t, k, u)
            .unwrap_err()
            .message()
            .to_owned()
    };
    assert_eq!(
        message(0.01, 0.001, 0.1, 0.005, 0.0),
        "a model unit must be a finite, positive length"
    );
    assert_eq!(
        message(2e-6, 1e-8, 0.1, 0.005, 1.0),
        "the positional tolerance must be between 0.00001 mm and 1 mm"
    );
    assert_eq!(
        message(0.01, 0.2, 0.1, 0.005, 1.0),
        "the fit tolerance must be between a thousandth of the positional tolerance and ten times it"
    );
    assert_eq!(
        message(0.01, 0.001, 6.0, 0.005, 1.0),
        "the tangent tolerance must be between 0.001° and 5°"
    );
    assert_eq!(
        message(0.01, 0.001, 0.1, 0.5, 1.0),
        "the curvature tolerance must be between 0.01 % and 10 %"
    );
    let inches = d.with_unit(25.4).unwrap();
    assert!((inches.fit() - 0.001 / 25.4).abs() < 1e-18 && inches.positional_mm() == 0.01);
}
