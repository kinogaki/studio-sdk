use super::*;
use kinogaki_geometry::{Attribute, Basis, Kind, Vec3};

fn triangle() -> Geometry {
    let t = Primitives::of_polygons(3, vec![0, 3].into(), vec![0, 1, 2].into()).unwrap();
    let positions = vec![[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]];
    Geometry::new(positions.into(), t, Attributes::new()).unwrap()
}

fn with(value: &Geometry, domain: Domain, name: &str, a: Attribute) -> Geometry {
    let attributes = value.attributes().with(domain, name, a).unwrap();
    value.with_attributes(attributes).unwrap()
}

fn at(x: f64) -> Xform {
    Xform::translation(Vec3::new(x, 0.0, 0.0))
}

#[test]
fn one_value_where_it_is_is_that_value() {
    let t = triangle();
    let same = gather(&[Placed::new(&t)]).unwrap();
    assert!(Arc::ptr_eq(same.primitives(), t.primitives()));
    assert_eq!(gather(&[]).unwrap().positions().len(), 0);
}

#[test]
fn elements_append_and_a_lacking_input_takes_the_default() {
    let plain = triangle();
    let colored = with(
        &plain,
        Domain::Point,
        names::CD,
        Attribute::tuples(vec![[0.0f32, 1.0, 0.0]; 3], Meaning::Color).unwrap(),
    );
    let weighted = with(
        &plain,
        Domain::Primitive,
        "weight",
        Attribute::tuples(vec![5.0f64], Meaning::None).unwrap(),
    );
    let all = gather(&[
        Placed::new(&colored),
        Placed::new(&plain),
        Placed::new(&weighted),
    ])
    .unwrap();
    assert_eq!((all.positions().len(), all.primitives().len()), (9, 3));
    assert_eq!(all.primitives().primitive(2), [6, 7, 8]);
    let cd = all
        .attributes()
        .tuples::<[f32; 3]>(Domain::Point, names::CD);
    assert_eq!(cd.unwrap()[0], [0.0, 1.0, 0.0]);
    assert_eq!(cd.unwrap()[3], [1.0, 1.0, 1.0], "white for a color");
    let weight = all.attributes().tuples::<f64>(Domain::Primitive, "weight");
    assert_eq!(weight.unwrap(), [0.0, 0.0, 5.0]);
}

#[test]
fn another_type_counts_as_lacking_it() {
    let plain = triangle();
    let a = Attribute::tuples(vec![1.0f32; 3], Meaning::None).unwrap();
    let b = Attribute::tuples(vec![7_i32; 3], Meaning::None).unwrap();
    let (a, b) = (
        with(&plain, Domain::Point, "x", a),
        with(&plain, Domain::Point, "x", b),
    );
    let all = gather(&[Placed::new(&a), Placed::new(&b)]).unwrap();
    let x = all.attributes().tuples::<f32>(Domain::Point, "x").unwrap();
    assert_eq!(x, [1.0, 1.0, 1.0, 0.0, 0.0, 0.0]);
}

#[test]
fn ids_keep_the_first_and_refresh_the_rest() {
    let t = ids::numbered(&triangle()).unwrap();
    let all = gather(&[Placed::new(&t), Placed::new(&t)]).unwrap();
    assert_eq!(
        all.ids(Domain::Point).unwrap().as_slice(),
        [0, 1, 2, 3, 4, 5]
    );
    assert_eq!(all.ids(Domain::Primitive).unwrap().as_slice(), [0, 1]);
}

