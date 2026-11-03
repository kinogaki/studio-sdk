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
    assert_eq!(counts.dirty[5], 1);
    engine.view(
        ViewId(1),
        ViewDesc {
            camera: ViewCamera::Free(Camera::default()),
            width: 4,
            height: 2,
            mode: ViewMode::Final,
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
fn mesh_attributes_hold_one_value_an_element_of_their_domain() {
    let attribute = |domain, components, values: usize| MeshAttribute {
        name: Arc::from("Cd"),
        domain,
        components,
        color: true,
        values: Numbers::Floats(Arc::from(vec![0.5f32; values])),
    };
    // Four points, two triangles.
    assert!(attribute(AttributeDomain::Point, 3, 12).is_consistent(4, 2));
    assert!(attribute(AttributeDomain::Corner, 1, 6).is_consistent(4, 2));
    assert!(attribute(AttributeDomain::Triangle, 4, 8).is_consistent(4, 2));
    assert!(attribute(AttributeDomain::Constant, 2, 2).is_consistent(4, 2));
    assert!(!attribute(AttributeDomain::Point, 3, 9).is_consistent(4, 2));
    assert!(!attribute(AttributeDomain::Constant, 5, 5).is_consistent(4, 2));
    assert!(!attribute(AttributeDomain::Constant, 0, 0).is_consistent(4, 2));
    assert_eq!(format!("{:?}", Dirty::ATTRIBUTES), "Dirty(ATTRIBUTES)");
}
