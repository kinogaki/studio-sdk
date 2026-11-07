//! [`FakeHost`]: the host interface without Studio, recording what a
//! package asks of it, its objects' networks [`FakeNetwork`]s;
//! [`Hosting`]: a registered package with its state,
//! called the way the host calls it (state taken out per call and put
//! back, callbacks queued, never nested).
use super::{FakeNetwork, HostCall};
use kinogaki_studio_api::Diagnostic;
use kinogaki_studio_api::facets::{SnapHit, SnapTargets};
use kinogaki_studio_api::geometry::Geometry;
use kinogaki_studio_api::network::{NetworkEdit, NetworkRead};
use kinogaki_studio_api::params::Unit;
use kinogaki_studio_api::session::SlotDoc;
use kinogaki_studio_api::{
    Bounds, ContextId, Data, Drawn, Error, GestureMark, GestureToken, ObjectBody, ObjectKindId,
    Package, RowsFor, SessionSlot, SessionSlots, SlotId, StoredId, Time,
};
use kinogaki_studio_api::{CancelToken, Components};
use std::any::Any;
use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use studio_ui_api::Pos;
use studio_ui_api::gizmo::{TransformSpace, TransformTool};
use studio_ui_api::host::{
    Cx, HostDoc, HostNetwork, HostPrefs, HostSelection, HostShell, HostSlots, HostStatus,
    HostTools, HostTransform, HostUndo, HostView, Inspected, InspectorCx, LatestAsked, LatestJob,
    LatestResult, NodeRef, NodeRows, ObjectRef, Place, SnapOn, UnitsService, View, ViewCamera,
    ViewCx, Waker,
};
use studio_ui_api::package::hosting::{Registered, State};

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
    /// The polygons its shown result draws as, if any
    /// ([`HostDoc::shown_polygons`]).
    pub polygons: Option<Geometry>,
}

/// What undo restores: the slots and the networks.
#[derive(Clone, Debug)]
struct Kept {
    slots: SessionSlots,
    components: Option<(ObjectRef, Vec<Components>)>,
    networks: BTreeMap<ObjectRef, FakeNetwork>,
}

impl Kept {
    /// Whether `self` and `other` hold the same: equal slots, and the
    /// same networks at the same revisions.
    fn same(&self, other: &Kept) -> bool {
        let revisions = |k: &Kept| -> Vec<(ObjectRef, u64)> {
            k.networks.iter().map(|(&o, n)| (o, n.revision())).collect()
        };
        let components = match (&self.components, &other.components) {
            (Some((a, x)), Some((b, y))) => {
                a == b && x.len() == y.len() && x.iter().zip(y).all(|(x, y)| x.ptr_eq(y))
            }
            (a, b) => a.is_none() && b.is_none(),
        };
        self.slots == other.slots && components && revisions(self) == revisions(other)
    }
}

/// One undo step: its label and what it changed from and to.
#[derive(Clone, Debug)]
struct Step {
    label: String,
    before: Kept,
    after: Kept,
}