#[test]
fn text_tables_join_and_details_are_the_first_holders() {
    let plain = triangle();
    let part = |name: &str| Attribute::text(Text::from_strs([name]));
    let a = with(&plain, Domain::Primitive, names::PART, part("part_a"));
    let b = with(&plain, Domain::Primitive, names::PART, part("part_b"));
    let c = with(&plain, Domain::Primitive, names::PART, part("part_a"));
    let a = with(
        &a,
        Domain::Detail,
        "note",
        Attribute::text(Text::from_strs(["first"])),
    );
    let b = with(
        &b,
        Domain::Detail,
        "note",
        Attribute::text(Text::from_strs(["second"])),
    );
    let all = gather(&[
        Placed::new(&a),
        Placed::new(&b),
        Placed::new(&plain),
        Placed::new(&c),
    ])
    .unwrap();
    let text = all
        .attributes()
        .text(Domain::Primitive, names::PART)
        .unwrap();
    let read: Vec<_> = text.iter().collect();
    assert_eq!(read, [Some("part_a"), Some("part_b"), None, Some("part_a")]);
    assert_eq!(text.table().len(), 2);
    let note = all.attributes().text(Domain::Detail, "note").unwrap();
    assert_eq!(note.get(0), Some("first"));
}

#[test]
fn groups_and_edge_values_follow_their_elements() {
    let plain = triangle();
    let edges = plain.count(Domain::Edge).unwrap();
    assert_eq!(edges, 3);
    let attributes = plain
        .attributes()
        .with_group(
            Domain::Edge,
            names::SHARP,
            Elements::from_indices(3, [1]).unwrap(),
        )
        .unwrap()
        .with_group(
            Domain::Point,
            "tip",
            Elements::from_indices(3, [2]).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Edge,
            "weight",
            Attribute::tuples(vec![1.0f32, 2.0, 3.0], Meaning::None).unwrap(),
        )
        .unwrap();
    let marked = plain.with_attributes(attributes).unwrap();
    let all = gather(&[Placed::new(&plain), Placed::new(&marked)]).unwrap();
    let tips = all.attributes().group(Domain::Point, "tip").unwrap();
    assert_eq!(tips.iter().collect::<Vec<_>>(), [5]);
    let sharp = all.attributes().group(Domain::Edge, names::SHARP).unwrap();
    assert_eq!(sharp.iter().count(), 1);
    let at_edge = |e: u32| all.primitives().edge(e as usize);
    let own = marked.primitives().edge(1);
    let found = at_edge(sharp.iter().next().unwrap());
    assert_eq!(
        found.points,
        own.points.map(|p| p + 3),
        "the same edge, shifted"
    );
    let weight = all
        .attributes()
        .tuples::<f32>(Domain::Edge, "weight")
        .unwrap();
    assert_eq!(weight, [0.0, 0.0, 0.0, 1.0, 2.0, 3.0]);
}

#[test]
fn a_placement_moves_what_turns_with_it() {
    let plain = triangle();
    // A quarter turn about Z at (5, 0, 0), scaled 2 along X (before the turn).
    let quarter = Xform::from_axes(
        Vec3::new(0.0, 2.0, 0.0),
        Vec3::new(-1.0, 0.0, 0.0),
        Vec3::new(0.0, 0.0, 1.0),
        Vec3::new(5.0, 0.0, 0.0),
    );
    let half = std::f32::consts::FRAC_1_SQRT_2;
    let attributes = plain
        .attributes()
        .with(
            Domain::Point,
            "dir",
            Attribute::tuples(vec![[1.0f32, 0.0, 0.0]; 3], Meaning::Vector).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Point,
            names::N,
            Attribute::tuples(vec![[1.0f32, 0.0, 0.0]; 3], Meaning::Normal).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Point,
            "spot",
            Attribute::tuples(vec![[1.0f64, 0.0, 0.0]; 3], Meaning::Point).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Point,
            names::ORIENT,
            Attribute::tuples(vec![[0.0f32, 0.0, 0.0, 1.0]; 3], Meaning::Rotation).unwrap(),
        )
        .unwrap()
        .with(
            Domain::Point,
            names::CD,
            Attribute::tuples(vec![[0.5f32, 0.5, 0.5]; 3], Meaning::Color).unwrap(),
        )
        .unwrap();
    let t = plain.with_attributes(attributes).unwrap();
    let placed = gather(&[Placed {
        value: &t,
        at: quarter,
    }])
    .unwrap();
    assert_eq!(placed.positions()[1], [5.0, 2.0, 0.0]);
    let a = placed.attributes();
    let close = |x: &[f64], y: &[f64]| x.iter().zip(y).all(|(p, q)| (p - q).abs() < 1e-6);
    let read = |name| {
        let v = a.tuples::<[f32; 3]>(Domain::Point, name).unwrap()[0];
        v.map(f64::from)
    };
    assert!(close(&read("dir"), &[0.0, 2.0, 0.0]), "a vector scales");
    assert!(close(&read(names::N), &[0.0, 1.0, 0.0]), "a normal is unit");
    assert!(close(
        &a.tuples::<[f64; 3]>(Domain::Point, "spot").unwrap()[0],
        &[5.0, 2.0, 0.0]
    ));
    let q = a.tuples::<[f32; 4]>(Domain::Point, names::ORIENT).unwrap()[0];
    assert!(close(
        &q.map(f64::from),
        &[0.0, 0.0, f64::from(half), f64::from(half)]
    ));
    assert_eq!(read(names::CD), [0.5, 0.5, 0.5], "a color stays");
}

