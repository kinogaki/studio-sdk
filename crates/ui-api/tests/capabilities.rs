//! The fake host's answers to the capabilities a context's tools lean on:
//! restarting a gesture, render views, Console records, the clipboard,
//! latest-wins jobs, the inspector's row hooks, the vocabulary's node
//! edit, a held node's base and result, and a library's viewport chrome.
use kinogaki_studio_api::network::{NetworkContext, NetworkEdit, NetworkRead, NewNode, Wire};
use kinogaki_studio_api::params::Severity;
use kinogaki_studio_api::{
    ContextId, CookCx, CookError, Data, DataTypeId, Description, HeldEdit, NodeRef, NodeType,
    NodeTypeId, Outputs, Package, PackageId, Port, Slot, SlotId, Typed,
};
use std::sync::LazyLock;
use std::sync::atomic::Ordering;
use studio_ui_api::Ui;
use studio_ui_api::gizmo::Pivot;
use studio_ui_api::host::{
    Cx, HostShell, HostSlotsExt, HostStatus, HostUndo, HostView, InspectorCx, LatestResult,
};
use studio_ui_api::package::{
    ContextPart, Inspector, Invoked, Library, RowAction, UiPackage, ViewportSpec, Vocabulary,
    WithUi,
};
use studio_ui_api::testing::{FakeHost, FakeNetwork, HostCall, Hosting};

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
    FakeHost::new(context(), [kinogaki_studio_api::SessionSlot::of::<Count>()])
}

#[test]
fn a_gesture_back_at_its_start_puts_back_its_changes_and_stays_one_step() {
    let mut host = host();
    let gesture = host.begin_gesture("Adjust").expect("none was open");
    host.set_slot("step", Count(1));
    host.set_slot("step", Count(2));
    host.back_to(gesture.start());
    assert_eq!(
        host.slot::<Count>(),
        &Count(0),
        "back at the gesture's start"
    );
    assert!(host.in_gesture());
    host.set_slot("step", Count(5));
    let start = gesture.start();
    host.finish_gesture(gesture).unwrap();
    assert_eq!(host.history(), ["Adjust"]);
    host.undo();
    assert_eq!(host.slot::<Count>(), &Count(0));
    // With its gesture ended, nothing happens.
    host.set_slot("step", Count(3));
    host.back_to(start);
    assert_eq!(host.slot::<Count>(), &Count(3));
    assert!(host.take_calls().contains(&HostCall::BackTo(start)));
}

#[test]
fn a_gesture_begun_inside_another_is_part_of_it_and_only_its_opener_ends_it() {
    let mut host = host();
    let tool = host.begin_gesture("Tool").expect("none was open");
    host.set_slot("step", Count(1));
    assert!(host.begin_gesture("Drag").is_none(), "the tool's is open");
    host.set_slot("step", Count(2));
    assert!(host.in_gesture(), "the drag's end leaves the tool's open");
    host.finish_gesture(tool).unwrap();
    assert_eq!(host.history(), ["Tool"]);
}

#[test]
fn views_render_consoles_record_and_the_clipboard_holds_text() {
    let mut host = host();
    host.rendering.insert(1);
    assert!(host.rendering(1) && !host.rendering(0) && !host.rendering(9));
    host.console(Severity::Warning, Some("Edit Mesh 1"), "3 open edges");
    host.console(Severity::Info, None, "done");
    assert_eq!(host.paste_text(), None);
    host.copy_text("1, 2, 3");
    assert_eq!(host.paste_text().as_deref(), Some("1, 2, 3"));
    assert_eq!(
        host.take_calls(),
        [
            HostCall::Console(
                Severity::Warning,
                Some("Edit Mesh 1".into()),
                "3 open edges".into()
            ),
            HostCall::Console(Severity::Info, None, "done".into()),
            HostCall::CopyText("1, 2, 3".into()),
        ]
    );
}

#[test]
fn the_latest_job_of_a_key_wins_and_lands_once() {
    let mut host = host();
    let job = |n: u32| -> studio_ui_api::host::LatestJob {
        Box::new(move |token| {
            assert!(!token.is_cancelled());
            Box::new(n) as LatestResult
        })
    };
    host.latest("weights", job(1));
    host.latest("weights", job(2));
    host.latest("check", job(3));
    assert!(host.take_latest("weights").is_none(), "nothing landed yet");
    assert_eq!(host.run_latest(), 2, "the first weights job was cancelled");
    assert_eq!(host.wakes.load(Ordering::SeqCst), 2);
    let weights = host.take_latest("weights").unwrap();
    assert_eq!(weights.downcast_ref::<u32>(), Some(&2));
    assert!(host.take_latest("weights").is_none(), "given once");
    assert_eq!(
        *host
            .take_latest("check")
            .unwrap()
            .downcast::<u32>()
            .unwrap(),
        3
    );
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
    fn edit(&self, s: &mut Heard, _: &mut dyn Cx, node: NodeRef) {
        s.0.push(format!("edit {}", node.0));
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
                .with_vocabulary(Edits),
        ]
    }
}

#[test]
fn row_actions_typing_and_node_edits_reach_the_contexts_parts() {
    let mut host = host();
    let package = Package::new(PackageId::declared("acme.counts"), "Counts").with_ui(Counts);
    let mut hosted = Hosting::new(package, &host);
    hosted.with(|p, s| {
        let inspector = p.contexts[0].inspector.as_ref().unwrap();
        inspector.row_action(s, &mut host, NodeRef(4), "group", RowAction::Pick);
        inspector.typing(s, &mut host, NodeRef(4), "group", Some("top"));
        inspector.typing(s, &mut host, NodeRef(4), "group", None);
        let vocabulary = p.contexts[0].vocabulary.as_ref().unwrap();
        vocabulary.edit(s, &mut host, NodeRef(4));
    });
    assert_eq!(
        hosted.state::<Heard>().0,
        [
            "4 group Pick",
            "4 group Some(\"top\")",
            "4 group None",
            "edit 4"
        ]
    );
}

#[test]
fn a_library_without_a_viewport_spec_gets_cores_scene_view() {
    assert_eq!(Library::default().viewport, None);
    let scene = ViewportSpec::scene();
    assert!(scene.shared_rows.tint_back_faces && !scene.shared_rows.flat_shaded);
    assert_eq!(scene.default_pivot, Pivot::Origin);
    assert_eq!(scene.popovers, [studio_ui_api::package::Popover::Gear]);
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
    fn cook(&self, _: &mut CookCx<'_>, _: &[Data]) -> Result<Outputs, CookError> {
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
    fn cook(&self, _: &mut CookCx<'_>, inputs: &[Data]) -> Result<Outputs, CookError> {
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
    ) -> Result<Outputs, CookError> {
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
