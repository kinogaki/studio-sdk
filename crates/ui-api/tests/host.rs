//! A toy package's UI on the fake host: registering erases its state's
//! type; its inspector, viewport layer, levels and creations run through
//! the host interface; its slot is undone with the document; its
//! settings section round-trips; and a nested call is refused.
use kinogaki_studio_api::element::{ElementReader, ElementWriter, Field, MemoryElement};
use kinogaki_studio_api::{ContextId, Error, ObjectKindId, Package, PackageId, Slot, SlotId};
use studio_ui_api::gizmo::TransformTool;
use studio_ui_api::host::{
    Cx, Ground, HostDoc, HostSettingsExt, HostSlotsExt, HostUndo, InspectorCx, NodeRef, ObjectRef,
    Place, View, ViewCamera, ViewCx, Waker,
};
use studio_ui_api::package::{
    ContextPart, Creation, Creations, Held, Inspector, KeyBinding, KeyStore, KeysActive, LevelSpec,
    Levels, Library, MarkingDefault, MenuDefaults, Places, Registered, Section, SettingsSection,
    Shared, Start, Status, ToolbarItem, Tools, UiPackage, ViewportLayer, VocabularyItem, When,
    WithUi,
};
use studio_ui_api::sheet::{Edits, Field as Row, Number, Schema};
use studio_ui_api::testing::{Call, FakeHost, HostCall, Hosting, RecordingUi};
use studio_ui_api::{
    ButtonSpec, Color, Modifiers, Pos, Projection, Rect, Response, Slot as PaintSlot, Stroke, Ui,
    Vec2,
};

fn forest() -> ContextId {
    ContextId::declared("toy.trees.forest")
}

fn scene() -> ContextId {
    ContextId::declared("toy.core.scene")
}

/// The trees picked: undoable, and starts over on another level.
#[derive(Clone, Debug, Default, PartialEq)]
struct Picked(Vec<u32>);

impl Slot for Picked {
    fn id() -> SlotId {
        SlotId::declared("toy.trees.picked")
    }
    fn initial() -> Self {
        Self::default()
    }
    const PLACE_BOUND: bool = true;
}

/// The package's UI state: what its parts remember between calls.
#[derive(Default)]
struct Forest {
    entered: u32,
    overlays: u32,
    polls: u32,
    level: usize,
    wake: Option<Waker>,
}

struct Trees;

impl UiPackage for Trees {
    type State = Forest;
    fn state(&self, wake: Waker) -> Forest {
        Forest {
            wake: Some(wake),
            ..Forest::default()
        }
    }
    fn contexts(&self) -> Vec<ContextPart<Forest>> {
        vec![
            ContextPart::new(forest())
                .with_library(library())
                .with_levels(TreeLevels)
                .with_viewport(TreeView)
                .with_inspector(TreeInspector)
                .with_creations(TreeCreations),
        ]
    }
    fn settings(&self) -> Vec<SettingsSection> {
        vec![SettingsSection::of::<Density>()]
    }
    fn poll(&self, s: &mut Forest, _cx: &mut dyn Cx) {
        s.polls += 1;
    }
}

/// Grow, bound to G, on A at the Tree level; at the Branch level only.
fn library() -> Library {
    Library {
        items: vec![VocabularyItem {
            name: "toy.trees.grow".into(),
            label: "Grow".into(),
            family: "Trees".into(),
            section: "Create".into(),
            when: When::levels(["Branch"]),
            places: Places {
                tools: true,
                menus: true,
                ..Places::default()
            },
            ..VocabularyItem::default()
        }],
        keys: Some(Shared::Own(KeyStore {
            id: "Trees".into(),
            label: "Trees".into(),
            parent: None,
            active: KeysActive::Always,
            defaults: vec![KeyBinding {
                item: "toy.trees.grow".into(),
                key: studio_ui_api::Key::G,
                modifiers: Modifiers::NONE,
            }],
        })),
        menus: Some(Shared::Own(MenuDefaults {
            store: "Trees".into(),
            titles: ["Grow".into(), "Prune".into(), "Select".into()],
            marking: vec![MarkingDefault {
                menu: Held::A,
                level: "Tree".into(),
                compass: [
                    Some("toy.trees.grow".into()),
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                    None,
                ],
                list: Vec::new(),
            }],
            shelves: Vec::new(),
        })),
        ..Library::default()
    }
}

