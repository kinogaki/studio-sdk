//! The package's UI on Studio's fake host, called as Studio calls it:
//! the package registers whole, its levels choose, Pin and Clear run as
//! undo steps and label themselves, and the inspector shows the count.
use super::*;
use studio_ui_api::host::HostUndo;
use studio_ui_api::package::Registered;
use studio_ui_api::testing::{Call, FakeHost, Hosting, RecordingUi};
use studio_ui_api::{Pos, Rect, Vec2};

fn host() -> (FakeHost, Hosting) {
    let package = package();
    let host = FakeHost::new(pinning(), package.slots.clone());
    let hosted = Hosting::new(package, &host);
    (host, hosted)
}

/// Runs `item` through the context's vocabulary.
fn run(hosted: &mut Hosting, host: &mut FakeHost, item: &str) {
    hosted.with(|p, s| {
        let vocabulary = p.contexts[0].vocabulary.as_ref().unwrap();
        vocabulary.run(s, host, item, Invoked::Views)
    });
}

#[test]
fn the_package_registers_whole() {
    let mut registry = Registry::new();
    registry.register(package()).unwrap();
    assert_eq!(
        registry.object_kind(board()).unwrap().context(),
        Some(pinning())
    );
    assert!(registry.network_context(pinning()).is_some());
    assert!(registry.slot(Pins::id()).is_some());
    let package = package();
    let ui = Registered::of(&package).unwrap();
    assert_eq!(ui.contexts[0].id, pinning());
    let levels = ui.contexts[0].levels.as_ref().unwrap().levels();
    let names: Vec<&str> = levels.iter().map(|l| l.label.as_str()).collect();
    assert_eq!(names, ["Pin", "Board"]);
    // Create offers the board, which opens.
    let creations = ui.contexts[0].creations.as_ref().unwrap().creations();
    assert_eq!((creations[0].kind, creations[0].opens), (board(), true));
}

#[test]
fn levels_choose_and_commands_are_undo_steps() {
    let (mut host, mut hosted) = host();
    hosted.with(|p, s| {
        let levels = p.contexts[0].levels.as_ref().unwrap();
        levels.choose(s, &mut host, 1);
        assert_eq!(levels.active(s, &host), [1]);
    });
    let shown = |hosted: &mut Hosting, host: &FakeHost| {
        hosted.with(|p, s| {
            let vocabulary = p.contexts[0].vocabulary.as_ref().unwrap();
            (
                vocabulary.shown(s, host, CLEAR),
                vocabulary.label(s, host, CLEAR),
            )
        })
    };
    assert!(!shown(&mut hosted, &host).0);
    run(&mut hosted, &mut host, PIN);
    run(&mut hosted, &mut host, PIN);
    assert_eq!(host.slot::<Pins>(), &Pins(2));
    assert_eq!(
        shown(&mut hosted, &host),
        (true, Some("Clear 2 Pins".into()))
    );
    run(&mut hosted, &mut host, CLEAR);
    assert_eq!(host.history(), ["Pin", "Pin", "Clear Pins"]);
    host.undo();
    assert_eq!(host.slot::<Pins>(), &Pins(2));
}

#[test]
fn the_inspector_shows_the_count() {
    let (mut host, mut hosted) = host();
    run(&mut hosted, &mut host, PIN);
    let mut ui = RecordingUi::new(Rect::from_min_size(Pos::ZERO, Vec2::new(300.0, 200.0)));
    hosted.with(|p, s| {
        let inspector = p.contexts[0].inspector.as_ref().unwrap();
        inspector.show(s, &mut host, &mut ui)
    });
    assert_eq!(ui.calls(), [Call::Sheet(vec!["Pins".into()])]);
}
