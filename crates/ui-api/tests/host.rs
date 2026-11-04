//! A toy package's UI on the fake host: registering erases its state's
//! type; its inspector, viewport layer and levels run through
//! the host interface; its slot is undone with the document; its
//! settings pages edit the stored values they are lent; a tool's card shows its node's rows
//! through the host, with More while they are fewer than the inspector's;
//! and a nested call is refused.
use kinogaki_studio_api::RowsFor;
use kinogaki_studio_api::element::{ElementReader, ElementWriter, Field};
use kinogaki_studio_api::{
    ContextId, Error, ObjectKindId, Package, PackageId, Scope, Slot, SlotId, Stored, StoredId,
    StoredValue, StoredValues,
};
use studio_ui_api::host::{
    Cx, HostDoc, HostDocExt, HostSlotsExt, HostUndo, InspectorCx, NodeRef, NodeRows, ObjectRef,
    Place, View, ViewCx, Waker,
};
use studio_ui_api::package::hosting::Registered;
use studio_ui_api::package::{
    ContextPart, Display, DisplayKind, DisplayOption, DisplayScope, Edited, Ended, Held, Inspector,
    KeyBinding, KeyStore, KeysActive, LevelSpec, Levels, Library, LiveTools, MarkingDefault,
    MenuDefaults, PageRows, Panel, PanelScope, Panels, Places, SettingsPage, Shared, Situation,
    Takes, UiPackage, VocabularyItem, When, WithUi,
};
use studio_ui_api::sheet::{Edits, Field as Row, Number, Schema};
use studio_ui_api::{ButtonSpec, Color, Modifiers, Painter, Pos, Rect, Stroke, Ui, Vec2};
use studio_ui_api_testing::{Call, FakeHost, HostCall, Hosting, RecordingUi};

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
    overlays: u32,
    polls: u32,
    /// How many times a settings page was drawn (it shows live state).
    pages: u32,
    level: usize,
    wake: Option<Waker>,
    /// The node its Plant tool adjusts while its card is up.
    planting: Option<NodeRef>,
    /// The card's More is on.
    more: bool,
    /// The card's rows were fewer than the inspector's last time.
    hidden: bool,
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
                .with_tools(TreeView)
                .with_panels(TreeView)
                .with_display(TreeView)
                .with_inspector(TreeInspector),
        ]
    }
    fn settings(&self) -> Vec<SettingsPage<Forest>> {
        vec![
            SettingsPage::user("Forest", DensityPage),
            SettingsPage::project("Trees", SpacingPage),
        ]
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
            objects: None,
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
                mode: kinogaki_studio_api::geometry::Domain::Object.into(),
                combines: false,
                double: Default::default(),
            })
            .into()
    }
    fn active(&self, s: &Forest, _cx: &dyn Cx) -> Vec<usize> {
        vec![s.level]
    }
    fn primary(&self, s: &Forest, _cx: &dyn Cx) -> usize {
        s.level
    }
    fn choose(&self, s: &mut Forest, cx: &mut dyn Cx, levels: &[usize]) {
        s.level = levels[0];
        // Choosing a level converts the selection, within the open step.
        cx.edit_slot("Change Level", |p: &mut Picked| p.0.clear());
    }
    fn toggle(&self, s: &mut Forest, cx: &mut dyn Cx, level: usize) {
        self.choose(s, cx, &[level]);
    }
}

struct TreeView;

/// The forest's per-view display option: leaves drawn over the trees.
const LEAVES: DisplayOption = DisplayOption {
    id: "toy.trees.leaves",
    label: "Leaves",
    scope: DisplayScope::PerView,
    kind: DisplayKind::Toggle { on: false },
};

