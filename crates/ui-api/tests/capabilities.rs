//! The fake host's answers to the capabilities a context's tools lean on:
//! restarting a gesture, render views, Console records, the clipboard,
//! latest-wins jobs, the inspector's row hooks, a dropped Material, the
//! vocabulary's node edit, a held node's result over its input, and a
//! library's viewport chrome.
use kinogaki_studio_api::Diagnostic;
use kinogaki_studio_api::network::{NetworkContext, NetworkEdit, NetworkRead, NewNode, Wire};
use kinogaki_studio_api::{
    ContextId, CookCx, Data, DataTypeId, Description, Error, HeldEdit, NodeRef, NodeType,
    NodeTypeId, Opened, Outputs, Package, PackageId, Port, Slot, SlotId, Typed,
};
use std::sync::LazyLock;
use std::sync::atomic::Ordering;
use studio_ui_api::Ui;
use studio_ui_api::gizmo::Pivot;
use studio_ui_api::host::ObjectRef;
use studio_ui_api::host::{
    Cx, HostLatestExt, HostShell, HostSlotsExt, HostStatus, HostUndo, HostUndoExt, HostView,
    InspectorCx, LatestKey,
};
use studio_ui_api::package::{
    ContextPart, Inspector, Invoked, Library, Materials, RowAction, UiPackage, ViewportSpec,
    Vocabulary, WithUi,
};
use studio_ui_api_testing::{FakeHost, FakeNetwork, HostCall, Hosting};

fn context() -> ContextId {
    ContextId::declared("acme.counts.network")
}

/// A count, undone with the document.
#[derive(Clone, Debug, Default, PartialEq)]
struct Count(u32);

impl Slot for Count {
    fn id() -> SlotId {
        SlotId::declared("acme.counts.count")
    }
    fn initial() -> Self {
        Self::default()
    }
}

fn host() -> FakeHost {
    FakeHost::new(
        context(),
        &[kinogaki_studio_api::SessionSlot::of::<Count>()],
    )
}

#[test]
fn a_gesture_back_at_its_start_puts_back_its_changes_and_stays_one_step() {
    let mut host = host();
    let Opened::Own(gesture) = host.begin_gesture("Adjust").into_opened() else {
        panic!("none was open");
    };
    host.edit_slot("step", |slot| *slot = Count(1));
    host.edit_slot("step", |slot| *slot = Count(2));
    host.back_to(gesture.start());
    assert_eq!(
        host.slot::<Count>(),
        &Count(0),
        "back at the gesture's start"
    );
    assert!(host.in_gesture());
    host.edit_slot("step", |slot| *slot = Count(5));
    let start = gesture.start();
    host.finish_gesture(gesture).unwrap();
    assert_eq!(host.history(), ["Adjust"]);
    host.undo();
    assert_eq!(host.slot::<Count>(), &Count(0));
    // With its gesture ended, nothing happens.
    host.edit_slot("step", |slot| *slot = Count(3));
    host.back_to(start);
    assert_eq!(host.slot::<Count>(), &Count(3));
}

#[test]
fn a_gesture_begun_inside_another_is_part_of_it_and_only_its_opener_ends_it() {
    let mut host = host();
    use kinogaki_studio_api::Begun;
    let tool = host.begin_gesture("Tool").into_opened();
    assert!(matches!(tool, Opened::Own(_)), "none was open");
    host.edit_slot("step", |slot| *slot = Count(1));
    let drag = host.begin_gesture("Drag").into_opened();
    assert!(matches!(drag, Opened::Joined(_)), "the tool's is open");
    host.edit_slot("step", |slot| *slot = Count(2));
    host.put_back(Begun::new(drag));
    assert_eq!(host.slot::<Count>(), &Count(1), "back to where it joined");
    assert!(host.in_gesture(), "going back leaves the tool's open");
    host.keep(Begun::new(tool)).unwrap();
    assert_eq!(host.history(), ["Tool"]);
}