/// The host interface without Studio: slots and objects' networks with a
/// real undo history, selections, a place, views and stored values,
/// all plain fields a test sets and reads; every request is recorded
/// ([`FakeHost::calls`]).
pub struct FakeHost {
    /// Every request so far.
    pub calls: Vec<HostCall>,
    /// The session's slots.
    pub slots: SessionSlots,
    /// The objects selected.
    pub objects: Vec<ObjectRef>,
    /// The components selected, and on which object.
    pub components: Option<(ObjectRef, Vec<Components>)>,
    /// The nodes picked, the active one last.
    pub nodes: Vec<NodeRef>,
    /// Where the user is.
    pub place: Place,
    /// The document's revision.
    pub revision: u64,
    /// The scene's current time.
    pub time: Time,
    /// The scene's objects.
    pub scene: BTreeMap<ObjectRef, FakeObject>,
    /// The objects' networks, which [`HostNetwork`] reads and edits (one
    /// undo step an edit).
    pub networks: BTreeMap<ObjectRef, FakeNetwork>,
    /// The views' cameras; the active one first.
    pub cameras: Vec<ViewCamera>,
    /// The view a view call is about.
    pub view: Option<View>,
    /// This frame's input, as view calls see it ([`ViewCx::input`]).
    pub input: studio_ui_api::UserInput,
    /// What the inspector shows.
    pub inspected: Inspected,
    /// The user's preferences.
    pub prefs: HostPrefs,
    /// The host's theme.
    pub theme: studio_ui_api::Theme,
    /// The project's stored values.
    pub stored: kinogaki_studio_api::StoredValues,
    /// The user's stored values.
    pub user_stored: kinogaki_studio_api::StoredValues,
    /// A text field has the keyboard.
    pub keyboard_taken: bool,
    /// The open network displays a held edit.
    pub editing: bool,
    /// The transform tool's space.
    pub space: TransformSpace,
    /// The transform pivot ([`HostTransform::pivot`]).
    pub pivot: Option<kinogaki_studio_api::Vec3>,
    /// The tools' remembered parameters, by node type (a fake host knows
    /// no declared defaults: `starting` is what was remembered).
    pub remembered: BTreeMap<kinogaki_studio_api::NodeTypeId, kinogaki_studio_api::Parameters>,
    /// The node the last tool ran on.
    pub adjustable: Option<NodeRef>,
    /// How many times the wake was called.
    pub wakes: Arc<AtomicUsize>,
    /// The views that render ([`HostView::rendering`]).
    pub rendering: BTreeSet<usize>,
    /// The clipboard's text.
    pub clipboard: Option<String>,
    /// What [`Cx::node_rows`] answers (it draws nothing).
    pub node_rows: NodeRows,
    /// What [`ViewCx::snap`] answers, for any query (a fake host searches
    /// no values).
    pub snap: Option<SnapHit>,
    /// The display options' values in each view, by the view's index and
    /// the option's id ([`HostView::displayed`]; a fake host knows no
    /// declarations, so every other option is 0).
    pub displayed: BTreeMap<(usize, String), usize>,
    /// The latest-wins jobs not yet run, by kind, with their tokens;
    /// [`FakeHost::run_latest`] runs them.
    latest: BTreeMap<&'static str, (CancelToken, LatestJob)>,
    /// Each kind's last ask.
    asked: BTreeMap<&'static str, Box<dyn LatestAsked>>,
    /// The landed results not yet taken, by kind.
    landed: BTreeMap<&'static str, LatestResult>,
    undo: Vec<Step>,
    redo: Vec<Step>,
    /// The open gesture: its number, label and start.
    gesture: Option<(u64, String, Kept)>,
    /// The gestures opened so far.
    gestures: u64,
}

impl FakeHost {
    /// A host at the object levels of context `scene`, holding `slots` (a
    /// package's, [`Package::slots`](kinogaki_studio_api::Package::slots))
    /// at their initial values.
    pub fn new(scene: ContextId, slots: &[SessionSlot]) -> Self {
        FakeHost {
            calls: Vec::new(),
            slots: SessionSlots::new(slots.iter().cloned()),
            objects: Vec::new(),
            components: None,
            nodes: Vec::new(),
            place: Place::Objects {
                context: scene,
                parent: None,
            },
            revision: 1,
            time: Time::ZERO,
            scene: BTreeMap::new(),
            networks: BTreeMap::new(),
            cameras: Vec::new(),
            view: None,
            input: Default::default(),
            inspected: Inspected::default(),
            prefs: HostPrefs::default(),
            theme: studio_ui_api::Theme::default(),
            stored: Default::default(),
            user_stored: Default::default(),
            keyboard_taken: false,
            editing: false,
            space: TransformSpace::World,
            pivot: None,
            remembered: BTreeMap::new(),
            adjustable: None,
            wakes: Arc::default(),
            rendering: BTreeSet::new(),
            clipboard: None,
            node_rows: NodeRows::default(),
            snap: None,
            displayed: BTreeMap::new(),
            latest: BTreeMap::new(),
            asked: BTreeMap::new(),
            landed: BTreeMap::new(),
            undo: Vec::new(),
            redo: Vec::new(),
            gesture: None,
            gestures: 0,
        }
    }