/// Shift takes the views' picks.
impl LiveTools<Forest> for TreeView {
    fn tools(&self, _s: &mut Forest, cx: &mut dyn ViewCx) -> Takes {
        Takes {
            picks: cx.input().modifiers.shift,
            ..Takes::default()
        }
    }
    fn cancel(&self, s: &mut Forest, _cx: &mut dyn Cx, _why: Ended) {
        s.planting = None;
    }
    fn accept(&self, s: &mut Forest, _cx: &mut dyn Cx) {
        s.planting = None;
    }
}

/// A ring at the pointer; leaves over the trees in the views showing
/// them.
impl Display<Forest> for TreeView {
    fn edited(&self, _s: &Forest, cx: &dyn ViewCx) -> Option<Edited> {
        (cx.displayed(LEAVES.id) == 1).then(Edited::default)
    }
    fn settle(&self, s: &mut Forest, _cx: &mut dyn Cx) {
        s.overlays += 1;
    }
    fn overlay(&self, _s: &Forest, cx: &dyn ViewCx, painter: &dyn Painter) {
        let Some(at) = cx.input().pointer.hover else {
            return;
        };
        painter.circle_stroke(at, 4.0, Stroke::new(1.0, Color::WHITE));
    }
}

/// Plant's card, over the view's top left corner while it runs.
impl Panels<Forest> for TreeView {
    fn panels(&self, s: &Forest, cx: &dyn ViewCx) -> Vec<Panel> {
        let at = cx.view().rect.min;
        let card = Panel {
            id: studio_ui_api::Id::new("toy.trees.plant"),
            rect: Rect::from_min_size(at, Vec2::new(120.0, 80.0)),
            scope: PanelScope::View,
        };
        s.planting.map(|_| card).into_iter().collect()
    }
    /// Its node's rows, More while some are left out, Done ending it.
    fn panel(&self, s: &mut Forest, cx: &mut dyn Cx, panel: &Panel, ui: &mut dyn Ui) {
        let (Some(node), Some(object)) = (s.planting, cx.place().object()) else {
            return;
        };
        let spec = studio_ui_api::CardSpec {
            id: panel.id,
            rect: panel.rect,
            bounds: panel.rect,
            title: "Plant",
            more: (s.more || s.hidden).then_some(s.more),
        };
        let purpose = RowsFor::Card { more: s.more };
        let mut drawn = NodeRows::default();
        let card = ui.card(&spec, &mut |ui| {
            drawn = cx.node_rows(ui, object, node, purpose);
        });
        s.hidden = drawn.hidden;
        if drawn.changed {
            cx.ran(node);
        }
        s.more ^= card.toggle_more;
        if card.done {
            s.planting = None;
        }
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
            cx.edit_slot("Pick Tree", |slot| *slot = Picked(more));
        }
    }
}

/// The forest's user setting: how dense new forests grow.
#[derive(Clone, Debug, PartialEq)]
struct Density(f64);

impl Stored for Density {
    fn id() -> StoredId {
        StoredId::declared("toy.trees.density")
    }
    fn scope() -> Scope {
        Scope::User
    }
    fn defaults() -> Self {
        Density(0.5)
    }
    fn write(&self, to: &mut dyn ElementWriter) -> Result<(), Error> {
        to.set("density", Field::Float(self.0))
    }
    fn read(from: &dyn ElementReader) -> Result<Self, Error> {
        Ok(Density(from.float("density")?))
    }
}

/// The Forest page of the user's settings: the density.
struct DensityPage;

impl PageRows<Forest> for DensityPage {
    fn rows(&self, s: &mut Forest, user: &StoredValues, _cx: &mut dyn Cx) -> Schema<u32> {
        s.pages += 1;
        let density = user.get::<Density>().map_or(0.0, |d| d.0);
        let mut rows = Schema::new();
        rows.push(Row::number(0, "Density", density, Number::length()));
        rows
    }
    fn apply(
        &self,
        _s: &mut Forest,
        user: &mut StoredValues,
        rows: &Schema<u32>,
        edits: &Edits<u32>,
        cx: &mut dyn Cx,
    ) {
        if edits.changed.contains(&0)
            && let Some(v) = rows.number(&0)
        {
            user.set(Density(v));
            cx.info("Density set");
        }
    }
}

