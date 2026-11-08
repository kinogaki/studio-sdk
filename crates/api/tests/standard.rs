//! The standard data types: handles, codecs, the tolerances and display
//! quality values and the conformance suite.
use kinogaki_studio_api::element::{Field, MemoryElement};
use kinogaki_studio_api::geometry::{
    Attributes, Domain, Elements, Geometry, Selection, Tolerances,
};
use kinogaki_studio_api::standard::{self, GeometryCodec};
use kinogaki_studio_api::{Data, DataCodec, Scope, StoredValue, Units, UpAxis};
use kinogaki_studio_api_testing as conformance;
use kinogaki_studio_api_testing::{Codecs, Consumer, Fixtures, Invariants};
use std::sync::Arc;

#[test]
fn every_fixture_passes_the_sdk_consumers() {
    let report = conformance::run(&[&Fixtures::default()], &[&Codecs, &Invariants]);
    assert!(report.failures().is_empty(), "{report}");
    assert_eq!(report.outcomes.len(), 2 * (11 + 7 + 3));
}

#[test]
#[ignore = "hand: a million faces"]
fn hand_run_the_large_grid_passes_the_sdk_consumers() {
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

fn grid() -> Geometry {
    Fixtures::default()
        .geometry()
        .into_iter()
        .find(|(n, _)| n == "quad grid")
        .unwrap()
        .1
}

#[test]
fn values_sharing_a_topology_share_it_once_read() {
    let a = grid();
    let b = a.with_positions(vec![[1.0; 3]; 25].into()).unwrap();
    let (a, b) = (standard::geometry().wrap(a), standard::geometry().wrap(b));
    let mut saved = MemoryElement::new("results");
    saved
        .write(|w| {
            GeometryCodec.write(&a, &mut *w.child("a", "result")?)?;
            GeometryCodec.write(&b, &mut *w.child("b", "result")?)
        })
        .unwrap();
    let (ra, rb) = saved
        .read(|r| {
            let read = |n| GeometryCodec.read(&*r.required_child(n).unwrap()).unwrap();
            Ok((read("a"), read("b")))
        })
        .unwrap();
    let (ra, rb) = (
        standard::geometry().get(&ra).unwrap(),
        standard::geometry().get(&rb).unwrap(),
    );
    assert!(Arc::ptr_eq(ra.primitives(), rb.primitives()));
    assert_eq!(rb.positions()[3], [1.0; 3]);
}

#[test]
fn a_saved_value_breaking_a_rule_does_not_read() {
    let data = standard::geometry().wrap(grid());
    let mut saved = MemoryElement::new("result");
    saved.write(|w| GeometryCodec.write(&data, w)).unwrap();
    // Point 0 of the saved positions made infinite.
    let mut bad = MemoryElement::new("result");
    bad.write(|w| {
        GeometryCodec.write(&data, w)?;
        let mut p: Vec<f64> = vec![0.0; 75];
        p[0] = f64::INFINITY;
        w.set_rows("positions", 3, Field::F64s(p))
    })
    .unwrap();
    let error = bad.read(|r| GeometryCodec.read(r)).unwrap_err();
    assert!(
        error.to_string().contains("point 0 is not finite"),
        "{error}"
    );
}

#[test]
fn components_hold_a_standard_selection() {
    let data = standard::geometry().wrap(grid());
    let members = Elements::from_indices(16, [3, 1]).unwrap();
    let s = Selection::new(Domain::Primitive, members, Some(3)).unwrap();
    let c = standard::components(data.ty(), s.clone());
    assert_eq!((c.len(), c.selection()), (2, Some(&s)));
    let every = Selection::new(Domain::Point, Elements::all(25), None).unwrap();
    assert_eq!(standard::components(data.ty(), every).len(), 25);
}

#[test]
fn the_tolerances_save_as_a_project_value() {
    let value = StoredValue::of::<Tolerances>();
    assert_eq!(value.id().as_str(), "kinogaki.core.tolerances");
    let units = Units::new(0.0254, UpAxis::Z).unwrap();
    let inches = Tolerances::default().with_units(units);
    let mut saved = MemoryElement::new("tolerances");
    saved.write(|w| value.write(&inches, w)).unwrap();
    let read = saved.read(|r| value.read(r)).unwrap();
    // The model's units aren't saved (the project's units are their
    // source): the value reads back in millimeters, and given the units
    // again it is the same.
    let read = read.downcast_ref::<Tolerances>().unwrap();
    assert_eq!(read.units(), Units::MILLIMETERS);
    assert_eq!(read.with_units(units), inches);
    let mut wrong = MemoryElement::new("tolerances");
    wrong
        .write(|w| {
            value.write(&inches, w)?;
            w.set("positionalMm", Field::Float(5.0))
        })
        .unwrap();
    assert!(wrong.read(|r| value.read(r)).is_err());
}

#[test]
fn the_display_quality_saves_as_a_project_value_by_its_label() {
    use kinogaki_studio_api::geometry::DisplayQuality;
    let value = StoredValue::of::<DisplayQuality>();
    assert_eq!(
        (value.id().as_str(), value.scope()),
        ("kinogaki.core.display_quality", Scope::Project)
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
        let t = kinogaki_studio_api::geometry::Primitives::of_polygons(
            4,
            vec![0, 4].into(),
            vec![0, 1, 2, 3].into(),
        )
        .unwrap();
        let attributes = Attributes::new()
            .with(Domain::Point, "label", Attribute::text(text))
            .unwrap();
        standard::geometry().wrap(Geometry::new(vec![[0.0; 3]; 4].into(), t, attributes).unwrap())
    };
    let save = |data: &Data| {
        let mut saved = MemoryElement::new("result");
        saved.write(|w| GeometryCodec.write(data, w)).unwrap();
        saved
    };
    // A gather keeping a larger, reordered table, and its compact twin.
    let table: Arc<[Arc<str>]> = ["unused", "b", "a"].map(Arc::from).into();
    let gathered = Text::new(vec![2, 1, 2, Text::NONE].into(), table).unwrap();
    let compact = Text::from_strs(["a", "b", "a", ""]);
    assert_eq!(gathered, compact);
    assert_eq!(save(&quad(gathered)), save(&quad(compact)));
}

/// A quad saved with one attribute element written by hand: `fields` on
/// `a0`, read back.
fn read_quad_with(fields: &[(&str, Field)]) -> Result<Geometry, String> {
    let quad = kinogaki_studio_api::geometry::Primitives::of_polygons(
        4,
        vec![0, 4].into(),
        vec![0, 1, 2, 3].into(),
    )
    .unwrap();
    let value = Geometry::new(vec![[0.0; 3]; 4].into(), quad, Attributes::default()).unwrap();
    let data = standard::geometry().wrap(value);
    let mut saved = MemoryElement::new("result");
    saved
        .write(|w| {
            GeometryCodec.write(&data, w)?;
            let mut a = w.child("a0", "attribute")?;
            for (name, field) in fields {
                a.set(name, field.clone())?;
            }
            Ok(())
        })
        .unwrap();
    let read = saved.read(|r| GeometryCodec.read(r));
    let read = read.map_err(|e| e.to_string())?;
    Ok(standard::geometry().get(&read).unwrap().clone())
}

#[test]
fn attributes_read_by_storage_width_and_meaning_strictly() {
    use kinogaki_studio_api::geometry::Meaning;
    let text = |s: &str| Field::Text(s.into());
    let element = |name: &str, storage: &str, width: i64, meaning: &str, values: Field| {
        vec![
            ("name", text(name)),
            ("domain", text("point")),
            ("storage", text(storage)),
            ("width", Field::Int(width)),
            ("meaning", text(meaning)),
            ("values", values),
        ]
    };
    // Ids past 2^53 read exactly.
    let big = (1i64 << 53) + 1;
    let ids = Field::I64s(vec![big, -big, 0, i64::MAX]);
    let read = read_quad_with(&element("id", "i64", 1, "none", ids)).unwrap();
    let id = read
        .attributes()
        .tuples::<i64>(Domain::Point, "id")
        .unwrap();
    assert_eq!(id, [big, -big, 0, i64::MAX]);
    // A color of four f32s.
    let rgba = Field::F32s(vec![0.25; 16]);
    let read = read_quad_with(&element("tint", "f32", 4, "color", rgba)).unwrap();
    let tint = read.attributes().get(Domain::Point, "tint").unwrap();
    assert_eq!((tint.ty().width, tint.ty().meaning), (4, Meaning::Color));
    // Arrays read their offsets.
    let mut lists = element("w", "f64", 1, "none", Field::F64s(vec![1.0, 2.0, 3.0]));
    lists.push(("offsets", Field::U32s(vec![0, 0, 1, 1, 3])));
    let read = read_quad_with(&lists).unwrap();
    let w = read.attributes().get(Domain::Point, "w").unwrap();
    assert_eq!(
        (w.ty().array, w.offsets().map(|o| o.to_vec())),
        (true, Some(vec![0, 0, 1, 1, 3]))
    );
    // Refused, naming the element.
    let refused = [
        (
            element("w", "f16", 1, "none", Field::F32s(vec![0.0; 4])),
            "no storage \"f16\"",
        ),
        (
            element("w", "f32", 2, "color", Field::F32s(vec![0.0; 8])),
            "color attributes have width 3 or 4, not 2",
        ),
        (
            element("w", "f32", 1, "smell", Field::F32s(vec![0.0; 4])),
            "no meaning \"smell\"",
        ),
        (
            element("w", "f32", 1, "none", Field::F64s(vec![0.0; 4])),
            "values is not an array of f32",
        ),
        (
            element("w", "f32", 0, "none", Field::F32s(vec![0.0; 4])),
            "at least one value an item",
        ),
        (
            element("w", "f32", 300, "none", Field::F32s(vec![0.0; 4])),
            "width is not 1 to 255",
        ),
        // An array's values without its offsets are items, one too many.
        (
            element(
                "w",
                "f64",
                1,
                "none",
                Field::F64s(vec![1.0, 2.0, 3.0, 4.0, 5.0]),
            ),
            "has 5 values for 4 elements",
        ),
        (
            element("Cd", "f64", 3, "color", Field::F64s(vec![0.0; 12])),
            "Cd is f32 × 3 color, not f64 × 3 color",
        ),
        (
            element("P", "f64", 3, "point", Field::F64s(vec![0.0; 12])),
            "never an attribute",
        ),
    ];
    for (fields, message) in refused {
        let error = read_quad_with(&fields).unwrap_err();
        assert!(error.contains(message), "{error}");
    }
    let path = read_quad_with(&element("w", "f16", 1, "none", Field::F32s(vec![0.0; 4])));
    assert!(path.unwrap_err().contains("a0"), "the element is named");
}

/// A two-point primitive table saved by hand with `kinds` and, when
/// given, a `curves` child of `degrees`, read back.
fn read_line_with(kinds: Vec<u8>, degrees: Option<Vec<u8>>) -> Result<Geometry, String> {
    let mut saved = MemoryElement::new("result");
    saved
        .write(|w| {
            w.set_rows(
                "positions",
                3,
                Field::F64s(vec![0.0, 0.0, 0.0, 1.0, 0.0, 0.0]),
            )?;
            let mut t = w.child("primitives", "primitives")?;
            t.set("points", Field::Int(2))?;
            t.set("offsets", Field::U32s(vec![0, 2]))?;
            t.set("vertices", Field::U32s(vec![0, 1]))?;
            t.set("kinds", Field::U8s(kinds.clone()))?;
            if let Some(degrees) = &degrees {
                let mut c = t.child("curves", "curves")?;
                c.set("degrees", Field::U8s(degrees.clone()))?;
                c.set("knotOffsets", Field::U32s(vec![0, 0]))?;
                c.set("knots", Field::F64s(Vec::new()))?;
            }
            Ok(())
        })
        .unwrap();
    let read = saved.read(|r| GeometryCodec.read(r));
    let read = read.map_err(|e| e.to_string())?;
    Ok(standard::geometry().get(&read).unwrap().clone())
}

/// Kinds a file can't hold refuse it: a byte naming no kind, a curve kind
/// without curve data, curve data without a curve, and a curve breaking
/// its basis's rule. The same table with its curve data reads.
#[test]
fn saved_kinds_and_curve_data_read_strictly() {
    use kinogaki_studio_api::geometry::{Basis, Kind};
    let bezier = Kind::curve(Basis::Bezier, false).byte();
    let line = read_line_with(vec![bezier], Some(vec![1])).unwrap();
    assert_eq!(line.curve(0).point(0.5).to_array(), [0.5, 0.0, 0.0]);
    let refused = [
        (vec![0x17], None, "no primitive kind 23"),
        (vec![bezier], None, "curve primitives need curve data"),
        (
            vec![Kind::POLYLINE.byte()],
            Some(vec![1]),
            "curve data without a curve primitive",
        ),
        (vec![bezier], Some(vec![3]), "curve 0 has 2 control points"),
    ];
    for (kinds, degrees, message) in refused {
        let error = read_line_with(kinds, degrees).unwrap_err();
        assert!(error.contains(message), "{error}");
    }
}
