//! [`FakeHost`]: the host interface without Studio, recording what a
//! package asks of it; [`Hosting`]: a registered package with its state,
//! called the way the host calls it (state taken out per call and put
//! back, callbacks queued, never nested).
use crate::gizmo::{GizmoFrame, Handle, TransformTool};
use crate::host::{
    Cx, GizmoService, HostDoc, HostNetwork, HostPrefs, HostSelection, HostSettings, HostShell,
    HostSlots, HostStatus, HostTransform, HostUndo, HostView, Inspected, InspectorCx, NodeRef,
    ObjectRef, Place, UnitsService, View, ViewCamera, ViewCx, Waker,
};
use crate::package::{Registered, State};
use crate::{Pos, Projection, Rect};
use kinogaki_studio_api::network::{NetworkEdit, NetworkRead};
use kinogaki_studio_api::params::Unit;
use kinogaki_studio_api::session::SlotDoc;
use kinogaki_studio_api::{
    Bounds, ContextId, Data, Drawn, Error, ObjectBody, ObjectKindId, Package, SessionSlot,
    SessionSlots, SlotId,
};
use std::any::Any;
use std::collections::BTreeMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

/// What a package asked of a [`FakeHost`], in order (queries are not
/// recorded).
#[derive(Clone, Debug, PartialEq)]
pub enum HostCall {
    /// [`HostUndo::begin_gesture`].
    BeginGesture(String),
    /// [`HostUndo::finish_gesture`].
    FinishGesture,
    /// [`HostUndo::cancel_gesture`].
    CancelGesture,
    /// [`HostUndo::undo`].
    Undo,
    /// [`HostUndo::redo`].
    Redo,
    /// [`HostSlots::set_slot_any`], by slot and step label.
    SetSlot(SlotId, String),
    /// [`HostSelection::select_objects`].
    SelectObjects(Vec<ObjectRef>),
    /// [`HostSelection::pick_node`].
    PickNode(NodeRef, bool),
    /// [`HostDoc::go_to`].
    GoTo(Place),
    /// [`HostView::frame_bounds`].
    Frame(Bounds),
    /// [`HostView::redraw`].
    Redraw,
    /// [`HostStatus::fail`].
    Fail(String),
    /// [`HostStatus::info`].
    Info(String),
    /// [`HostShell::run_action`].
    RunAction(String, String),
    /// [`HostTransform::set_transform_tool`].
    SetTransformTool(TransformTool),
    /// [`HostTransform::set_snapping`].
    SetSnapping(bool),
    /// [`HostSettings::set_section_any`].
    SetSection(ContextId),
}

/// An object of a [`FakeHost`]'s scene.
#[derive(Clone, Debug)]
pub struct FakeObject {
    /// Its name.
    pub name: String,
    /// Its kind.
    pub kind: ObjectKindId,
    /// Its package-owned body, if any.
    pub body: Option<Arc<dyn ObjectBody>>,
    /// Its shown result, if any.
    pub shown: Option<(Data, Drawn)>,
}

/// One undo step: its label and the slots before and after it.
#[derive(Clone, Debug)]
struct Step {
    label: String,
    before: SessionSlots,
    after: SessionSlots,
}

/// The host interface without Studio: slots with a real undo history,
/// selections, a place, views and settings sections, all plain fields a
/// test sets and reads; every request is recorded ([`FakeHost::calls`]).
pub struct FakeHost {
    /// Every request so far.
    pub calls: Vec<HostCall>,
    /// The session's slots.
    pub slots: SessionSlots,
    /// The objects selected.
    pub objects: Vec<ObjectRef>,
    /// The nodes picked, the active one last.
    pub nodes: Vec<NodeRef>,
    /// Where the user is.
    pub place: Place,
    /// The document's revision.
    pub revision: u64,
    /// The scene's objects.
    pub scene: BTreeMap<ObjectRef, FakeObject>,
    /// The views' cameras; the active one first.
    pub cameras: Vec<ViewCamera>,
    /// The view a view call is about.
    pub view: Option<View>,
    /// What the inspector shows.
    pub inspected: Inspected,
    /// The user's preferences.
    pub prefs: HostPrefs,
    /// Settings sections' applied values, by context.
    pub sections: BTreeMap<ContextId, Arc<dyn Any + Send + Sync>>,
    /// The project's stored values.
    pub stored: kinogaki_studio_api::StoredValues,
    /// Each view's stored values.
    pub view_stored: Vec<kinogaki_studio_api::StoredValues>,
    /// A text field has the keyboard.
    pub keyboard_taken: bool,
    /// The open network displays a held edit.
    pub editing: bool,
    /// How many times the wake was called.
    pub wakes: Arc<AtomicUsize>,
    undo: Vec<Step>,
    redo: Vec<Step>,
    gesture: Option<(String, SessionSlots)>,
}