/// The forest's spacing, a project setting of its own.
#[derive(Clone, Debug, PartialEq)]
struct Spacing(f64);

impl kinogaki_studio_api::Stored for Spacing {
    fn id() -> kinogaki_studio_api::StoredId {
        kinogaki_studio_api::StoredId::declared("toy.trees.spacing")
    }
    fn scope() -> kinogaki_studio_api::Scope {
        kinogaki_studio_api::Scope::Project
    }
    fn defaults() -> Self {
        Spacing(2.0)
    }
    fn write(&self, to: &mut dyn ElementWriter) -> Result<(), Error> {
        to.set("spacing", Field::Float(self.0))
    }
    fn read(from: &dyn ElementReader) -> Result<Self, Error> {
        Ok(Spacing(from.float("spacing")?))
    }
}

/// Settings ▸ Project's Trees section: the spacing.
struct SpacingPage;

impl PageRows<Forest> for SpacingPage {
    fn rows(&self, s: &mut Forest, project: &StoredValues, _cx: &mut dyn Cx) -> Schema<u32> {
        s.pages += 1;
        let spacing = project.get::<Spacing>().map_or(0.0, |s| s.0);
        let mut rows = Schema::new();
        rows.push(Row::number(0, "Spacing", spacing, Number::length()));
        rows
    }
    fn apply(
        &self,
        _s: &mut Forest,
        project: &mut StoredValues,
        rows: &Schema<u32>,
        edits: &Edits<u32>,
        _cx: &mut dyn Cx,
    ) {
        if edits.changed.contains(&0)
            && let Some(v) = rows.number(&0)
        {
            project.set(Spacing(v));
        }
    }
}

/// Inside `object` of the forest, with `subnet` open.
fn inside(object: ObjectRef, subnet: Option<NodeRef>) -> Place {
    Place::Inside {
        context: forest(),
        object,
        subnet,
        node: None,
    }
}

fn host() -> FakeHost {
    let mut host = FakeHost::new(scene(), package().slots);
    host.place = inside(ObjectRef(7), None);
    host
}

/// The package: its slot, and its UI.
fn package() -> Package {
    Package::new(PackageId::declared("toy.trees"), "Trees")
        .with_slot::<Picked>()
        .with_stored::<Density>()
        .with_stored::<Spacing>()
        .with_ui(Trees)
}

fn hosted(host: &FakeHost) -> Hosting {
    Hosting::new(package(), host)
}

fn rect() -> Rect {
    Rect::from_min_size(Pos::ZERO, Vec2::new(400.0, 300.0))
}

/// The trees' view: the testing crate's, at [`rect`].
fn view() -> View {
    studio_ui_api_testing::view(rect())
}

