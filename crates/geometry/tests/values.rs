//! Selection, Detail, DisplayQuality and Tolerances: their rules.
use kinogaki_geometry::{
    Detail, DisplayQuality, Domain, Elements, ErrorKind, Selection, Tolerances, Units, UpAxis,
};

#[test]
fn selections_are_sorted_distinct_and_hold_their_active_element() {
    let members = Elements::from_indices(10, [5, 1, 5, 3]).unwrap();
    let s = Selection::new(Domain::Primitive, members, Some(5)).unwrap();
    assert_eq!(s.members().iter().collect::<Vec<_>>(), [1, 3, 5]);
    assert_eq!(
        (s.domain(), s.active(), s.len(), s.members().is_all()),
        (Domain::Primitive, Some(5), 3, false)
    );
    let two = Elements::from_indices(10, [1, 2]).unwrap();
    let e = Selection::new(Domain::Point, two.clone(), Some(4)).unwrap_err();
    assert_eq!(
        (e.kind, e.diagnostic.element),
        (ErrorKind::Invalid, Some(4))
    );
    assert_eq!(e.message(), "the active point 4 is not selected");
    let all = Selection::new(Domain::Edge, Elements::all(7), None).unwrap();
    assert!(all.contains(6) && !all.contains(7) && all.len() == 7 && !all.is_empty());
    assert!(Selection::none(Domain::Point, 3).is_empty());
    // Ids are one per member.
    let s = Selection::new(Domain::Point, two, None).unwrap();
    assert!(s.clone().restored(None, Some(vec![4, 5].into())).is_ok());
    assert!(s.restored(None, Some(vec![4].into())).is_err());
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
            Detail::render(chord, angle).unwrap_err().kind,
            ErrorKind::Invalid
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
            d.units().millimeters_per_unit()
        ),
        (0.01, 0.001, 0.1, 0.005, 1.0)
    );
    let mm = d.units();
    let ok = |p, f, t, k| Tolerances::new(p, f, t, k, mm).is_ok();
    assert!(ok(1e-5, 1e-8, 0.001, 1e-4) && ok(1.0, 10.0, 5.0, 0.1));
    let message = |p, f, t, k| {
        Tolerances::new(p, f, t, k, mm)
            .unwrap_err()
            .message()
            .to_owned()
    };
    assert_eq!(
        message(2e-6, 1e-8, 0.1, 0.005),
        "the positional tolerance must be between 0.00001 mm and 1 mm"
    );
    assert_eq!(
        message(0.01, 0.2, 0.1, 0.005),
        "the fit tolerance must be between a thousandth of the positional tolerance and ten times it"
    );
    assert_eq!(
        message(0.01, 0.001, 6.0, 0.005),
        "the tangent tolerance must be between 0.001° and 5°"
    );
    assert_eq!(
        message(0.01, 0.001, 0.1, 0.5),
        "the curvature tolerance must be between 0.01 % and 10 %"
    );
    let inches = d.with_units(Units::new(0.0254, UpAxis::Y).unwrap());
    assert!((inches.fit() - 0.001 / 25.4).abs() < 1e-18 && inches.positional_mm() == 0.01);
    assert!(Units::new(0.0, UpAxis::Y).is_err() && Units::new(f64::NAN, UpAxis::Z).is_err());
}