struct TreeLevels;

impl Levels<Forest> for TreeLevels {
    fn levels(&self) -> Vec<LevelSpec> {
        ["Tree", "Branch"]
            .map(|name| LevelSpec {
                id: name.into(),
                label: name.into(),
                icon: "cube".into(),
                level: kinogaki_studio_api::ElementKind::Object,
                combines: false,
            })
            .into()
    }
    fn active(&self, s: &Forest, _cx: &dyn Cx) -> Vec<usize> {
        vec![s.level]
    }
    fn primary(&self, s: &Forest, _cx: &dyn Cx) -> usize {
        s.level
    }
    fn choose(&self, s: &mut Forest, cx: &mut dyn Cx, level: usize) {
        s.level = level;
        // Choosing a level converts the selection, within the open step.
        cx.slot_mut::<Picked>().0.clear();
    }
    fn toggle(&self, s: &mut Forest, cx: &mut dyn Cx, level: usize) {
        self.choose(s, cx, level);
    }
}

struct TreeView;

impl ViewportLayer<Forest> for TreeView {
    fn name(&self) -> &str {
        "Trees"
    }
    fn enter(&self, s: &mut Forest, _cx: &mut dyn Cx) {
        s.entered += 1;
    }
    fn toolbar(&self, _s: &Forest, cx: &dyn Cx) -> Vec<ToolbarItem> {
        vec![ToolbarItem {
            id: "grow".into(),
            icon: "plus".into(),
            label: "Grow".into(),
            on: cx.transform_tool() == TransformTool::Move,
            unavailable: None,
        }]
    }
    fn run_tool(&self, _s: &mut Forest, cx: &mut dyn Cx, id: &str) {
        if id == "grow" {
            cx.set_transform_tool(TransformTool::Move);
        }
    }
    fn status(&self, _s: &mut Forest, cx: &mut dyn Cx) -> Status {
        Status {
            left: vec![(format!("{} picked", cx.slot::<Picked>().0.len()), false)],
            ..Status::default()
        }
    }
    fn tools(&self, _s: &mut Forest, cx: &mut dyn ViewCx, _ui: &mut dyn Ui) -> Tools {
        Tools {
            busy: cx.view().modifiers.shift,
            ..Tools::default()
        }
    }
    fn overlay(&self, s: &mut Forest, cx: &mut dyn ViewCx, ui: &mut dyn Ui) {
        s.overlays += 1;
        let Some(at) = cx.view().pointer else {
            return;
        };
        let stroke = Stroke::new(1.0, Color::WHITE);
        ui.painter().circle_stroke(at, 4.0, stroke);
    }
}

struct TreeInspector;

impl Inspector<Forest> for TreeInspector {
    fn show(&self, _s: &mut Forest, cx: &mut dyn InspectorCx, ui: &mut dyn Ui) {
        let picked = cx.slot::<Picked>().0.clone();
        ui.label(&format!("{} trees", picked.len()), Default::default());
        if ui.button(&ButtonSpec::new("Pick")).clicked {
            let next = picked.len() as u32;
            let mut more = picked;
            more.push(next);
            cx.set_slot("Pick Tree", Picked(more));
        }
    }
}

struct TreeCreations;

impl Creations<Forest> for TreeCreations {
    fn creations(&self) -> Vec<Creation> {
        vec![Creation {
            kind: ObjectKindId::declared("toy.trees.object"),
            label: "Forest".into(),
            aliases: vec!["woods".into()],
            opens: true,
            root_only: None,
            starts: vec![Start {
                id: "toy.trees.oak".into(),
                label: "Oak".into(),
                family: "Trees".into(),
                aliases: Vec::new(),
            }],
        }]
    }
    fn start(
        &self,
        _s: &mut Forest,
        cx: &mut dyn Cx,
        _kind: ObjectKindId,
        start: &str,
    ) -> Result<(), String> {
        cx.info(&format!("planted {start}"));
        Ok(())
    }
}