#[test]
fn registering_reads_the_contexts_once_and_erases_the_state() {
    let whole = package();
    // One id, label and set of slots: the package's.
    assert_eq!(whole.id, PackageId::declared("toy.trees"));
    assert_eq!(whole.label, "Trees");
    assert_eq!(whole.slots[0].id(), Picked::id());
    let package = Registered::of(&whole).unwrap();
    assert!(Registered::of(&Package::new(PackageId::declared("toy.bare"), "Bare")).is_none());
    let context = package.context(forest()).unwrap();
    assert!(context.levels.is_some() && context.tools.is_some() && context.display.is_some());
    assert!(context.vocabulary.is_none());
    let pages: Vec<_> = (package.settings.iter())
        .map(|p| (p.scope(), p.label.as_str()))
        .collect();
    assert_eq!(pages, [(Scope::User, "Forest"), (Scope::Project, "Trees")]);
    let levels = context.levels.as_ref().unwrap().levels();
    assert_eq!(levels.len(), 2);
    // The library is data, kept as given.
    assert_eq!(context.library, library());
    let grow = &context.library.items[0];
    assert!(grow.when.holds(&Situation {
        level: Some("Branch"),
        ..Situation::default()
    }));
    assert!(!grow.when.holds(&Situation {
        level: Some("Tree"),
        ..Situation::default()
    }));
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
        [HostCall::EditSlot(Picked::id(), "Pick Tree".into())]
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
    host.edit_slot("Pick", |slot| *slot = Picked(vec![1, 2]));
    // A level change inside a gesture rides along with it.
    let level = host.begin_gesture("Change Level").unwrap();
    trees.with(|p, s| {
        p.contexts[0]
            .levels
            .as_ref()
            .unwrap()
            .choose(s, &mut host, &[1])
    });
    host.finish_gesture(level).unwrap();
    assert_eq!(host.history(), ["Pick", "Change Level"]);
    assert_eq!(trees.state::<Forest>().level, 1);
    let drag = host.begin_gesture("Drag").unwrap();
    host.edit_slot("Drag", |p: &mut Picked| p.0.push(9));
    host.cancel_gesture(drag);
    assert_eq!(host.slot::<Picked>(), &Picked::default());
    assert_eq!(host.history(), ["Pick", "Change Level"]);
    // Undo restores the slot, not the package's own state.
    host.undo();
    assert_eq!(host.slot::<Picked>(), &Picked(vec![1, 2]));
    assert_eq!(trees.state::<Forest>().level, 1);
}

#[test]
fn the_viewport_parts_read_the_views_input_and_draw_through_the_painter() {
    let mut host = host();
    let mut trees = hosted(&host);
    host.view = Some(view());
    host.input.pointer.hover = Some(Pos::new(10.0, 20.0));
    host.input.modifiers = Modifiers::SHIFT;
    let painter = studio_ui_api_testing::RecordingPainter::new(rect());
    let takes = trees.with(|p, s| {
        let context = &p.contexts[0];
        let takes = context.tools.as_ref().unwrap().tools(s, &mut host);
        let display = context.display.as_ref().unwrap();
        display.settle(s, &mut host);
        display.overlay(s, &host, &painter);
        takes
    });
    assert_eq!(
        takes,
        Takes {
            picks: true,
            ..Takes::default()
        }
    );
    assert_eq!(painter.shapes().len(), 1);
    assert_eq!(trees.state::<Forest>().overlays, 1, "settled once");
}

#[test]
fn a_views_calls_answer_its_own_display_options_and_place() {
    let mut host = host();
    let mut trees = hosted(&host);
    host.displayed.insert((1, LEAVES.id.into()), 1);
    trees.with(|_, s| s.downcast_mut::<Forest>().unwrap().planting = Some(NodeRef(3)));
    let at = |index: usize| View {
        index,
        rect: Rect::from_min_size(Pos::new(index as f32 * 400.0, 0.0), Vec2::new(400.0, 300.0)),
        ..view()
    };
    let asked = |host: &mut FakeHost, trees: &mut Hosting, index| {
        host.view = Some(at(index));
        trees.with(|p, s| {
            let context = &p.contexts[0];
            let panels = context.panels.as_ref().unwrap().panels(s, host);
            let edited = context.display.as_ref().unwrap().edited(s, host);
            (edited.is_some(), panels)
        })
    };
    let (leaves, panels) = asked(&mut host, &mut trees, 0);
    assert!(!leaves, "view 0 has them off");
    assert_eq!(panels[0].rect.min, Pos::ZERO);
    let (leaves, panels) = asked(&mut host, &mut trees, 1);
    assert!(leaves, "view 1 has them on");
    assert_eq!(
        panels[0].rect.min,
        Pos::new(400.0, 0.0),
        "over its own view"
    );
}

