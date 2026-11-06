//! Batches, ids and one-shot results.
use super::*;
use std::sync::atomic::Ordering;

#[test]
fn an_add_then_a_remove_cancel() {
    let mut first = SceneUpdate::new();
    first.added.push((id(0), mesh(3)));
    first.added.push((id(1), mesh(3)));
    let mut second = SceneUpdate::new();
    second
        .changed
        .push(Change::new(id(0), Dirty::TRANSFORM, mesh(3)));
    second.removed.push(id(0));
    first.merge(second);
    assert_eq!(first.added.len(), 1);
    assert_eq!(first.added[0].0, id(1));
    assert!(first.removed.is_empty() && first.changed.is_empty());
}

/// Batches of many prims fold by id, keeping each list's order: a drag of
/// 20,000 objects folds in one pass, not one search a change.
#[test]
fn many_prims_fold_by_id_in_order() {
    let count = 20_000;
    let mut first = SceneUpdate::new();
    let mut second = SceneUpdate::new();
    for i in 0..count {
        first
            .changed
            .push(Change::new(id(i), Dirty::TRANSFORM, mesh(3)));
        second
            .changed
            .push(Change::new(id(count - 1 - i), Dirty::VISIBILITY, mesh(4)));
    }
    second.added.push((id(count), mesh(3)));
    second.removed.extend([id(7), id(count)]);
    first.merge(second);
    let ids: Vec<PrimId> = first.changed.iter().map(|c| c.id).collect();
    let kept: Vec<PrimId> = (0..count).filter(|&i| i != 7).map(id).collect();
    assert_eq!(ids, kept, "held order, the removed one gone");
    let both = Dirty::TRANSFORM | Dirty::VISIBILITY;
    assert!(
        first
            .changed
            .iter()
            .all(|c| c.dirty == both && c.prim == mesh(4))
    );
    assert!(first.added.is_empty(), "added and removed: nothing");
    assert_eq!(first.removed, [id(7)]);
}

#[test]
fn a_change_to_an_added_prim_replaces_its_payload() {
    let mut first = SceneUpdate::new();
    first.added.push((id(0), mesh(3)));
    let mut second = SceneUpdate::new();
    second
        .changed
        .push(Change::new(id(0), Dirty::POINTS, mesh(5)));
    first.merge(second);
    assert_eq!(first.added, vec![(id(0), mesh(5))]);
    assert!(first.changed.is_empty());
}

#[test]
fn dirty_bits_or_and_the_later_payload_wins() {
    let mut first = SceneUpdate::new();
    first
        .changed
        .push(Change::new(id(2), Dirty::TRANSFORM, mesh(3)));
    let mut second = SceneUpdate::new();
    second
        .changed
        .push(Change::new(id(2), Dirty::VISIBILITY, mesh(4)));
    first.merge(second);
    assert_eq!(first.changed.len(), 1);
    assert_eq!(first.changed[0].dirty, Dirty::TRANSFORM | Dirty::VISIBILITY);
    assert_eq!(first.changed[0].prim, mesh(4));
    // A removal after a change drops the change.
    let mut third = SceneUpdate::new();
    third.removed.push(id(2));
    first.merge(third);
    assert!(first.changed.is_empty());
    assert_eq!(first.removed, vec![id(2)]);
}