impl FakeHost {
    /// A host at the object levels of context `scene`, holding `slots` at
    /// their initial values.
    pub fn new(scene: ContextId, slots: impl IntoIterator<Item = SessionSlot>) -> Self {
        FakeHost {
            calls: Vec::new(),
            slots: SessionSlots::new(slots),
            objects: Vec::new(),
            nodes: Vec::new(),
            place: Place {
                context: scene,
                object: None,
                objects: true,
                subnet: None,
                node: None,
            },
            revision: 1,
            scene: BTreeMap::new(),
            cameras: Vec::new(),
            view: None,
            inspected: Inspected::default(),
            prefs: HostPrefs::default(),
            sections: BTreeMap::new(),
            stored: Default::default(),
            view_stored: Vec::new(),
            keyboard_taken: false,
            editing: false,
            wakes: Arc::default(),
            undo: Vec::new(),
            redo: Vec::new(),
            gesture: None,
        }
    }

    /// The recorded requests, taken (the next test step starts empty).
    pub fn take_calls(&mut self) -> Vec<HostCall> {
        std::mem::take(&mut self.calls)
    }

    /// The labels of the undo history, oldest first.
    pub fn history(&self) -> Vec<&str> {
        self.undo.iter().map(|s| s.label.as_str()).collect()
    }

    /// Records the change from `before` as a step named `label` (inside an
    /// open gesture, nothing yet).
    fn record(&mut self, label: &str, before: SessionSlots) {
        if self.gesture.is_some() || before == self.slots {
            return;
        }
        self.undo.push(Step {
            label: label.to_owned(),
            before,
            after: self.slots.clone(),
        });
        self.redo.clear();
    }

    /// Brings the slots in line with the place, as a host does after an
    /// undo.
    fn keep(&mut self) {
        let doc = Here(self.place);
        self.slots.keep(&doc);
    }
}

/// The place as what a slot's restore hook reads.
struct Here(Place);

impl SlotDoc for Here {
    fn context(&self) -> ContextId {
        self.0.context
    }
    fn in_mode(&self) -> bool {
        self.0.node.is_some() && !self.0.objects
    }
}

impl HostUndo for FakeHost {
    fn begin_gesture(&mut self, label: &str) {
        self.calls.push(HostCall::BeginGesture(label.into()));
        if self.gesture.is_none() {
            self.gesture = Some((label.into(), self.slots.clone()));
        }
    }
    fn finish_gesture(&mut self) {
        self.calls.push(HostCall::FinishGesture);
        if let Some((label, before)) = self.gesture.take() {
            self.record(&label, before);
        }
    }
    fn cancel_gesture(&mut self) {
        self.calls.push(HostCall::CancelGesture);
        if let Some((_, before)) = self.gesture.take() {
            self.slots = before;
        }
    }
    fn in_gesture(&self) -> bool {
        self.gesture.is_some()
    }
    fn undo(&mut self) {
        self.calls.push(HostCall::Undo);
        if let Some(step) = self.undo.pop() {
            self.slots = step.before.clone();
            self.redo.push(step);
            self.keep();
        }
    }
    fn redo(&mut self) {
        self.calls.push(HostCall::Redo);
        if let Some(step) = self.redo.pop() {
            self.slots = step.after.clone();
            self.undo.push(step);
            self.keep();
        }
    }
    fn undo_label(&self) -> Option<String> {
        self.undo.last().map(|s| s.label.clone())
    }
    fn redo_label(&self) -> Option<String> {
        self.redo.last().map(|s| s.label.clone())
    }
}

impl HostSlots for FakeHost {
    fn slot_any(&self, id: SlotId) -> Option<&(dyn Any + Send + Sync)> {
        self.slots.get_any(id)
    }
    fn slot_any_mut(&mut self, id: SlotId) -> Option<&mut (dyn Any + Send + Sync)> {
        self.slots.get_any_mut(id)
    }
    fn set_slot_any(&mut self, id: SlotId, label: &str, value: Arc<dyn Any + Send + Sync>) -> bool {
        self.calls.push(HostCall::SetSlot(id, label.into()));
        let before = self.slots.clone();
        let set = self.slots.set_any(id, value);
        if set {
            self.record(label, before);
        }
        set
    }
}