#[test]
fn another_level_starts_place_bound_slots_over() {
    let mut host = host();
    host.edit_slot("Pick", |slot| *slot = Picked(vec![3]));
    host.go_to(inside(ObjectRef(7), Some(NodeRef(2)))).unwrap();
    assert_eq!(host.slot::<Picked>(), &Picked(vec![3]));
    host.go_to(inside(ObjectRef(8), Some(NodeRef(2)))).unwrap();
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

/// Each page draws its rows over the draft of its scope's values, lent
/// the package's state, and writes the edits it is told of into the draft;
/// the host then gives the values back typed.
#[test]
fn settings_pages_edit_the_stored_values_they_are_lent() {
    let mut host = host();
    let mut trees = hosted(&host);
    let edits = Edits {
        changed: vec![0],
        ..Edits::default()
    };
    let edited = |value| {
        let mut rows = Schema::new();
        rows.push(Row::number(0, "", value, Number::length()));
        rows
    };
    let mut user = StoredValues::new([StoredValue::of::<Density>()]).unwrap();
    let mut project = StoredValues::new([StoredValue::of::<Spacing>()]).unwrap();
    let drawn = trees.with(|ui, s| ui.settings[0].rows.rows(s, &user, &mut host));
    assert_eq!(drawn.number(&0), Some(0.5));
    let drawn = trees.with(|ui, s| ui.settings[1].rows.rows(s, &project, &mut host));
    assert_eq!(drawn.number(&0), Some(2.0));
    assert_eq!(trees.state::<Forest>().pages, 2);
    let rows = edited(0.8);
    trees.with(|ui, s| {
        ui.settings[0]
            .rows
            .apply(s, &mut user, &rows, &edits, &mut host)
    });
    assert!(
        host.take_calls()
            .contains(&HostCall::Info("Density set".into()))
    );
    let rows = edited(3.5);
    trees.with(|ui, s| {
        ui.settings[1]
            .rows
            .apply(s, &mut project, &rows, &edits, &mut host)
    });
    assert_eq!(user.get::<Density>(), Some(&Density(0.8)));
    assert_eq!(project.get::<Spacing>(), Some(&Spacing(3.5)));
    (host.user_stored, host.stored) = (user, project);
    assert_eq!(host.stored::<Density>(), &Density(0.8));
    assert_eq!(host.stored::<Spacing>(), &Spacing(3.5));
}

/// The registry checks a UI's contexts like every id a package declares:
/// under the package's vendor, and given parts by one package alone.
#[test]
fn the_registry_admits_a_ui_s_contexts_once_and_under_its_vendor() {
    use kinogaki_studio_api::{Error, Registry};
    let mut registry = Registry::new();
    registry.register(package()).unwrap();
    let groves = PackageId::declared("toy.groves");
    assert_eq!(
        registry.check(&Package::new(groves, "Groves").with_ui(Trees)),
        Err(Error::refused(format_args!(
            "{package} declares {what} {id}, which {first} declared",
            package = groves,
            what = "context's UI",
            id = "toy.trees.forest",
            first = PackageId::declared("toy.trees")
        )))
    );
    let acme = PackageId::declared("acme.trees");
    assert_eq!(
        Registry::new().check(&Package::new(acme, "Trees").with_ui(Trees)),
        Err(Error::refused(format_args!(
            "{package} declares {id}, under another vendor's name",
            package = acme,
            id = "toy.trees.forest"
        )))
    );
}

/// A fake host hands a context the drawn polygons it was given for an
/// object (a B-rep's join: each polygon's `face` naming the value's face),
/// sharing them; none for an object it draws no polygons for.
#[test]
fn shown_polygons_are_the_ones_the_object_draws() {
    use kinogaki_studio_api::geometry::{Attribute, Domain, Meaning};
    let mut host = host();
    let (_, grid) = (kinogaki_studio_api_testing::Fixtures::default().geometry())
        .into_iter()
        .next()
        .unwrap();
    let faces: Vec<i32> = (0..grid.primitives().polygons().len() as i32)
        .rev()
        .collect();
    let named = (grid.attributes())
        .with(
            Domain::Primitive,
            kinogaki_studio_api::mesher::FACE,
            Attribute::tuples(faces.clone(), Meaning::None).unwrap(),
        )
        .unwrap();
    let joined = grid.with_attributes(named).unwrap();
    let object = ObjectRef(7);
    host.scene.insert(
        object,
        studio_ui_api_testing::FakeObject {
            name: "Body".into(),
            kind: ObjectKindId::declared("toy.trees.body"),
            body: None,
            shown: None,
            polygons: Some(joined.clone()),
        },
    );
    let shown = host.shown_polygons(object).unwrap();
    assert!(shown.positions().ptr_eq(joined.positions()));
    let face = shown
        .attributes()
        .tuples::<i32>(Domain::Primitive, kinogaki_studio_api::mesher::FACE);
    assert_eq!(face.unwrap(), faces.as_slice());
    assert!(host.shown_polygons(ObjectRef(8)).is_none());
}

/// A body set through the host is the object's next body, of its kind only.
#[test]
fn a_body_is_set_whole_and_only_of_its_kind() {
    #[derive(Debug)]
    struct Grove(ObjectKindId);
    impl kinogaki_studio_api::ObjectBody for Grove {
        fn kind(&self) -> ObjectKindId {
            self.0
        }
    }
    let grove = ObjectKindId::declared("toy.trees.grove");
    let mut host = host();
    let object = ObjectRef(7);
    host.scene.insert(
        object,
        studio_ui_api_testing::FakeObject {
            name: "Grove".into(),
            kind: grove,
            body: None,
            shown: None,
            polygons: None,
        },
    );
    let other = ObjectKindId::declared("toy.trees.other");
    assert!(
        host.set_object_body(object, "Refresh", std::sync::Arc::new(Grove(other)))
            .is_err()
    );
    host.set_object_body(object, "Refresh", std::sync::Arc::new(Grove(grove)))
        .unwrap();
    assert!(host.object_body(object).is_some());
    assert_eq!(
        host.take_calls(),
        [
            HostCall::SetBody(object, "Refresh".into()),
            HostCall::SetBody(object, "Refresh".into())
        ]
    );
}

#[test]
fn a_tools_card_shows_its_nodes_rows_and_more_while_some_are_left_out() {
    let mut host = host();
    host.node_rows = NodeRows {
        changed: true,
        hidden: true,
    };
    let mut trees = hosted(&host);
    trees.with(|_, s| s.downcast_mut::<Forest>().unwrap().planting = Some(NodeRef(3)));
    let mut cards = |host: &mut FakeHost, ui: &mut RecordingUi| {
        trees.with(|p, s| {
            let panels = p.contexts[0].panels.as_ref().unwrap();
            let panel = Panel {
                id: studio_ui_api::Id::new("toy.trees.plant"),
                rect: rect(),
                scope: PanelScope::View,
            };
            panels.panel(s, host, &panel, ui);
        });
    };
    let mut ui = RecordingUi::new(rect());
    // Its first frame learns that rows are left out: the click waits for
    // the More shown after.
    ui.click("More");
    cards(&mut host, &mut ui);
    assert!(ui.calls().contains(&Call::Card("Plant".into())));
    cards(&mut host, &mut ui);
    cards(&mut host, &mut ui);
    let rows = |more| HostCall::NodeRows(ObjectRef(7), NodeRef(3), RowsFor::Card { more });
    let asked: Vec<_> = (host.take_calls().into_iter())
        .filter(|c| matches!(c, HostCall::NodeRows(..)))
        .collect();
    assert_eq!(asked, [rows(false), rows(false), rows(true)]);
    assert_eq!(
        host.adjustable,
        Some(NodeRef(3)),
        "an edit keeps it adjustable"
    );
    ui.click("Done");
    cards(&mut host, &mut ui);
    assert!(trees.state::<Forest>().planting.is_none(), "Done ends it");
}