#[test]
fn point_ranges_merge_and_fall_back_to_whole_past_the_cap() {
    let mut first = SceneUpdate::new();
    first
        .changed
        .push(Change::points(id(0), mesh(100_000), Ranges::of(0..10)));
    let mut second = SceneUpdate::new();
    second
        .changed
        .push(Change::points(id(0), mesh(100_000), Ranges::of(20..30)));
    let mut third = SceneUpdate::new();
    third
        .changed
        .push(Change::points(id(0), mesh(100_000), Ranges::of(500..510)));
    first.merge(second);
    first.merge(third);
    let points = &first.changed[0].points;
    // 10..20 is within the gap: one span; 500 is far: a second.
    assert_eq!(points.spans(), &[0..30, 500..510]);
    assert_eq!(points.len(), 40);
    // Many scattered spans: the whole stream.
    let mut ranges = Ranges::new();
    for at in 0..=Ranges::MOST as u32 {
        ranges.add(at * 100..at * 100 + 1);
    }
    assert!(ranges.is_whole());
    // A whole change absorbs a patch, and a patch after TRANSFORM keeps its ranges.
    let mut whole = SceneUpdate::new();
    whole
        .changed
        .push(Change::new(id(0), Dirty::POINTS, mesh(10)));
    let mut patch = SceneUpdate::new();
    patch
        .changed
        .push(Change::points(id(0), mesh(10), Ranges::of(1..2)));
    whole.merge(patch);
    assert!(whole.changed[0].points.is_whole());
    let mut moved = SceneUpdate::new();
    moved
        .changed
        .push(Change::new(id(0), Dirty::TRANSFORM, mesh(10)));
    let mut patch = SceneUpdate::new();
    patch
        .changed
        .push(Change::points(id(0), mesh(10), Ranges::of(1..2)));
    moved.merge(patch);
    assert_eq!(moved.changed[0].points.spans().to_vec(), vec![1..2]);
    assert_eq!(moved.changed[0].dirty, Dirty::TRANSFORM | Dirty::POINTS);
}

#[test]
fn settling_survives_later_non_drag_batches() {
    let mut first = SceneUpdate::new();
    first.settled = true;
    let mut camera = SceneUpdate::new();
    camera.changed.push(Change::new(
        id(9),
        Dirty::TRANSFORM,
        Prim::Camera(Camera::default()),
    ));
    first.merge(camera);
    assert!(first.settled);
    let mut drag = SceneUpdate::new();
    drag.changed
        .push(Change::points(id(0), mesh(10), Ranges::of(0..1)));
    first.merge(drag);
    assert!(!first.settled);
}

#[test]
fn ids_are_reused_only_with_a_new_generation() {
    let mut ids = IdAllocator::new();
    let a = ids.allocate();
    let b = ids.allocate();
    assert_eq!((a.slot, b.slot), (0, 1));
    assert!(ids.release(a));
    assert!(!ids.release(a), "a released id cannot be released again");
    assert!(!ids.is_live(a));
    let c = ids.allocate();
    assert_eq!(c.slot, a.slot);
    assert_ne!(c.generation, a.generation);
    assert!(ids.is_live(c) && !ids.is_live(a));
    assert!(!ids.release(a), "the old id does not name the new prim");
    assert_eq!(ids.slots(), 2);
}

#[test]
fn pending_delivers_once_and_wakes_once() {
    let (wake, wakes) = counter();
    let (deliver, result) = pending::<u32>(wake);
    assert_eq!(result.take(), None);
    assert_eq!(wakes.load(Ordering::SeqCst), 0);
    let worker = std::thread::spawn(move || deliver.deliver(7));
    worker.join().expect("the producer runs");
    assert_eq!(wakes.load(Ordering::SeqCst), 1);
    assert_eq!(result.take(), Some(Ok(7)));
    assert_eq!(result.take(), None);
    assert_eq!(wakes.load(Ordering::SeqCst), 1);
}

