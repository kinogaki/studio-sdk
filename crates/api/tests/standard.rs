//! The standard data types: handles, codecs, the tolerances and display
//! quality values and the conformance suite.
use kinogaki_studio_api::conformance::{self, Codecs, Consumer, Fixtures, Invariants};
use kinogaki_studio_api::element::{Field, MemoryElement};
use kinogaki_studio_api::geometry::{Class, Polygons, Selection, Tolerances};
use kinogaki_studio_api::standard::{self, PolygonsCodec};
use kinogaki_studio_api::{Data, DataCodec, Scope, StoredValue};
use std::sync::Arc;

#[test]
fn every_fixture_passes_the_sdk_consumers() {
    let report = conformance::run(&[&Fixtures::default()], &[&Codecs, &Invariants]);
    assert!(report.failures().is_empty(), "{report}");
    assert_eq!(report.outcomes.len(), 2 * (9 + 7 + 6));
}

#[test]
#[ignore = "1M faces: run with --ignored"]
fn the_large_grid_passes_the_sdk_consumers() {
    let report = conformance::run(&[&Fixtures { large: true }], &[&Codecs, &Invariants]);
    assert!(report.failures().is_empty(), "{report}");
}

#[test]
fn a_failing_consumer_is_reported() {
    struct Picky;
    impl Consumer for Picky {
        fn name(&self) -> &str {
            "picky"
        }
        fn check(&self, fixture: &str, _: &Data) -> Result<(), String> {
            (fixture != "points only")
                .then_some(())
                .ok_or("no faces".into())
        }
    }
    let report = conformance::run(&[&Fixtures::default()], &[&Picky]);
    let failed = report.failures();
    assert_eq!(failed.len(), 1);
    assert_eq!(
        (failed[0].fixture.as_str(), failed[0].result.clone()),
        ("points only", Err("no faces".into()))
    );
    assert!(
        report
            .to_string()
            .contains("SDK fixtures / points only / picky: FAILED: no faces")
    );
}

fn grid() -> Polygons {
    Fixtures::default()
        .polygons()
        .into_iter()
        .find(|(n, _)| n == "quad grid")
        .unwrap()
        .1
}

#[test]
fn values_sharing_a_topology_share_it_once_read() {
    let a = grid();
    let b = a.with_positions(vec![[1.0; 3]; 25].into()).unwrap();
    let (a, b) = (standard::polygons().wrap(a), standard::polygons().wrap(b));
    let mut saved = MemoryElement::new("results");
    saved
        .write(|w| {
            PolygonsCodec.write(&a, None, &mut *w.child("a", "result")?)?;
            PolygonsCodec.write(&b, None, &mut *w.child("b", "result")?)
        })
        .unwrap();
    let (ra, rb) = saved.read(|r| {
        let read = |n| {
            PolygonsCodec
                .read(&*r.required_child(n).unwrap(), None)
                .unwrap()
        };
        (read("a"), read("b"))
    });
    let (ra, rb) = (
        standard::polygons().get(&ra).unwrap(),
        standard::polygons().get(&rb).unwrap(),
    );
    assert!(Arc::ptr_eq(ra.topology(), rb.topology()));
    assert_eq!(rb.positions()[3], [1.0; 3]);
}

#[test]
fn a_saved_value_breaking_a_rule_does_not_read() {
    let data = standard::polygons().wrap(grid());
    let mut saved = MemoryElement::new("result");
    saved
        .write(|w| PolygonsCodec.write(&data, None, w))
        .unwrap();
    // Point 0 of the saved positions made infinite.
    let mut bad = MemoryElement::new("result");
    bad.write(|w| {
        PolygonsCodec.write(&data, None, w)?;
        let mut p: Vec<f64> = vec![0.0; 75];
        p[0] = f64::INFINITY;
        w.set_rows("positions", 3, Field::F64s(p))
    })
    .unwrap();
    let error = bad.read(|r| PolygonsCodec.read(r, None)).unwrap_err();
    assert!(
        error.to_string().contains("point 0 is not finite"),
        "{error}"
    );
}

#[test]
fn components_hold_a_standard_selection() {
    let data = standard::polygons().wrap(grid());
    let s = Selection::new(Class::Primitive, vec![3, 1], Some(3), None).unwrap();
    let c = standard::components(data.ty(), s.clone(), 16);
    assert_eq!((c.len(), c.selection()), (2, Some(&s)));
    let all = standard::components(data.ty(), Selection::all(Class::Point, None), 25);
    assert_eq!(all.len(), 25);
}

#[test]
fn the_tolerances_save_as_a_project_value() {
    let value = StoredValue::of::<Tolerances>();
    assert_eq!(value.id(), "tolerances");
    let inches = Tolerances::default().with_unit(25.4).unwrap();
    let mut saved = MemoryElement::new("tolerances");
    saved.write(|w| value.write(&inches, w)).unwrap();
    let read = saved.read(|r| value.read(r)).unwrap();
    // The model unit isn't saved (the project's units are its source): the
    // value reads back unsized, and sized again it is the same.
    let read = read.downcast_ref::<Tolerances>().unwrap();
    assert_eq!(read.millimeters_per_unit(), 1.0);
    assert_eq!(read.with_unit(25.4).unwrap(), inches);
    let mut wrong = MemoryElement::new("tolerances");
    wrong
        .write(|w| {
            value.write(&inches, w)?;
            w.set("positional_mm", Field::Float(5.0))
        })
        .unwrap();
    assert!(wrong.read(|r| value.read(r)).is_err());
}

#[test]
fn the_display_quality_saves_as_a_project_value_by_its_label() {
    use kinogaki_studio_api::geometry::DisplayQuality;
    let value = StoredValue::of::<DisplayQuality>();
    assert_eq!(
        (value.id(), value.scope()),
        ("display_quality", Scope::Project)
    );
    for q in DisplayQuality::ALL {
        let mut saved = MemoryElement::new("display_quality");
        saved.write(|w| value.write(&q, w)).unwrap();
        let read = saved.read(|r| value.read(r)).unwrap();
        assert_eq!(read.downcast_ref::<DisplayQuality>(), Some(&q));
    }
    let mut wrong = MemoryElement::new("display_quality");
    wrong
        .write(|w| w.set("quality", Field::Text("Ultra".into())))
        .unwrap();
    assert!(wrong.read(|r| value.read(r)).is_err());
}

#[test]
fn equal_texts_save_alike_whatever_their_tables() {
    use kinogaki_studio_api::geometry::{Attribute, Attributes, Text};
    let quad = |text: Text| {
        let t = kinogaki_studio_api::geometry::Topology::new(
            4,
            vec![0, 4].into(),
            vec![0, 1, 2, 3].into(),
            1,
        )
        .unwrap();
        let attributes = Attributes::new()
            .with(Class::Point, "label", Attribute::text(text))
            .unwrap();
        standard::polygons().wrap(Polygons::new(vec![[0.0; 3]; 4].into(), t, attributes).unwrap())
    };
    let save = |data: &Data| {
        let mut saved = MemoryElement::new("result");
        saved.write(|w| PolygonsCodec.write(data, None, w)).unwrap();
        saved
    };
    // A gather keeping a larger, reordered table, and its compact twin.
    let table: Arc<[Arc<str>]> = ["unused", "b", "a"].map(Arc::from).into();
    let gathered = Text::new(vec![2, 1, 2, Text::NONE].into(), table).unwrap();
    let compact = Text::from_strs(["a", "b", "a", ""]);
    assert_eq!(gathered, compact);
    assert_eq!(save(&quad(gathered)), save(&quad(compact)));
}