/// The forest's user settings: how dense new forests grow.
#[derive(Clone, Debug, PartialEq)]
struct Density(f64);

impl Section for Density {
    fn context() -> ContextId {
        forest()
    }
    fn pages() -> Vec<String> {
        vec!["Forest".into()]
    }
    fn defaults() -> Self {
        Density(0.5)
    }
    fn page(&self, _page: &str) -> Schema<u32> {
        let mut s = Schema::new();
        s.push(Row::number(0, "Density", self.0, Number::length()));
        s
    }
    fn apply(&mut self, _page: &str, rows: &Schema<u32>, edits: &Edits<u32>) {
        if edits.changed.contains(&0) {
            self.0 = rows.number(&0).unwrap_or(self.0);
        }
    }
    fn write(&self, to: &mut dyn ElementWriter) -> Result<(), Error> {
        to.set("density", Field::Float(self.0))
    }
    fn read(from: &dyn ElementReader) -> Result<Self, Error> {
        Ok(Density(from.float("density")?))
    }
}

fn host() -> FakeHost {
    let mut host = FakeHost::new(scene(), package().slots);
    host.place = Place {
        context: forest(),
        object: Some(ObjectRef(7)),
        objects: false,
        subnet: None,
        node: None,
    };
    host
}

/// The package: its slot, and its UI.
fn package() -> Package {
    Package::new(PackageId::declared("toy.trees"), "Trees")
        .with_slot::<Picked>()
        .with_ui(Trees)
}

fn hosted(host: &FakeHost) -> Hosting {
    Hosting::new(package(), host)
}

fn rect() -> Rect {
    Rect::from_min_size(Pos::ZERO, Vec2::new(400.0, 300.0))
}

fn view(pointer: Option<Pos>, modifiers: Modifiers) -> View {
    View {
        rect: rect(),
        index: 0,
        interactive: true,
        modal: false,
        response: Response::new(Default::default(), rect()),
        over_chrome: false,
        over_cube: false,
        pointer,
        modifiers,
        stacking: false,
        under: PaintSlot(0),
        camera: ViewCamera {
            projection: Projection {
                clip_from_object: Default::default(),
                object_from_clip: Default::default(),
                backward: kinogaki_studio_api::Vec3::new(0.0, 0.0, 1.0),
            },
            eye: kinogaki_studio_api::Vec3::new(0.0, 0.0, 10.0),
            target: kinogaki_studio_api::Vec3::ZERO,
            orthographic: false,
            ground: Ground {
                origin: kinogaki_studio_api::Vec3::ZERO,
                normal: kinogaki_studio_api::Vec3::new(0.0, 1.0, 0.0),
                step: 1.0,
            },
        },
    }
}

#[test]
fn registering_reads_the_contexts_once_and_erases_the_state() {
    let whole = package();
    // One id, label and set of slots: the package's.
    assert_eq!(whole.id, PackageId::declared("toy.trees"));
    assert_eq!(whole.label, "Trees");
    assert_eq!(whole.slots[0].id(), Picked::id());
    // Its UI names the contexts it gives parts to, for the registry.
    assert_eq!(whole.ui.as_ref().unwrap().contexts(), [(forest(), false)]);
    let package = Registered::of(&whole).unwrap();
    assert!(Registered::of(&Package::new(PackageId::declared("toy.bare"), "Bare")).is_none());
    let context = package.context(forest(), false).unwrap();
    assert!(context.levels.is_some() && context.viewport.is_some());
    assert!(context.vocabulary.is_none());
    assert!(package.context(forest(), true).is_none());
    assert_eq!(package.settings[0].context(), forest());
    let levels = context.levels.as_ref().unwrap().levels();
    assert_eq!(levels.len(), 2);
    // The library is data, kept as given.
    assert_eq!(context.library, library());
    let grow = &context.library.items[0];
    assert!(grow.when.holds(Some("Branch"), false, false));
    assert!(!grow.when.holds(Some("Tree"), false, false));
}