impl HostSelection for FakeHost {
    fn selected_objects(&self) -> Vec<ObjectRef> {
        self.objects.clone()
    }
    fn select_objects(&mut self, objects: &[ObjectRef]) {
        self.calls.push(HostCall::SelectObjects(objects.to_vec()));
        self.objects = objects.to_vec();
    }
    fn picked_nodes(&self) -> Vec<NodeRef> {
        self.nodes.clone()
    }
    fn active_node(&self) -> Option<NodeRef> {
        self.nodes.last().copied()
    }
    fn pick_node(&mut self, node: NodeRef, add: bool) {
        self.calls.push(HostCall::PickNode(node, add));
        if !add {
            self.nodes.clear();
        }
        self.nodes.retain(|&n| n != node);
        self.nodes.push(node);
    }
}

impl HostDoc for FakeHost {
    fn place(&self) -> Place {
        self.place
    }
    fn go_to(&mut self, place: Place) -> Result<(), String> {
        self.calls.push(HostCall::GoTo(place));
        if place.context != self.place.context || place.object != self.place.object {
            self.slots.reset_place_bound();
        }
        self.place = place;
        Ok(())
    }
    fn revision(&self) -> u64 {
        self.revision
    }
    fn object_name(&self, object: ObjectRef) -> Option<String> {
        self.scene.get(&object).map(|o| o.name.clone())
    }
    fn object_kind(&self, object: ObjectRef) -> Option<ObjectKindId> {
        self.scene.get(&object).map(|o| o.kind)
    }
    fn object_body(&self, object: ObjectRef) -> Option<Arc<dyn ObjectBody>> {
        self.scene.get(&object)?.body.clone()
    }
    fn objects_of_kind(&self, kind: ObjectKindId) -> Vec<ObjectRef> {
        self.scene
            .iter()
            .filter(|(_, o)| o.kind == kind)
            .map(|(&id, _)| id)
            .collect()
    }
    fn stored_any(&self, id: &str, view: Option<usize>) -> Option<&(dyn Any + Send + Sync)> {
        match view {
            None => self.stored.get_any(id),
            Some(v) => self.view_stored.get(v)?.get_any(id),
        }
    }
    fn set_stored_any(
        &mut self,
        id: &str,
        view: Option<usize>,
        value: Arc<dyn Any + Send + Sync>,
    ) -> bool {
        match view {
            None => self.stored.set_any(id, value),
            Some(v) => self
                .view_stored
                .get_mut(v)
                .is_some_and(|s| s.set_any(id, value)),
        }
    }
    fn shown(&self, object: ObjectRef) -> Option<(Data, Drawn)> {
        self.scene.get(&object)?.shown.clone()
    }
    fn editing(&self) -> bool {
        self.editing
    }
}

/// The fake holds no networks: core owns them, so a package's network
/// edits are tested against Studio's own host.
impl HostNetwork for FakeHost {
    fn network(&self, _object: ObjectRef) -> Option<&dyn NetworkRead> {
        None
    }
    fn edit_network(
        &mut self,
        object: ObjectRef,
        _label: &str,
        _edit: &mut dyn FnMut(&mut dyn NetworkEdit) -> Result<(), Error>,
    ) -> Result<(), Error> {
        Err(Error::invalid(format_args!(
            "object {} holds no network in the fake host",
            object.0
        )))
    }
}

impl HostView for FakeHost {
    fn view_count(&self) -> usize {
        self.cameras.len().max(1)
    }
    fn active_view(&self) -> usize {
        0
    }
    fn camera(&self, index: usize) -> Option<ViewCamera> {
        self.cameras.get(index).copied()
    }
    fn frame_bounds(&mut self, bounds: Bounds) {
        self.calls.push(HostCall::Frame(bounds));
    }
    fn redraw(&mut self) {
        self.calls.push(HostCall::Redraw);
    }
}

impl HostStatus for FakeHost {
    fn fail(&mut self, message: &str) {
        self.calls.push(HostCall::Fail(message.into()));
    }
    fn info(&mut self, message: &str) {
        self.calls.push(HostCall::Info(message.into()));
    }
}

impl HostShell for FakeHost {
    fn run_action(&mut self, vocabulary: &str, item: &str) {
        self.calls
            .push(HostCall::RunAction(vocabulary.into(), item.into()));
    }
    fn keyboard_taken(&self) -> bool {
        self.keyboard_taken
    }
}

impl HostTransform for FakeHost {
    fn transform_tool(&self) -> TransformTool {
        self.prefs.transform
    }
    fn set_transform_tool(&mut self, tool: TransformTool) {
        self.calls.push(HostCall::SetTransformTool(tool));
        self.prefs.transform = tool;
    }
    fn snapping(&self) -> bool {
        self.prefs.snapping
    }
    fn set_snapping(&mut self, on: bool) {
        self.calls.push(HostCall::SetSnapping(on));
        self.prefs.snapping = on;
    }
}

