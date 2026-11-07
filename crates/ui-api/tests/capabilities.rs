//! The fake host's answers to the capabilities a context's tools lean on:
//! restarting a gesture, render views, Console records, the clipboard,
//! latest-wins jobs, the inspector's row hooks, the vocabulary's node
//! edit, a held node's base and result, and a library's viewport chrome.
use kinogaki_studio_api::Diagnostic;
use kinogaki_studio_api::network::{NetworkContext, NetworkEdit, NetworkRead, NewNode, Wire};
use kinogaki_studio_api::{
    ContextId, CookCx, Data, DataTypeId, Description, Error, HeldEdit, NodeRef, NodeType,
    NodeTypeId, Outputs, Package, PackageId, Port, Slot, SlotId, Typed,
};
use std::sync::LazyLock;
use std::sync::atomic::Ordering;
use studio_ui_api::Ui;
use studio_ui_api::gizmo::Pivot;
use studio_ui_api::host::{
    Cx, HostLatestExt, HostShell, HostSlotsExt, HostStatus, HostUndo, HostView, InspectorCx,
    LatestKey,
};
use studio_ui_api::package::{
    ContextPart, Inspector, Invoked, Library, RowAction, UiPackage, ViewportSpec, Vocabulary,
    WithUi,
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
    let gesture = host.begin_gesture("Adjust").expect("none was open");
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
    let tool = host.begin_gesture("Tool").expect("none was open");
    host.edit_slot("step", |slot| *slot = Count(1));
    assert!(host.begin_gesture("Drag").is_none(), "the tool's is open");
    host.edit_slot("step", |slot| *slot = Count(2));
    assert!(host.in_gesture(), "the drag's end leaves the tool's open");
    host.finish_gesture(tool).unwrap();
    assert_eq!(host.history(), ["Tool"]);
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
                .with_vocabulary(Edits),
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
fn a_held_node_takes_a_new_base_and_reports_its_result() {
    let package = Package::new(PackageId::declared("acme.counts"), "Counts")
        .with_context(NetworkContext::new(context(), "Counts", "counts"))
        .with_node_type(One)
        .with_node_type(Edit)
        .with_held_edit(AddOffset);
    let mut net = FakeNetwork::new(context(), &[&package]).unwrap();
    let one = net
        .add_node(
            NewNode::new(NodeTypeId::declared("acme.counts.one")),
            [0.0, 0.0],
        )
        .unwrap();
    let edit = net
        .add_node(
            NewNode::new(NodeTypeId::declared("acme.counts.edit")),
            [0.0, 1.0],
        )
        .unwrap();
    net.connect(
        Wire {
            node: one,
            output: 0,
        },
        edit,
        0,
    )
    .unwrap();
    net.set_delta(edit, OFFSET.wrap(0.5)).unwrap();
    let result = |net: &FakeNetwork| {
        *NUMBER
            .get(net.node(edit).unwrap().result.as_ref().unwrap())
            .unwrap()
    };
    assert_eq!(result(&net), 1.5);
    // Bake: the result becomes the base, the delta empties.
    let baked = net.node(edit).unwrap().result.unwrap();
    net.rebase_held(edit, baked).unwrap();
    net.set_delta(edit, OFFSET.wrap(0.0)).unwrap();
    assert_eq!(result(&net), 1.5);
    assert!(
        net.rebase_held(edit, OFFSET.wrap(1.0)).is_err(),
        "a base of another type"
    );
    assert!(
        net.rebase_held(one, NUMBER.wrap(1.0)).is_err(),
        "not a held edit"
    );
}