#[test]
fn an_inspector_click_is_one_undo_step_of_its_slot() {
    let mut host = host();
    let mut trees = hosted(&host);
    let show = |trees: &mut Hosting, host: &mut FakeHost, ui: &mut RecordingUi| {
        trees.with(|p, s| {
            let inspector = p.contexts[0].inspector.as_ref().unwrap();
            inspector.show(s, host, ui);
        })
    };
    let mut ui = RecordingUi::new(rect());
    ui.click("Pick");
    show(&mut trees, &mut host, &mut ui);
    assert_eq!(host.slot::<Picked>(), &Picked(vec![0]));
    assert_eq!(host.history(), ["Pick Tree"]);
    assert_eq!(
        host.take_calls(),
        [HostCall::SetSlot(Picked::id(), "Pick Tree".into())]
    );
    let mut ui = RecordingUi::new(rect());
    show(&mut trees, &mut host, &mut ui);
    assert_eq!(ui.calls()[0], Call::Label("1 trees".into()));
    // Undo restores the slot; redo brings it back.
    host.undo();
    assert_eq!(host.slot::<Picked>(), &Picked::default());
    assert_eq!(host.redo_label().as_deref(), Some("Pick Tree"));
    host.redo();
    assert_eq!(host.slot::<Picked>(), &Picked(vec![0]));
}

#[test]
fn a_gesture_is_one_step_and_a_cancelled_one_none() {
    let mut host = host();
    let mut trees = hosted(&host);
    host.set_slot("Pick", Picked(vec![1, 2]));
    // A level change inside a gesture rides along with it.
    host.begin_gesture("Change Level");
    trees.with(|p, s| {
        p.contexts[0]
            .levels
            .as_ref()
            .unwrap()
            .choose(s, &mut host, 1)
    });
    host.finish_gesture();
    assert_eq!(host.history(), ["Pick", "Change Level"]);
    assert_eq!(trees.state::<Forest>().level, 1);
    host.begin_gesture("Drag");
    host.slot_mut::<Picked>().0.push(9);
    host.cancel_gesture();
    assert_eq!(host.slot::<Picked>(), &Picked::default());
    assert_eq!(host.history(), ["Pick", "Change Level"]);
    // Undo restores the slot, not the package's own state.
    host.undo();
    assert_eq!(host.slot::<Picked>(), &Picked(vec![1, 2]));
    assert_eq!(trees.state::<Forest>().level, 1);
}

#[test]
fn a_viewport_layer_reads_its_view_and_draws_through_the_painter() {
    let mut host = host();
    let mut trees = hosted(&host);
    host.view = Some(view(Some(Pos::new(10.0, 20.0)), Modifiers::SHIFT));
    let mut ui = RecordingUi::new(rect());
    let tools = trees.with(|p, s| {
        let layer = p.contexts[0].viewport.as_ref().unwrap();
        layer.enter(s, &mut host);
        let tools = layer.tools(s, &mut host, &mut ui);
        layer.overlay(s, &mut host, &mut ui);
        tools
    });
    assert!(tools.busy);
    assert_eq!(ui.painter_record().shapes().len(), 1);
    assert_eq!(trees.state::<Forest>().entered, 1);
    assert_eq!(trees.state::<Forest>().overlays, 1);
    // Toolbar, tools and status through the host's transform and slots.
    let (before, status) = trees.with(|p, s| {
        let layer = p.contexts[0].viewport.as_ref().unwrap();
        let before = layer.toolbar(s, &host);
        layer.run_tool(s, &mut host, "grow");
        (before, layer.status(s, &mut host))
    });
    assert!(!before[0].on);
    assert_eq!(
        host.take_calls(),
        [HostCall::SetTransformTool(TransformTool::Move)]
    );
    assert_eq!(status.left, [("0 picked".to_owned(), false)]);
    let after = trees.with(|p, s| p.contexts[0].viewport.as_ref().unwrap().toolbar(s, &host));
    assert!(after[0].on);
}