#[test]
fn a_gesture_opened_with_what_it_makes_is_put_back_when_making_fails() {
    let mut host = host();
    let refused = host.begin_gesture_with("Add", |host| {
        host.edit_slot("step", |slot| *slot = Count(1));
        Err::<(), _>(Error::invalid("refused"))
    });
    assert!(refused.is_err());
    assert!(!host.in_gesture(), "put back");
    assert_eq!(host.slot::<Count>(), &Count(0));
    let made = host.begin_gesture_with("Add", |host| {
        host.edit_slot("step", |slot| *slot = Count(2));
        Ok(7)
    });
    let (begun, seven) = made.expect("made");
    assert_eq!(seven, 7);
    host.keep(begun).unwrap();
    assert_eq!(host.history(), ["Add"]);
}

/// A `Begun` ends once: dropped without `keep`, `put_back` or `forget` it
/// would leave its gesture open, which a debug build says with a panic;
/// `forget` lets one go with its document.
#[test]
fn a_begun_dropped_unended_panics_in_a_debug_build_and_forget_lets_it_go() {
    let mut forgetting = host();
    forgetting.begin_gesture("Gone with its document").forget();
    let mut dropping = host();
    let dropped = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        drop(dropping.begin_gesture("Dropped"));
    }));
    assert_eq!(dropped.is_err(), cfg!(debug_assertions));
}

#[test]
fn views_render_consoles_record_and_the_clipboard_holds_text() {
    let mut host = host();
    host.rendering.insert(1);
    assert!(host.rendering(1) && !host.rendering(0) && !host.rendering(9));
    let (open, done) = (
        Diagnostic::warning("3 open edges"),
        Diagnostic::info("done"),
    );
    host.report(open.clone(), Some("Edit Mesh 1"));
    host.report(done.clone(), None);
    assert_eq!(host.paste_text(), None);
    host.copy_text("1, 2, 3");
    assert_eq!(host.paste_text().as_deref(), Some("1, 2, 3"));
    assert_eq!(
        host.take_calls(),
        [
            HostCall::Report(open, Some("Edit Mesh 1".into())),
            HostCall::Report(done, None),
            HostCall::CopyText("1, 2, 3".into()),
        ]
    );
}

/// Soft-selection weights, asked for a radius.
struct Weights;

impl LatestKey for Weights {
    type Asked = u32;
    type Out = u32;
}

/// A check of the shown value, asked for its revision.
struct Check;

impl LatestKey for Check {
    type Asked = u64;
    type Out = &'static str;
}

#[test]
fn the_newest_ask_of_a_kind_wins_lands_once_and_an_ask_again_runs_nothing() {
    let mut host = host();
    host.latest::<Weights>(1, |token| {
        assert!(token.is_cancelled(), "cancelled by the newer ask");
        1
    });
    host.latest::<Weights>(2, |_| 20);
    host.latest::<Check>(7, |_| "ok");
    assert!(
        host.take_latest::<Weights>().is_none(),
        "nothing landed yet"
    );
    assert_eq!(host.run_latest(), 2, "the first weights job was cancelled");
    assert_eq!(host.wakes.load(Ordering::SeqCst), 2);
    assert_eq!(host.take_latest::<Weights>(), Some((2, 20)), "with its ask");
    assert!(host.take_latest::<Weights>().is_none(), "given once");
    assert_eq!(host.take_latest::<Check>(), Some((7, "ok")));
    // The same ask again runs nothing: what it asked for stands.
    host.take_calls();
    host.latest::<Check>(7, |_| "again");
    assert_eq!(host.run_latest(), 0);
    assert!(host.take_calls().is_empty());
    // A new ask runs, and a result of the older one is never given.
    host.latest::<Check>(8, |_| "eight");
    host.latest::<Check>(9, |_| "nine");
    host.run_latest();
    assert_eq!(host.take_latest::<Check>(), Some((9, "nine")));
}