impl HostSettings for FakeHost {
    fn section_any(&self, context: ContextId) -> Option<&(dyn Any + Send + Sync)> {
        self.sections.get(&context).map(|v| &**v)
    }
    fn set_section_any(&mut self, context: ContextId, value: Arc<dyn Any + Send + Sync>) -> bool {
        self.calls.push(HostCall::SetSection(context));
        match self.sections.get_mut(&context) {
            Some(kept) if (**kept).type_id() == (*value).type_id() => {
                *kept = value;
                true
            }
            _ => false,
        }
    }
}

/// Lengths in millimetres with two places; angles in degrees.
impl UnitsService for FakeHost {
    fn parse(&self, text: &str, current: f64, _unit: Unit) -> Option<f64> {
        let text = text.trim();
        match text.strip_prefix("+=") {
            Some(by) => by.trim().parse::<f64>().ok().map(|by| current + by),
            None => text.trim_end_matches("mm").trim().parse().ok(),
        }
    }
    fn format(&self, stored: f64, unit: Unit, decimals: usize) -> String {
        match unit {
            Unit::Length | Unit::Millimeters => format!("{stored:.decimals$} mm"),
            Unit::Angle => format!("{stored:.decimals$}°"),
            Unit::Percent => format!("{stored:.decimals$}%"),
            Unit::Plain => format!("{stored:.decimals$}"),
            Unit::Suffix(suffix) => format!("{stored:.decimals$}{suffix}"),
        }
    }
}

impl Cx for FakeHost {
    fn prefs(&self) -> HostPrefs {
        self.prefs
    }
    fn wake(&self) -> Waker {
        let wakes = self.wakes.clone();
        Arc::new(move || {
            wakes.fetch_add(1, Ordering::SeqCst);
        })
    }
    fn units(&self) -> &dyn UnitsService {
        self
    }
}

/// The gizmo draws nothing and nothing is hit; its projection is the
/// view's.
impl GizmoService for FakeHost {
    fn projection(&self, _at: Rect) -> Projection {
        self.view.map_or(
            Projection {
                clip_from_object: Default::default(),
                object_from_clip: Default::default(),
                backward: kinogaki_studio_api::Vec3::new(0.0, 0.0, 1.0),
            },
            |v| v.camera.projection,
        )
    }
    fn paint(
        &self,
        _frame: &GizmoFrame,
        _tool: TransformTool,
        _hot: Option<Handle>,
        _axes: [bool; 3],
    ) {
    }
    fn hit(
        &self,
        _frame: &GizmoFrame,
        _tool: TransformTool,
        _p: Pos,
        _axes: [bool; 3],
    ) -> Option<Handle> {
        None
    }
}

impl ViewCx for FakeHost {
    fn view(&self) -> &View {
        self.view
            .as_ref()
            .expect("a view call on a FakeHost with its `view` set")
    }
    fn gizmo(&self) -> &dyn GizmoService {
        self
    }
}

impl InspectorCx for FakeHost {
    fn inspected(&self) -> Inspected {
        self.inspected
    }
}

/// A package's UI with its state on a [`FakeHost`], called as the host
/// calls it: the state is taken out for the call and put back after,
/// and a call while it is out (a nested call, which a host never makes)
/// panics.
pub struct Hosting {
    /// The package.
    pub package: Package,
    state: Option<State>,
}

impl Hosting {
    /// `package` with a new state of its UI, its wake `host`'s.
    ///
    /// # Panics
    /// When `package` has no UI ([`WithUi`](crate::package::WithUi)).
    pub fn new(package: Package, host: &FakeHost) -> Self {
        let state = Self::ui(&package).state(host.wake());
        Hosting {
            package,
            state: Some(state),
        }
    }

    /// Runs `call` with the package and its state taken out.
    ///
    /// # Panics
    /// When the state is out already: a call nested in another.
    pub fn with<R>(&mut self, call: impl FnOnce(&Registered, &mut dyn Any) -> R) -> R {
        let mut state = self
            .state
            .take()
            .expect("a package call nested in another: hosts queue callbacks instead");
        let out = call(Self::ui(&self.package), &mut *state);
        self.state = Some(state);
        out
    }

    /// `package`'s UI.
    fn ui(package: &Package) -> &Registered {
        Registered::of(package).expect("a package with a UI")
    }

    /// The UI, as the host reads it.
    pub fn registered(&self) -> &Registered {
        Self::ui(&self.package)
    }

    /// The state, as its package's type (to check it in a test).
    pub fn state<S: Any>(&self) -> &S {
        self.state
            .as_ref()
            .and_then(|s| s.downcast_ref())
            .expect("the package's state, of its type")
    }

    /// [`UiPackage::poll`](crate::package::UiPackage::poll) on `host`.
    pub fn poll(&mut self, host: &mut FakeHost) {
        self.with(|p, s| p.poll(s, host))
    }
}