#[test]
fn creations_start_objects_through_the_package() {
    let mut host = host();
    let mut trees = hosted(&host);
    let kind = ObjectKindId::declared("toy.trees.object");
    let started = trees.with(|p, s| {
        let creations = p.contexts[0].creations.as_ref().unwrap();
        assert_eq!(creations.creations()[0].starts[0].label, "Oak");
        creations.start(s, &mut host, kind, "toy.trees.oak")
    });
    assert_eq!(started, Ok(()));
    assert_eq!(host.calls, [HostCall::Info("planted toy.trees.oak".into())]);
}

#[test]
fn another_level_starts_place_bound_slots_over() {
    let mut host = host();
    host.set_slot("Pick", Picked(vec![3]));
    let mut place = host.place;
    place.subnet = Some(NodeRef(2));
    host.go_to(place).unwrap();
    assert_eq!(host.slot::<Picked>(), &Picked(vec![3]));
    place.object = Some(ObjectRef(8));
    host.go_to(place).unwrap();
    assert_eq!(host.slot::<Picked>(), &Picked::default());
}

#[test]
fn polls_reach_the_state_and_workers_wake_the_host() {
    let mut host = host();
    let mut trees = hosted(&host);
    (trees.state::<Forest>().wake.as_ref().unwrap())();
    assert_eq!(host.wakes.load(std::sync::atomic::Ordering::SeqCst), 1);
    trees.poll(&mut host);
    assert_eq!(trees.state::<Forest>().polls, 1);
}

#[test]
fn a_settings_section_edits_its_draft_and_round_trips() {
    let section = SettingsSection::of::<Density>();
    let mut draft = section.defaults();
    let value = std::sync::Arc::get_mut(&mut draft).unwrap();
    // The host's sheet reports the edited row with its new value in place.
    let rows = Density(0.8).page("Forest");
    let edits = Edits {
        changed: vec![0],
        ..Edits::default()
    };
    section.apply(value, "Forest", &rows, &edits);
    assert!(!section.eq(&*draft, &*section.defaults()));
    let mut element = MemoryElement::new("forest");
    element.write(|w| section.write(&*draft, w)).unwrap();
    let back = element.read(|r| section.read(r)).unwrap();
    assert!(section.eq(&*back, &*draft));
    assert_eq!(back.downcast_ref::<Density>(), Some(&Density(0.8)));
    // The host gives the applied section back typed.
    let mut host = host();
    host.sections.insert(forest(), back);
    assert_eq!(host.section::<Density>(forest()), Some(&Density(0.8)));
    let empty = MemoryElement::new("forest");
    assert!(empty.read(|r| section.read(r)).is_err());
}

/// The registry checks a UI's contexts like every id a package declares:
/// under the package's vendor, and given parts by one package alone.
#[test]
fn the_registry_admits_a_ui_s_contexts_once_and_under_its_vendor() {
    use kinogaki_studio_api::{Registry, RegistryError};
    let mut registry = Registry::new();
    registry.register(package()).unwrap();
    let groves = PackageId::declared("toy.groves");
    assert_eq!(
        registry.check(&Package::new(groves, "Groves").with_ui(Trees)),
        Err(RegistryError::DuplicateId {
            package: groves,
            what: "context's UI",
            id: "toy.trees.forest",
            first: PackageId::declared("toy.trees"),
        })
    );
    let acme = PackageId::declared("acme.trees");
    assert_eq!(
        Registry::new().check(&Package::new(acme, "Trees").with_ui(Trees)),
        Err(RegistryError::ForeignId {
            package: acme,
            id: "toy.trees.forest",
        })
    );
}