    /// The recorded requests, taken (the next test step starts empty).
    pub fn take_calls(&mut self) -> Vec<HostCall> {
        std::mem::take(&mut self.calls)
    }

    /// Runs every pending latest-wins job, as the host's lanes would:
    /// each result lands (replacing an untaken one of its kind) and wakes
    /// the package once. Returns how many ran.
    pub fn run_latest(&mut self) -> usize {
        let jobs = std::mem::take(&mut self.latest);
        let ran = jobs.len();
        for (key, (token, job)) in jobs {
            let result = job(&token);
            if !token.is_cancelled() {
                self.landed.insert(key, result);
                self.wakes.fetch_add(1, Ordering::SeqCst);
            }
        }
        ran
    }

    /// The labels of the undo history, oldest first.
    pub fn history(&self) -> Vec<&str> {
        self.undo.iter().map(|s| s.label.as_str()).collect()
    }

    /// What undo would restore now.
    fn kept(&self) -> Kept {
        Kept {
            slots: self.slots.clone(),
            components: self.components.clone(),
            networks: self.networks.clone(),
        }
    }

    /// Puts back what `kept` holds.
    fn restore(&mut self, kept: Kept) {
        self.slots = kept.slots;
        self.components = kept.components;
        self.networks = kept.networks;
    }