#[test]
fn the_recording_engine_counts_what_it_was_sent() {
    let (wake, wakes) = counter();
    let mut engine = RecordingEngine::new(wake);
    let mut first = SceneUpdate::new();
    first.added.push((id(0), mesh(3)));
    engine.update(first);
    let mut moved = SceneUpdate::new();
    moved
        .changed
        .push(Change::new(id(0), Dirty::TRANSFORM, mesh(3)));
    engine.update(moved);
    let mut drag = SceneUpdate::new();
    drag.changed
        .push(Change::points(id(0), mesh(3), Ranges::of(1..2)));
    engine.update(drag);
    let counts = &engine.counts;
    assert_eq!((counts.updates, counts.added, counts.changed), (3, 1, 2));
    assert_eq!(counts.triangles, 1, "moves and drags send no triangles");
    assert_eq!(counts.points, 4);
    assert_eq!(counts.dirty[4], 1);
    engine.view(
        ViewId(1),
        ViewDesc {
            camera: ViewCamera::Free(Camera::default()),
            width: 4,
            height: 2,
            mode: ViewMode::Final,
            values: Default::default(),
        },
    );
    let result = engine.finish(
        ViewId(1),
        &[AovDesc {
            aov: aov::standard::beauty(),
            format: AovFormat::F32,
        }],
    );
    let outputs = result.take().expect("delivered at once").unwrap();
    assert_eq!(outputs.planes[0].1.len(), 4 * 2 * 4);
    assert_eq!(wakes.load(Ordering::SeqCst), 1);
}

#[test]
fn a_result_dropped_is_no_longer_wanted_and_a_preview_answers() {
    let (wake, _) = counter();
    let (deliver, result) = pending::<u32>(wake.clone());
    assert!(deliver.wanted());
    drop(result);
    assert!(!deliver.wanted(), "a host's cancel, as the engine sees it");
    let mut engine = RecordingEngine::new(wake);
    let result = engine.preview(PreviewRequest {
        network: Arc::default(),
        textures: Vec::new(),
        shape: PreviewShape::Sphere,
        lighting: PreviewLighting::Studio,
        side: 8,
    });
    let image = result.take().expect("at once").unwrap();
    assert_eq!((image.side, image.pixels.len()), (8, 8 * 8 * 4));
    assert_eq!(engine.previews.len(), 1);
}