/// What the inspector heard.
#[derive(Default)]
struct Heard(Vec<String>);

struct Rows;

impl Inspector<Heard> for Rows {
    fn show(&self, _: &mut Heard, _: &mut dyn InspectorCx, _: &mut dyn Ui) {}
    fn row_action(
        &self,
        s: &mut Heard,
        _: &mut dyn Cx,
        node: NodeRef,
        row: &str,
        action: RowAction,
    ) {
        s.0.push(format!("{} {row} {action:?}", node.0));
    }
    fn typing(&self, s: &mut Heard, _: &mut dyn Cx, node: NodeRef, row: &str, text: Option<&str>) {
        s.0.push(format!("{} {row} {text:?}", node.0));
    }
}

struct Edits;

impl Vocabulary<Heard> for Edits {
    fn name(&self) -> &str {
        "acme.counts"
    }
    fn run(&self, _: &mut Heard, _: &mut dyn Cx, _: &str, _: Invoked) {}
}

struct Dropped;

impl Materials<Heard> for Dropped {
    fn assign(
        &self,
        s: &mut Heard,
        _: &mut dyn Cx,
        object: ObjectRef,
        path: &str,
    ) -> Result<(), Error> {
        match path {
            "" => Err(Error::invalid("no Material")),
            _ => {
                s.0.push(format!("{} {path}", object.0));
                Ok(())
            }
        }
    }
}

struct Counts;

impl UiPackage for Counts {
    type State = Heard;
    fn state(&self, _: studio_ui_api::host::Waker) -> Heard {
        Heard::default()
    }
    fn contexts(&self) -> Vec<ContextPart<Heard>> {
        vec![
            ContextPart::new(context())
                .with_inspector(Rows)
                .with_vocabulary(Edits)
                .with_materials(Dropped),
        ]
    }
}

#[test]
fn row_actions_and_typing_reach_the_contexts_parts() {
    let mut host = host();
    let package = Package::new(PackageId::declared("acme.counts"), "Counts").with_ui(Counts);
    let mut hosted = Hosting::new(package, &host);
    hosted.with(|p, s| {
        let inspector = p.contexts[0].inspector.as_ref().unwrap();
        inspector.row_action(s, &mut host, NodeRef(4), "group", RowAction::Pick);
        inspector.typing(s, &mut host, NodeRef(4), "group", Some("top"));
        inspector.typing(s, &mut host, NodeRef(4), "group", None);
    });
    assert_eq!(
        hosted.state::<Heard>().0,
        ["4 group Pick", "4 group Some(\"top\")", "4 group None",]
    );
}

#[test]
fn a_dropped_material_reaches_the_contexts_part_with_its_refusal() {
    let mut host = host();
    let package = Package::new(PackageId::declared("acme.counts"), "Counts").with_ui(Counts);
    let mut hosted = Hosting::new(package, &host);
    let mut refused = None;
    hosted.with(|p, s| {
        let materials = p.contexts[0].materials.as_ref().unwrap();
        materials
            .assign(s, &mut host, ObjectRef(3), "/Looks/Red")
            .unwrap();
        refused = materials.assign(s, &mut host, ObjectRef(3), "").err();
    });
    assert_eq!(hosted.state::<Heard>().0, ["3 /Looks/Red"]);
    assert_eq!(
        refused.map(|e| e.to_string()).as_deref(),
        Some("no Material")
    );
}

#[test]
fn a_library_without_a_viewport_spec_gets_cores_scene_view() {
    assert_eq!(Library::default().viewport, None);
    let scene = ViewportSpec::scene();
    assert_eq!(scene.default_pivot, Pivot::Origin);
    assert!(scene.display.is_empty(), "core's display rows alone");
}

static NUMBER: LazyLock<Typed<f64>> =
    LazyLock::new(|| Typed::new(DataTypeId::declared("acme.counts.number")));