    /// Records the change from `before` as a step named `label` (inside an
    /// open gesture, nothing yet).
    fn record(&mut self, label: &str, before: Kept) {
        let after = self.kept();
        if self.gesture.is_some() || before.same(&after) {
            return;
        }
        self.undo.push(Step {
            label: label.to_owned(),
            before,
            after,
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
        self.0.context()
    }
}

impl HostUndo for FakeHost {
    fn begin_gesture(&mut self, label: &str) -> Option<GestureToken> {
        self.calls.push(HostCall::BeginGesture(label.into()));
        if self.gesture.is_some() {
            return None;
        }
        self.gestures += 1;
        self.gesture = Some((self.gestures, label.into(), self.kept()));
        Some(GestureToken::new(self.gestures))
    }
    fn finish_gesture(&mut self, gesture: GestureToken) -> Result<(), Error> {
        self.calls.push(HostCall::FinishGesture);
        if let Some((_, label, before)) = self.gesture.take_if(|g| g.0 == gesture.id()) {
            self.record(&label, before);
        }
        Ok(())
    }
    fn cancel_gesture(&mut self, gesture: GestureToken) {
        self.calls.push(HostCall::CancelGesture);
        if let Some((_, _, before)) = self.gesture.take_if(|g| g.0 == gesture.id()) {
            self.restore(before);
        }
    }
    /// A fake host marks only gestures' starts.
    fn back_to(&mut self, mark: GestureMark) {
        if let Some((_, _, before)) = &self.gesture
            && mark == GestureMark::new(self.gestures, 0)
        {
            let before = before.clone();
            self.restore(before);
        }
    }
    fn in_gesture(&self) -> bool {
        self.gesture.is_some()
    }
    fn undo(&mut self) {
        self.calls.push(HostCall::Undo);
        if let Some(step) = self.undo.pop() {
            self.restore(step.before.clone());
            self.redo.push(step);
            self.keep();
        }
    }
    fn redo(&mut self) {
        self.calls.push(HostCall::Redo);
        if let Some(step) = self.redo.pop() {
            self.restore(step.after.clone());
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
    fn edit_slot_any(
        &mut self,
        id: SlotId,
        label: &str,
        edit: &mut dyn FnMut(&mut (dyn Any + Send + Sync)),
    ) -> bool {
        self.calls.push(HostCall::EditSlot(id, label.into()));
        let before = self.kept();
        let Some(value) = self.slots.get_any_mut(id) else {
            return false;
        };
        edit(value);
        self.record(label, before);
        true
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
    fn selected_components(&self, object: ObjectRef) -> Vec<Components> {
        match &self.components {
            Some((on, sets)) if *on == object => sets.clone(),
            _ => Vec::new(),
        }
    }
    fn select_components(&mut self, object: ObjectRef, sets: Vec<Components>) {
        let before = self.kept();
        self.components = (!sets.is_empty()).then_some((object, sets));
        self.record("Select", before);
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
    fn go_to(&mut self, place: Place) -> Result<(), Error> {
        self.calls.push(HostCall::GoTo(place));
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
    fn set_object_body(
        &mut self,
        object: ObjectRef,
        label: &str,
        body: Arc<dyn ObjectBody>,
    ) -> Result<(), Error> {
        self.calls.push(HostCall::SetBody(object, label.to_owned()));
        let found =
            (self.scene.get_mut(&object)).ok_or_else(|| Error::invalid("no such object"))?;
        if body.kind() != found.kind {
            let (object, kind) = (object.0, found.kind.as_str());
            return Err(Error::invalid(format_args!("{object} is not a {kind}")));
        }
        found.body = Some(body);
        Ok(())
    }
    fn objects_of_kind(&self, kind: ObjectKindId) -> Vec<ObjectRef> {
        self.scene
            .iter()
            .filter(|(_, o)| o.kind == kind)
            .map(|(&id, _)| id)
            .collect()
    }
    fn stored_any(&self, id: StoredId) -> Option<&(dyn Any + Send + Sync)> {
        (self.stored.get_any(id)).or_else(|| self.user_stored.get_any(id))
    }
    fn set_stored_any(&mut self, id: StoredId, value: Arc<dyn Any + Send + Sync>) -> bool {
        match self.stored.get_any(id) {
            Some(_) => self.stored.set_any(id, value),
            None => self.user_stored.set_any(id, value),
        }
    }
    fn shown(&self, object: ObjectRef) -> Option<(Data, Drawn)> {
        self.scene.get(&object)?.shown.clone()
    }
    fn shown_polygons(&self, object: ObjectRef) -> Option<Geometry> {
        self.scene.get(&object)?.polygons.clone()
    }
    fn editing(&self) -> bool {
        self.editing
    }
    fn time(&self) -> Time {
        self.time
    }
    fn set_time(&mut self, time: Time) {
        self.calls.push(HostCall::SetTime(time));
        self.time = time;
    }
}

/// The objects' networks are [`FakeHost::networks`]: an edit runs on a
/// copy, which replaces the network only when the edit and the commit
/// check ([`FakeNetwork::check`]) pass, as one undo step.
impl HostNetwork for FakeHost {
    fn network(&self, object: ObjectRef) -> Option<&dyn NetworkRead> {
        self.networks.get(&object).map(|n| n as &dyn NetworkRead)
    }
    fn edit_network(
        &mut self,
        object: ObjectRef,
        label: &str,
        edit: &mut dyn FnMut(&mut dyn NetworkEdit) -> Result<(), Error>,
    ) -> Result<(), Error> {
        self.calls.push(HostCall::EditNetwork(object, label.into()));
        let mut edited =
            self.networks.get(&object).cloned().ok_or_else(|| {
                Error::invalid(format_args!("object {} holds no network", object.0))
            })?;
        edit(&mut edited)?;
        edited.check()?;
        let before = self.kept();
        self.networks.insert(object, edited);
        self.record(label, before);
        Ok(())
    }
}

impl HostView for FakeHost {
    fn shown_views(&self) -> std::ops::Range<usize> {
        0..self.cameras.len().max(1)
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
    fn rendering(&self, view: usize) -> bool {
        self.rendering.contains(&view)
    }
    /// Its `displayed` value of `id` in `view`.
    fn displayed(&self, view: usize, id: &str) -> usize {
        (self.displayed.get(&(view, id.to_owned()))).map_or(0, |&v| v)
    }
}

impl HostStatus for FakeHost {
    fn report(&mut self, diagnostic: Diagnostic, subject: Option<&str>) {
        (self.calls).push(HostCall::Report(diagnostic, subject.map(str::to_owned)));
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
    fn copy_text(&mut self, text: &str) {
        self.calls.push(HostCall::CopyText(text.into()));
        self.clipboard = Some(text.into());
    }
    fn paste_text(&self) -> Option<String> {
        self.clipboard.clone()
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
    fn transform_space(&self) -> TransformSpace {
        self.space
    }
    fn set_transform_space(&mut self, space: TransformSpace) {
        self.calls.push(HostCall::SetTransformSpace(space));
        self.space = space;
    }
    fn pivot(&self) -> Option<kinogaki_studio_api::Vec3> {
        self.pivot
    }
}

impl HostTools for FakeHost {
    fn starting(&self, ty: kinogaki_studio_api::NodeTypeId) -> kinogaki_studio_api::Parameters {
        self.remembered.get(&ty).cloned().unwrap_or_default()
    }
    fn remember(
        &mut self,
        ty: kinogaki_studio_api::NodeTypeId,
        parameters: &kinogaki_studio_api::Parameters,
    ) {
        self.remembered.insert(ty, parameters.clone());
    }
    fn adjustable(&self) -> Option<NodeRef> {
        self.adjustable
    }
    fn ran(&mut self, node: NodeRef) {
        self.adjustable = Some(node);
    }
}

/// Lengths in millimetres with two places; angles in degrees.
impl UnitsService for FakeHost {
    fn parse(&self, text: &str, current: f64, _unit: &Unit) -> Option<f64> {
        let text = text.trim();
        match text.strip_prefix("+=") {
            Some(by) => by.trim().parse::<f64>().ok().map(|by| current + by),
            None => text.trim_end_matches("mm").trim().parse().ok(),
        }
    }
    fn format(&self, stored: f64, unit: &Unit, decimals: usize) -> String {
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
    /// No color management: every space the working one.
    fn colors(&self) -> &dyn kinogaki_studio_api::color::ColorService {
        &kinogaki_studio_api_testing::Unconverted
    }
    fn theme(&self) -> &studio_ui_api::Theme {
        &self.theme
    }
    /// Queues `job` until [`FakeHost::run_latest`] unless `asked` is the
    /// kind's last ask (cancelling a queued job of the kind, which then
    /// never runs, and dropping a landed result of an older ask).
    fn latest_any(&mut self, key: &'static str, asked: Box<dyn LatestAsked>, job: LatestJob) {
        if self.asked.get(key).is_some_and(|last| last.same(&*asked)) {
            return;
        }
        self.calls.push(HostCall::Latest(key.into()));
        self.asked.insert(key, asked);
        self.landed.remove(key);
        let token = CancelToken::default();
        if let Some((previous, _)) = self.latest.insert(key, (token, job)) {
            previous.cancel();
        }
    }
    fn take_latest_any(
        &mut self,
        key: &'static str,
    ) -> Option<(Box<dyn LatestAsked>, LatestResult)> {
        let result = self.landed.remove(key)?;
        Some((self.asked.get(key)?.copy(), result))
    }
    /// Records the call and answers its `node_rows` field.
    fn node_rows(
        &mut self,
        _ui: &mut dyn studio_ui_api::Ui,
        object: ObjectRef,
        node: NodeRef,
        purpose: RowsFor,
    ) -> NodeRows {
        self.calls.push(HostCall::NodeRows(object, node, purpose));
        self.node_rows
    }
}

impl ViewCx for FakeHost {
    /// Nothing: a fake shows nothing to pick.
    fn element_at(
        &mut self,
        _: Pos,
        _: &[kinogaki_studio_api::viewport::ElementKind],
    ) -> Option<kinogaki_studio_api::Components> {
        None
    }
    fn view(&self) -> &View {
        self.view
            .as_ref()
            .expect("a view call on a FakeHost with its `view` set")
    }
    /// Its `input` field.
    fn input(&self) -> &studio_ui_api::UserInput {
        &self.input
    }
    /// Its `snap` field.
    fn snap(
        &mut self,
        _at: Pos,
        _targets: SnapTargets,
        _radius: f64,
        _on: SnapOn,
    ) -> Option<SnapHit> {
        self.snap
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
    /// When `package` has no UI ([`WithUi`](studio_ui_api::package::WithUi)).
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

    /// [`UiPackage::poll`](studio_ui_api::package::UiPackage::poll) on `host`.
    pub fn poll(&mut self, host: &mut FakeHost) {
        self.with(|p, s| p.poll(s, host))
    }
}