#[test]
fn a_shape_is_refused_unless_whole() {
    use crate::geometry::{Attribute, Domain, Kind, Meaning, Primitives, Sampled, Xform};
    let points = || -> crate::geometry::Column<[f32; 3]> { vec![[0.0f32; 3]; 4].into() };
    let triangles = || Topology::Triangles(vec![[0u32, 1, 2], [0, 2, 3]].into());
    let shape = || Shape::builder(points(), triangles());
    let floats = |n: usize| Attribute::tuples(vec![0.5f32; n], Meaning::None).unwrap();
    // Four points, two triangles: every domain's size.
    let whole = shape()
        .primvar(Domain::Point, "a", floats(4))
        .primvar(Domain::Vertex, "b", floats(6))
        .primvar(Domain::Primitive, "c", floats(2))
        .primvar(Domain::Detail, "d", floats(1))
        .build()
        .unwrap();
    assert_eq!(whole.primvar("b").map(|(d, _)| d), Some(Domain::Vertex));
    assert!(whole.primvar("nothing").is_none());
    let refused = |b: ShapeBuilder| b.build().unwrap_err().message().to_owned();
    assert!(refused(shape().primvar(Domain::Point, "a", floats(3))).contains("a has 3 values"));
    assert!(refused(shape().primvar(Domain::Vertex, "a", floats(2))).contains("a has 2 values"));
    assert!(refused(shape().primvar(Domain::Primitive, "a", floats(4))).contains("a has 4"));
    assert!(refused(shape().primvar(Domain::Detail, "a", floats(2))).contains("a has 2"));
    assert!(refused(shape().primvar(Domain::Edge, "a", floats(5))).contains("edge"));
    assert!(refused(shape().primvar(Domain::Point, "N", floats(4))).contains("N is"));
    assert!(refused(shape().primvar(Domain::Point, "not a name", floats(4))).contains("name"));
    // Connectivity within the points, slots within the materials, finite
    // points; a rebuild is checked as a build is.
    let past = Topology::Triangles(vec![[0u32, 1, 9]].into());
    assert!(refused(Shape::builder(points(), past)).contains("point 9"));
    let far: crate::geometry::Column<[f32; 3]> = vec![[f32::NAN; 3]; 4].into();
    assert!(refused(Shape::builder(far, triangles())).contains("finite"));
    let slots = Subsets::new(vec![0u16, 0].into(), Vec::new());
    assert!(refused(shape().subsets(slots)).contains("slot 0"));
    let short = Subsets::new(vec![0u16].into(), vec![Id::of(id(1))]);
    assert!(refused(shape().subsets(short)).contains("1 material slots"));
    let fine = Subsets::new(vec![Subsets::DEFAULT, 0].into(), vec![Id::of(id(1))]);
    let dressed = shape().subsets(fine).build().unwrap();
    assert_eq!(dressed.subsets().of(0), None);
    assert_eq!(dressed.subsets().of(1), Some(Id::of(id(1))));
    assert!(refused(whole.rebuilt().points(vec![[0.0f32; 3]; 2].into())).contains("point"));
    // Curves hold no polygons and number their own points.
    let kinds = vec![Kind::POLYLINE].into();
    let line = Primitives::new(4, vec![0, 4].into(), vec![0, 1, 2, 3].into(), kinds, None).unwrap();
    let curves = Shape::builder(points(), Topology::Curves(Arc::new(line)))
        .primvar(Domain::Vertex, "w", floats(4))
        .primvar(Domain::Primitive, "u", floats(1))
        .build()
        .unwrap();
    assert_eq!(Prim::Shape(curves).kind(), PrimKind::Curves);
    let quad = Primitives::of_polygons(4, vec![0, 4].into(), vec![0, 1, 2, 3].into()).unwrap();
    let polygons = Topology::Curves(Arc::new(quad));
    assert!(refused(Shape::builder(points(), polygons)).contains("polygons"));
    let few = Primitives::new(
        3,
        vec![0, 3].into(),
        vec![0, 1, 2].into(),
        vec![Kind::POLYLINE].into(),
        None,
    )
    .unwrap();
    let few = Topology::Curves(Arc::new(few));
    assert!(refused(Shape::builder(points(), few)).contains("over 3 points"));
    assert_eq!(format!("{:?}", Dirty::PRIMVARS), "Dirty(PRIMVARS)");
    // A placement across the shutter stays sorted: Sampled refuses otherwise.
    let moved = Sampled::new(vec![0.0, 1.0], vec![Xform::IDENTITY; 2]).unwrap();
    assert_eq!(
        shape().xform(moved.clone()).build().unwrap().xform(),
        &moved
    );
}

#[test]
fn an_instancer_is_refused_unless_whole() {
    use crate::geometry::{Attribute, Domain, Meaning, Xform};
    let xforms = || vec![Xform::IDENTITY; 3].into();
    let many = || Instancer::builder(vec![id(1), id(2)], xforms());
    let floats = |n: usize| Attribute::tuples(vec![0.5f32; n], Meaning::None).unwrap();
    let table = |d, n| {
        crate::geometry::Attributes::new()
            .with(d, "a", floats(n))
            .unwrap()
    };
    let whole = many()
        .which(Some(vec![1, 0, 1].into()))
        .primvars(table(Domain::Instance, 3))
        .build()
        .unwrap();
    assert_eq!(whole.prototype_of(0), id(2));
    assert_eq!(whole.prototype_of(1), id(1));
    let refused = |b: InstancerBuilder| b.build().unwrap_err().message().to_owned();
    assert!(refused(many().which(Some(vec![0].into()))).contains("1 prototype indices"));
    assert!(refused(many().which(Some(vec![0, 2, 0].into()))).contains("past the 2"));
    assert!(refused(many().primvars(table(Domain::Instance, 2))).contains("a has 2"));
    assert!(refused(many().primvars(table(Domain::Point, 3))).contains("point"));
    let none: Vec<PrimId> = Vec::new();
    assert!(refused(Instancer::builder(none, xforms())).contains("no prototype"));
}
