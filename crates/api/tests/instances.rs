//! Instances: what they refuse, what a move shares, and how they save.
use kinogaki_studio_api::Data;
use kinogaki_studio_api::DataCodec;
use kinogaki_studio_api::element::MemoryElement;
use kinogaki_studio_api::geometry::{
    Attribute, Attributes, Domain, Geometry, Meaning, Vec3, Xform, names,
};
use kinogaki_studio_api::standard::{self, Instances, InstancesCodec, MAX_DEPTH};

fn along(n: usize) -> Vec<Xform> {
    (0..n)
        .map(|i| Xform::translation(Vec3::new(i as f64, 0.0, 0.0)))
        .collect()
}

fn prototype() -> Data {
    standard::geometry().wrap(Geometry::empty())
}

#[test]
fn instances_refuse_what_they_cant_place() {
    let two = vec![prototype(), prototype()];
    let none = Attributes::new();
    // Several prototypes need each instance's; an index must name one.
    assert!(Instances::new(two.clone(), along(2).into(), None, none.clone()).is_err());
    assert!(
        Instances::new(
            two.clone(),
            along(2).into(),
            Some(vec![0, 2].into()),
            none.clone()
        )
        .is_err()
    );
    assert!(
        Instances::new(
            two.clone(),
            along(2).into(),
            Some(vec![0].into()),
            none.clone()
        )
        .is_err()
    );
    let ok = Instances::new(two, along(2).into(), Some(vec![1, 0].into()), none.clone()).unwrap();
    assert!(ok.prototype_index(0) == 1);
    // Instances without a prototype; a placement that isn't finite.
    assert!(Instances::new(Vec::new(), along(1).into(), None, none.clone()).is_err());
    let mut bad = along(2);
    bad[1] = Xform::translation(Vec3::new(f64::NAN, 0.0, 0.0));
    assert!(Instances::new(vec![prototype()], bad.into(), None, none.clone()).is_err());
    // Attributes on Instance and Detail only, sized for the instances.
    let colors = |n| Attribute::tuples(vec![[1.0f32, 0.5, 0.0]; n], Meaning::Color).unwrap();
    let on = |domain, n| {
        Attributes::new()
            .with(domain, names::CD, colors(n))
            .unwrap()
    };
    assert!(
        Instances::new(
            vec![prototype()],
            along(3).into(),
            None,
            on(Domain::Instance, 3)
        )
        .is_ok()
    );
    assert!(
        Instances::new(
            vec![prototype()],
            along(3).into(),
            None,
            on(Domain::Instance, 2)
        )
        .is_err()
    );
    assert!(
        Instances::new(
            vec![prototype()],
            along(3).into(),
            None,
            on(Domain::Point, 3)
        )
        .is_err()
    );
    // Nesting deeper than MAX_DEPTH.
    let mut nested = prototype();
    for depth in 1..=MAX_DEPTH {
        let level = Instances::new(vec![nested], along(1).into(), None, none.clone()).unwrap();
        assert_eq!(level.depth(), depth);
        nested = standard::instances().wrap(level);
    }
    assert!(Instances::new(vec![nested], along(1).into(), None, none).is_err());
}

#[test]
fn a_move_shares_the_prototypes_and_attributes() {
    let ids = Attribute::tuples(vec![[7i64], [9]], Meaning::None).unwrap();
    let attributes = Attributes::new()
        .with(Domain::Instance, names::ID, ids)
        .unwrap();
    let copies = Instances::new(vec![prototype()], along(2).into(), None, attributes).unwrap();
    let moved = copies.with_xforms(vec![Xform::IDENTITY; 2].into()).unwrap();
    assert!(moved.prototypes()[0].ptr_eq(&copies.prototypes()[0]));
    assert!(
        std::ptr::eq(moved.attributes(), copies.attributes()),
        "shared"
    );
    assert_eq!(moved.xforms()[1], Xform::IDENTITY);
    assert!(
        copies.with_xforms(along(3).into()).is_err(),
        "as many placements"
    );
}

#[test]
fn instances_round_trip_with_each_prototype_written_once() {
    let (a, b) = (prototype(), prototype());
    let copies = Instances::new(
        vec![a.clone(), b],
        along(4).into(),
        Some(vec![0, 1, 1, 0].into()),
        Attributes::new(),
    )
    .unwrap();
    // A value holding these instances twice over, and `a` again.
    let outer = Instances::new(
        vec![standard::instances().wrap(copies.clone()), a.clone()],
        along(2).into(),
        Some(vec![0, 1].into()),
        Attributes::new(),
    )
    .unwrap();
    let data = standard::instances().wrap(outer);
    let mut saved = MemoryElement::new("result");
    saved
        .write(|w| InstancesCodec.write(&data, None, w))
        .unwrap();
    let read = saved.read(|r| InstancesCodec.read(r, None)).unwrap();
    let read = standard::instances().get(&read).unwrap();
    assert_eq!(read.xforms().as_slice(), along(2).as_slice());
    assert_eq!(read.prototype_indices().unwrap().as_slice(), &[0, 1]);
    assert!(
        read.prototypes()[1].ptr_eq(&a),
        "memory keeps the values themselves"
    );
    let inner = standard::instances().get(&read.prototypes()[0]).unwrap();
    assert_eq!(inner.len(), copies.len());
}