static OFFSET: LazyLock<Typed<f64>> =
    LazyLock::new(|| Typed::new(DataTypeId::declared("acme.counts.offset")));

/// Makes 1.
struct One;

impl NodeType for One {
    fn id(&self) -> NodeTypeId {
        NodeTypeId::declared("acme.counts.one")
    }
    fn description(&self) -> Description {
        Description::new("One").context(context()).output(Port::new(
            "number",
            "Number",
            NUMBER.id(),
        ))
    }
    fn cook(&self, _: &mut CookCx<'_>, _: &[Data]) -> Result<Outputs, Error> {
        Ok(Outputs::one(NUMBER.wrap(1.0)))
    }
}

/// Holds an offset over its input.
struct Edit;

impl NodeType for Edit {
    fn id(&self) -> NodeTypeId {
        NodeTypeId::declared("acme.counts.edit")
    }
    fn description(&self) -> Description {
        Description::new("Edit")
            .context(context())
            .input(Port::new("number", "Number", NUMBER.id()))
            .output(Port::new("number", "Number", NUMBER.id()))
    }
    fn held(&self) -> Option<DataTypeId> {
        Some(OFFSET.id())
    }
    fn cook(&self, _: &mut CookCx<'_>, inputs: &[Data]) -> Result<Outputs, Error> {
        Ok(Outputs::one(inputs[0].clone()))
    }
}

struct AddOffset;

impl HeldEdit for AddOffset {
    fn base(&self) -> DataTypeId {
        NUMBER.id()
    }
    fn delta(&self) -> DataTypeId {
        OFFSET.id()
    }
    fn apply(
        &self,
        _: &mut CookCx<'_>,
        base: &Data,
        delta: &Data,
        _: &[Option<Data>],
    ) -> Result<Outputs, Error> {
        let sum = NUMBER.get(base).unwrap() + OFFSET.get(delta).unwrap();
        Ok(Outputs::one(NUMBER.wrap(sum)))
    }
}

#[test]
fn a_held_node_applies_its_delta_to_its_input_as_it_is_and_reports_its_result() {
    let package = Package::new(PackageId::declared("acme.counts"), "Counts")
        .with_context(NetworkContext::new(context(), "Counts", "counts"))
        .with_node_type(One)
        .with_node_type(Edit)
        .with_held_edit(AddOffset);
    let mut net = FakeNetwork::new(context(), &[&package]).unwrap();
    let add = |net: &mut FakeNetwork, id: &'static str, y: f32| {
        net.add_node(NewNode::new(NodeTypeId::declared(id)), [0.0, y])
            .unwrap()
    };
    let one = add(&mut net, "acme.counts.one", 0.0);
    let first = add(&mut net, "acme.counts.edit", 1.0);
    let edit = add(&mut net, "acme.counts.edit", 2.0);
    for (from, to) in [(one, first), (first, edit)] {
        net.connect(
            Wire {
                node: from,
                output: 0,
            },
            to,
            0,
        )
        .unwrap();
    }
    net.set_delta(first, OFFSET.wrap(2.0)).unwrap();
    net.set_delta(edit, OFFSET.wrap(0.5)).unwrap();
    let result = |net: &FakeNetwork| {
        *NUMBER
            .get(net.node(edit).unwrap().result.as_ref().unwrap())
            .unwrap()
    };
    assert_eq!(result(&net), 3.5);
    // An edit upstream reaches it: its delta applies to the input as it is.
    net.set_delta(first, OFFSET.wrap(10.0)).unwrap();
    assert_eq!(result(&net), 11.5);
    assert!(
        net.set_delta(edit, NUMBER.wrap(1.0)).is_err(),
        "a delta of another type"
    );
    assert!(
        net.set_delta(one, OFFSET.wrap(1.0)).is_err(),
        "not a held edit"
    );
}