#[test]
fn a_transform_composes_after_the_placement() {
    let plain = triangle();
    let mut m = [0.0; 16];
    (m[0], m[5], m[10], m[15], m[3]) = (2.0, 1.0, 1.0, 1.0, 10.0);
    let t = with(
        &plain,
        Domain::Point,
        names::TRANSFORM,
        Attribute::tuples(vec![m; 3], Meaning::Transform).unwrap(),
    );
    let placed = gather(&[Placed {
        value: &t,
        at: at(5.0),
    }])
    .unwrap();
    let got = placed
        .attributes()
        .tuples::<[f64; 16]>(Domain::Point, names::TRANSFORM)
        .unwrap()[0];
    assert_eq!((got[0], got[3]), (2.0, 15.0));
}

#[test]
fn curves_keep_their_data_beside_polygons() {
    let data = CurveData::new(
        vec![1].into(),
        Knots::new(vec![0, 0].into(), Column::default()),
        None,
    );
    let kinds = vec![Kind::curve(Basis::Bezier, false)];
    let table = Primitives::new(
        2,
        vec![0, 2].into(),
        vec![0, 1].into(),
        kinds.into(),
        Some(data),
    )
    .unwrap();
    let line = Geometry::new(
        vec![[0.0; 3], [1.0, 0.0, 0.0]].into(),
        table,
        Attributes::new(),
    )
    .unwrap();
    let t = triangle();
    let all = gather(&[Placed::new(&t), Placed::new(&line)]).unwrap();
    assert!(all.primitives().kind(0).is_polygon());
    assert_eq!(all.primitives().kind(1).basis(), Basis::Bezier);
    assert_eq!(
        all.primitives().curves().unwrap().degrees().as_slice(),
        [1, 1]
    );
}

#[test]
fn arrays_gather_with_their_offsets() {
    let plain = triangle();
    let ty = AttributeType {
        array: true,
        ..AttributeType::new(Storage::I32, 1, Meaning::None)
    };
    let a = Attribute::arrays(
        ty,
        vec![0, 2, 2, 3].into(),
        Values::I32(vec![1, 2, 3].into()),
    )
    .unwrap();
    let held = with(&plain, Domain::Point, "tags", a);
    let all = gather(&[Placed::new(&held), Placed::new(&plain), Placed::new(&held)]).unwrap();
    let got = all.attributes().get(Domain::Point, "tags").unwrap();
    assert_eq!(
        got.offsets().unwrap().as_slice(),
        [0, 2, 2, 3, 3, 3, 3, 5, 5, 6]
    );
}
