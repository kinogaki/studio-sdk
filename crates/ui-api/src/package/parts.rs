//! A context's parts, one trait each, over the package's state `S`: the
//! selection levels, the viewport layer, the inspector, the vocabulary,
//! the network view, and the object kinds' inspector and outliner rows.
//!
//! Each method gets the package's state and the call's context (`cx`);
//! a query gets them shared (`&S`, `&dyn Cx`) and cannot change anything.
//! Parts are stateless values: their state is `S`, which the host lends
//! for one call at a time ([`WithUi`](super::WithUi)). Every method
//! runs on the UI thread and returns at once.
//!
//! The host erases `S` when a package registers: [`Erased`] turns each
//! part over `S` into the same part over `dyn Any`, downcasting the state
//! the host lends back to `S`.
use super::data::{Candidate, LevelPreset, LevelSpec, NumericBar, OutlineRow, TimeDomain, Tools};
use super::edited::Edited;
use super::view::{Holds, KeyPress, Popover, PopoverRow, RowEdit};
use crate::gizmo::{PivotFrame, TransformPhase, TransformStep};
use crate::host::{Cx, InspectorCx, NodeRef, ObjectRef, ViewCx};
use crate::sheet::{Edits, Schema};
use crate::{Pos, Rect, Ui};
use kinogaki_studio_api::geometry::Polygons;
use kinogaki_studio_api::{Combine, Components, ObjectKindId, Vec3};
use std::any::Any;
use std::marker::PhantomData;

/// A context's selection levels (Mesh's Piece, Face, Edge, Point; CAD's
/// Body, Face, Edge, CV…). The selection itself is the package's own
/// [slot](kinogaki_studio_api::Slot).
pub trait Levels<S: ?Sized>: Send + Sync {
    /// Its levels, in order (number keys 1, 2, … choose them). Read once,
    /// when the package registers.
    fn levels(&self) -> Vec<LevelSpec>;
    /// The active levels, by index.
    fn active(&self, s: &S, cx: &dyn Cx) -> Vec<usize>;
    /// The level selections are kept at and the marking menus show.
    fn primary(&self, s: &S, cx: &dyn Cx) -> usize;
    /// Makes `level` the only active level, converting the selection to it.
    fn choose(&self, s: &mut S, cx: &mut dyn Cx, level: usize);
    /// Adds `level` to the active levels, or removes it (never the last).
    fn toggle(&self, s: &mut S, cx: &mut dyn Cx, level: usize);
    /// The level of what is selected, by index (an edge selected while
    /// faces are the active level: the edge level); `None`: nothing is.
    /// The library's entries are held against it, else against the
    /// primary level ([`When`](super::When)).
    fn selected(&self, s: &S, cx: &dyn Cx) -> Option<usize> {
        let _ = (s, cx);
        None
    }
    /// The open object and its selected components, as components of the
    /// value it shows ([`HostDoc::shown`](crate::host::HostDoc::shown)):
    /// what a Material dropped on that object takes, and the selection the
    /// spreadsheet shows (an empty set while nothing is). `None`: the
    /// context keeps no component selection there.
    fn components(&self, s: &S, cx: &dyn Cx) -> Option<(ObjectRef, Components)> {
        let _ = (s, cx);
        None
    }
    /// What the pointer is over at the active level, as the host picked it
    /// through the open value's `Pickable` (`None`: nothing there): show it
    /// hovered.
    fn hovered(&self, s: &mut S, cx: &mut dyn Cx, hit: Option<Components>) {
        let _ = (s, cx, hit);
    }
    /// A click, box or lasso at the active level, as the host picked it
    /// through the open value's `Pickable` with its pick policy (`None`:
    /// nothing there), combined with the selection as `how` says (Shift
    /// adds, Command removes): the context decides what the hit selects.
    fn picked(&self, s: &mut S, cx: &mut dyn Cx, hit: Option<Components>, how: Combine) {
        let _ = (s, cx, hit, how);
    }
    /// The selection as the transform pivot reads it, in the context's
    /// space; `None`: nothing to pivot about (no pivot button). Called
    /// once a frame while the numeric bar shows.
    fn pivot(&self, s: &mut S, cx: &mut dyn Cx) -> Option<PivotFrame> {
        let _ = (s, cx);
        None
    }
    /// The user chose another pivot: a live tool takes it.
    fn pivot_changed(&self, s: &mut S, cx: &mut dyn Cx) {
        let _ = (s, cx);
    }
    /// The selection's normal, in the context's space: the Normal
    /// transform space's Y. `None`: Normal works as Object.
    fn normal(&self, s: &mut S, cx: &mut dyn Cx) -> Option<Vec3> {
        let _ = (s, cx);
        None
    }
    /// Its preset after its levels, if it has one (read once, as
    /// [`levels`](Self::levels)).
    fn preset(&self) -> Option<LevelPreset> {
        None
    }
    /// Whether its preset is on.
    fn preset_on(&self, s: &S, cx: &dyn Cx) -> bool {
        let _ = (s, cx);
        false
    }
    /// Turns its preset on or off (the button, or its number key).
    fn toggle_preset(&self, s: &mut S, cx: &mut dyn Cx) {
        let _ = (s, cx);
    }
    /// What lies under `pointer` in view `rect` at the active levels,
    /// nearest first: the stacked list's rows. Empty: the list holds
    /// objects only.
    fn candidates(&self, s: &S, cx: &dyn Cx, rect: Rect, pointer: Pos) -> Vec<Candidate> {
        let _ = (s, cx, rect, pointer);
        Vec::new()
    }
    /// The user picked `candidate` in the stacked list: select it as a
    /// click on it would (Shift adds, Command removes).
    fn pick_candidate(
        &self,
        s: &mut S,
        cx: &mut dyn Cx,
        candidate: &Candidate,
        shift: bool,
        command: bool,
    ) {
        let _ = (s, cx, candidate, shift, command);
    }
    /// Shows `candidate` hovered (the stacked list's row under the
    /// pointer); `None` clears it.
    fn hover_candidate(&self, s: &mut S, cx: &mut dyn Cx, candidate: Option<&Candidate>) {
        let _ = (s, cx, candidate);
    }
}

/// What a context adds to the viewport: its live tools (their input in a
/// view, overlays, cards and numeric bar), what it draws over the open
/// object, its keys, its popovers' rows and its time. The rest of the
/// viewport is core's: the left toolbar, the status bar, framing, the
/// gizmo, pivot picks and component picks (through [`Levels`]).
///
/// A view's frame calls, in order: [`controls`](Self::controls) and
/// [`holds`](Self::holds) (what the pointer may reach), [`tools`](Self::tools)
/// before the camera's navigation, then core's gizmo, handles and picks,
/// then [`overlay`](Self::overlay); after every view,
/// [`cards`](Self::cards), and [`edited`](Self::edited) for each view drawn.
pub trait ViewportLayer<S: ?Sized>: Send + Sync {
    /// Escape, an undo, or the location leaving the context: end live
    /// drags and gestures.
    fn cancel(&self, s: &mut S, cx: &mut dyn Cx) {
        let _ = (s, cx);
    }
    /// A key or typed text over the viewport, before any binding (see
    /// [`KeyPress`]): a live tool's own keys (Enter, Escape), a focused
    /// value field's typing. `true`: the layer took it, and nothing else
    /// sees it.
    fn key(&self, s: &mut S, cx: &mut dyn Cx, press: &KeyPress) -> bool {
        let _ = (s, cx, press);
        false
    }
    /// What a live gesture keeps from the rest of the viewport now (asked
    /// once a frame, before the views' input).
    fn holds(&self, s: &S, cx: &dyn Cx) -> Holds {
        let _ = (s, cx);
        Holds::default()
    }
    /// The context's rows of `popover`, called while it is open (and,
    /// for the Gear, once a frame to know whether it has any).
    fn rows(&self, s: &mut S, cx: &mut dyn Cx, popover: Popover) -> Vec<PopoverRow> {
        let _ = (s, cx, popover);
        Vec::new()
    }
    /// An edit of one of [`rows`](Self::rows)'s rows.
    fn apply_row(&self, s: &mut S, cx: &mut dyn Cx, popover: Popover, edit: RowEdit) {
        let _ = (s, cx, popover, edit);
    }
    /// The numeric bar of a live tool: while `Some`, it stands in the
    /// transform tool's place under the active view.
    fn numeric_bar(&self, s: &mut S, cx: &mut dyn Cx) -> Option<NumericBar> {
        let _ = (s, cx);
        None
    }
    /// A value typed or chosen in field `field` of
    /// [`numeric_bar`](Self::numeric_bar) (`current`: what the field held).
    fn apply_numeric(&self, s: &mut S, cx: &mut dyn Cx, field: usize, current: f64, value: f64) {
        let _ = (s, cx, field, current, value);
    }
    /// What the context adds over the open object in view `view` this
    /// frame: overlays, guides, lines and style (`None`: nothing). Core
    /// draws the object's value itself; called once per drawn view after
    /// the views' input, so it is also where a context refreshes what its
    /// drawing needs.
    fn edited(&self, s: &mut S, cx: &mut dyn Cx, view: usize) -> Option<Edited> {
        let _ = (s, cx, view);
        None
    }
    /// A live tool's input in a view, before camera navigation: its drags,
    /// clicks and wheel (Alt's drags are the camera's). What it says it
    /// takes ([`Tools`]) keeps core's gizmo, handles, picks, wheel and
    /// right-click menu off this frame. A drag edits in the tool's gesture,
    /// one undo step.
    fn tools(&self, s: &mut S, cx: &mut dyn ViewCx, ui: &mut dyn Ui) -> Tools {
        let _ = (s, cx, ui);
        Tools::default()
    }
    /// The open object's own snap targets beyond its shown value (a
    /// curve's control points, a sketch's points and lines), in its space:
    /// points snap as points, polylines as edges, their middles and where
    /// they cross. [`ViewCx::snap`] searches them with the shown value, in
    /// one order and with one feedback. Asked once a frame, before the
    /// views' [`tools`](Self::tools); the same value (its positions and
    /// topology shared) while nothing changed costs nothing. `None`: none.
    fn own_targets(&self, s: &S, cx: &dyn Cx) -> Option<Polygons> {
        let _ = (s, cx);
        None
    }
    /// Draws over a view after its navigation and pointer handling, so
    /// what it places follows this frame's camera: labels, handles,
    /// heads-up values.
    fn overlay(&self, s: &mut S, cx: &mut dyn ViewCx, ui: &mut dyn Ui) {
        let _ = (s, cx, ui);
    }
    /// The node a live tool works on (its card is up). While there is one,
    /// the open network's templated branches are pickable in the views: a
    /// click wires the branch under the pointer into the node's operand
    /// input (Command takes it off), in the tool's gesture. Core marks the
    /// branch under the pointer and those wired into the node. `None`: no
    /// tool is live.
    fn live_node(&self, s: &S, cx: &dyn Cx) -> Option<NodeRef> {
        let _ = (s, cx);
        None
    }
    /// Whether the context's own transform gizmo takes precedence over the
    /// selected node's declared transform handles now (the user picked
    /// components to move). `false`: the node's handles show in the
    /// transform tool's place, as Maya's manipulators do.
    fn own_gizmo(&self, s: &S, cx: &dyn Cx) -> bool {
        let _ = (s, cx);
        false
    }
    /// Whether the host's transform gizmo shows for the selection now,
    /// at [`Levels::pivot`] and along the user's transform space. The
    /// host runs its drags and hands each step to
    /// [`transform`](Self::transform). `false`: no gizmo (nothing
    /// selected, or a live tool has the pointer).
    fn transforms(&self, s: &S, cx: &dyn Cx) -> bool {
        let _ = (s, cx);
        false
    }
    /// A transform drag of the selection, in the host's undo gesture:
    /// show `step` (a preview, not yet an edit: points moved through
    /// [`Edited::moving`]) while it `Began` or `Moved`, make it the edit
    /// on `Released`, and put back what the drag showed on `Cancelled`
    /// (the host cancels its gesture).
    fn transform(&self, s: &mut S, cx: &mut dyn Cx, phase: TransformPhase, step: &TransformStep) {
        let _ = (s, cx, phase, step);
    }
    /// Floating cards drawn once a frame over the views in `rect`, after
    /// every view's input: a live tool's card ([`Ui::card`](crate::Ui::card)),
    /// a panel. Where they are is [`controls`](Self::controls).
    fn cards(&self, s: &mut S, cx: &mut dyn Cx, ui: &mut dyn Ui, rect: Rect) {
        let _ = (s, cx, ui, rect);
    }
    /// Where the context's floating controls are over the view at `view`
    /// this frame (its cards, its panels in the view): the pointer over
    /// one is theirs, not the view's tools. A popup taking the pointer over
    /// every view gives `view` itself. Said from the state the controls
    /// draw from, never recorded while drawing, so it holds for the frame
    /// that draws them.
    fn controls(&self, s: &S, cx: &dyn Cx, view: Rect) -> Vec<Rect> {
        let _ = (s, cx, view);
        Vec::new()
    }
    /// The context's own time, which the Timeline shows instead of the
    /// scene's; `None`: the scene's.
    fn time(&self, s: &mut S, cx: &mut dyn Cx) -> Option<TimeDomain> {
        let _ = (s, cx);
        None
    }
    /// Shows the context at time `t` (a scrub, a step or a played frame).
    fn set_time(&self, s: &mut S, cx: &mut dyn Cx, t: f64) {
        let _ = (s, cx, t);
    }
}

/// The inspector panel while the context is open, drawn under the node
/// sheet of the inspected node (where there is one).
pub trait Inspector<S: ?Sized>: Send + Sync {
    /// Draws the panel (its sheets through [`Ui::sheet`]).
    fn show(&self, s: &mut S, cx: &mut dyn InspectorCx, ui: &mut dyn Ui);
    /// Selects what a Console record's subject names inside the open
    /// object (its own spelling, as its reports wrote it).
    fn select_subject(&self, s: &mut S, cx: &mut dyn Cx, subject: &str) {
        let _ = (s, cx, subject);
    }
    /// The user took `action` on row `row` (a parameter's name) of node
    /// `node`'s sheet: [`RowAction::Pick`] on a row offering a pick in the
    /// viewport (a group from the node's input). Core's sheet draws the
    /// rows and owns file, path and reference rows itself.
    fn row_action(&self, s: &mut S, cx: &mut dyn Cx, node: NodeRef, row: &str, action: RowAction) {
        let _ = (s, cx, node, row, action);
    }
    /// The text being typed into row `row` of node `node`'s sheet, as it
    /// is now (each change); `None` once typing ends. For live feedback
    /// (the elements a group expression names), never the edit itself.
    fn typing(&self, s: &mut S, cx: &mut dyn Cx, node: NodeRef, row: &str, text: Option<&str>) {
        let _ = (s, cx, node, row, text);
    }
}

/// What the user did on an inspector row ([`Inspector::row_action`]). New
/// actions may be added; a context that doesn't know one ignores it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum RowAction {
    /// Pick in the viewport for the row (its pick button).
    Pick,
}

/// How a context's items run, and what of them applies now. The items
/// themselves, and every menu, shelf, key and Tab entry holding them, are
/// its [`Library`](super::Library).
pub trait Vocabulary<S: ?Sized>: Send + Sync {
    /// Its name, which actions carry ("acme.trees").
    fn name(&self) -> &str;
    /// Runs `item`, as `how` says: a tool over the views, its node over
    /// the network, or again with its last values (the host's Repeat). An
    /// item can be run where it doesn't apply (a key, a shelf tile): it
    /// then does nothing but say why
    /// ([`HostStatus::fail`](crate::host::HostStatus::fail)).
    fn run(&self, s: &mut S, cx: &mut dyn Cx, item: &str, how: Invoked);
    /// Whether `item` applies now: at the active level, to the selection,
    /// in this mode (Fillet needs edges selected; Repeat a last tool).
    /// Menus, Tab and the marking menus hide what doesn't apply; nothing
    /// is greyed. Never a passing state (a cook under way): [`run`](Self::run)
    /// refuses that, saying why. Called while a menu shows, once an entry
    /// a frame.
    fn shown(&self, s: &S, cx: &dyn Cx, item: &str) -> bool {
        let _ = (s, cx, item);
        true
    }
    /// `item`'s label now, when its state renames it ("Repeat Bevel",
    /// "Slice Off"); `None`: its library label.
    fn label(&self, s: &S, cx: &dyn Cx, item: &str) -> Option<String> {
        let _ = (s, cx, item);
        None
    }
}

/// Where an item was run from: what running it means.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Invoked {
    /// Over the views, from a menu, shelf, key or Tab listing tools: the
    /// tool runs on what is selected.
    #[default]
    Views,
    /// From Tab over the network: its node is added.
    Network,
    /// The host's Repeat: the item runs again with its last values (the
    /// package keeps them; an item it has none for runs as from the views).
    Repeat,
    /// At an object level (the scene root, a Part), from its menus and
    /// shelves: the item runs on the selected objects it takes
    /// ([`Places::objects`](super::Places::objects)).
    Objects,
}

/// The inspector rows of the object kinds a context owns, below an
/// object's Name and Type.
pub trait ObjectRows<S: ?Sized>: Send + Sync {
    /// The kinds whose rows it gives. Read once.
    fn kinds(&self) -> Vec<ObjectKindId>;
    /// Object `object`'s rows.
    fn rows(&self, s: &S, cx: &dyn Cx, object: ObjectRef) -> Schema<u32>;
    /// The edits of those rows.
    fn apply(
        &self,
        s: &mut S,
        cx: &mut dyn Cx,
        object: ObjectRef,
        rows: &Schema<u32>,
        edits: &Edits<u32>,
    );
    /// Whether object `object` is cooking (its outliner row and tile show
    /// it busy).
    fn cooking(&self, s: &S, cx: &dyn Cx, object: ObjectRef) -> bool {
        let _ = (s, cx, object);
        false
    }
    /// Object `object` is inspected for the first time since the document
    /// opened (a printer reads its trays when it has none yet). Nothing by
    /// default.
    fn opened(&self, s: &mut S, cx: &mut dyn Cx, object: ObjectRef) {
        let _ = (s, cx, object);
    }
}

/// What the outliner lists inside objects of the kinds a context owns.
pub trait OutlineRows<S: ?Sized>: Send + Sync {
    /// The kinds whose rows it gives. Read once.
    fn kinds(&self) -> Vec<ObjectKindId>;
    /// The rows under object `object`; with `searching` they include those
    /// inside collapsed branches.
    fn rows(&self, s: &S, cx: &dyn Cx, object: ObjectRef, searching: bool) -> Vec<OutlineRow>;
    /// One of its rows picked.
    fn pick(&self, s: &mut S, cx: &mut dyn Cx, row: &OutlineRow);
}

/// A part over `S` as the same part over `dyn Any`: the host's state,
/// lent as `dyn Any`, is downcast back to `S` for each call.
pub struct Erased<S, P: ?Sized> {
    pub(super) part: Box<P>,
    state: PhantomData<fn() -> S>,
}

impl<S, P: ?Sized> Erased<S, P> {
    /// `part`, erased.
    pub fn new(part: Box<P>) -> Self {
        Erased {
            part,
            state: PhantomData,
        }
    }
}

/// The state the host lent, as the package's own.
pub(super) fn own<S: 'static>(s: &dyn Any) -> &S {
    s.downcast_ref()
        .expect("the host lends each package its own state")
}

pub(super) fn own_mut<S: 'static>(s: &mut dyn Any) -> &mut S {
    s.downcast_mut()
        .expect("the host lends each package its own state")
}

impl<S: 'static> Levels<dyn Any> for Erased<S, dyn Levels<S>> {
    fn levels(&self) -> Vec<LevelSpec> {
        self.part.levels()
    }
    fn active(&self, s: &dyn Any, cx: &dyn Cx) -> Vec<usize> {
        self.part.active(own(s), cx)
    }
    fn primary(&self, s: &dyn Any, cx: &dyn Cx) -> usize {
        self.part.primary(own(s), cx)
    }
    fn choose(&self, s: &mut dyn Any, cx: &mut dyn Cx, level: usize) {
        self.part.choose(own_mut(s), cx, level)
    }
    fn toggle(&self, s: &mut dyn Any, cx: &mut dyn Cx, level: usize) {
        self.part.toggle(own_mut(s), cx, level)
    }
    fn selected(&self, s: &dyn Any, cx: &dyn Cx) -> Option<usize> {
        self.part.selected(own(s), cx)
    }
    fn components(&self, s: &dyn Any, cx: &dyn Cx) -> Option<(ObjectRef, Components)> {
        self.part.components(own(s), cx)
    }
    fn hovered(&self, s: &mut dyn Any, cx: &mut dyn Cx, hit: Option<Components>) {
        self.part.hovered(own_mut(s), cx, hit)
    }
    fn picked(&self, s: &mut dyn Any, cx: &mut dyn Cx, hit: Option<Components>, how: Combine) {
        self.part.picked(own_mut(s), cx, hit, how)
    }
    fn pivot(&self, s: &mut dyn Any, cx: &mut dyn Cx) -> Option<PivotFrame> {
        self.part.pivot(own_mut(s), cx)
    }
    fn pivot_changed(&self, s: &mut dyn Any, cx: &mut dyn Cx) {
        self.part.pivot_changed(own_mut(s), cx)
    }
    fn normal(&self, s: &mut dyn Any, cx: &mut dyn Cx) -> Option<Vec3> {
        self.part.normal(own_mut(s), cx)
    }
    fn preset(&self) -> Option<LevelPreset> {
        self.part.preset()
    }
    fn preset_on(&self, s: &dyn Any, cx: &dyn Cx) -> bool {
        self.part.preset_on(own(s), cx)
    }
    fn toggle_preset(&self, s: &mut dyn Any, cx: &mut dyn Cx) {
        self.part.toggle_preset(own_mut(s), cx)
    }
    fn candidates(&self, s: &dyn Any, cx: &dyn Cx, rect: Rect, pointer: Pos) -> Vec<Candidate> {
        self.part.candidates(own(s), cx, rect, pointer)
    }
    fn pick_candidate(
        &self,
        s: &mut dyn Any,
        cx: &mut dyn Cx,
        candidate: &Candidate,
        shift: bool,
        command: bool,
    ) {
        self.part
            .pick_candidate(own_mut(s), cx, candidate, shift, command)
    }
    fn hover_candidate(&self, s: &mut dyn Any, cx: &mut dyn Cx, candidate: Option<&Candidate>) {
        self.part.hover_candidate(own_mut(s), cx, candidate)
    }
}

impl<S: 'static> ViewportLayer<dyn Any> for Erased<S, dyn ViewportLayer<S>> {
    fn cancel(&self, s: &mut dyn Any, cx: &mut dyn Cx) {
        self.part.cancel(own_mut(s), cx)
    }
    fn key(&self, s: &mut dyn Any, cx: &mut dyn Cx, press: &KeyPress) -> bool {
        self.part.key(own_mut(s), cx, press)
    }
    fn holds(&self, s: &dyn Any, cx: &dyn Cx) -> Holds {
        self.part.holds(own(s), cx)
    }
    fn rows(&self, s: &mut dyn Any, cx: &mut dyn Cx, popover: Popover) -> Vec<PopoverRow> {
        self.part.rows(own_mut(s), cx, popover)
    }
    fn apply_row(&self, s: &mut dyn Any, cx: &mut dyn Cx, popover: Popover, edit: RowEdit) {
        self.part.apply_row(own_mut(s), cx, popover, edit)
    }
    fn numeric_bar(&self, s: &mut dyn Any, cx: &mut dyn Cx) -> Option<NumericBar> {
        self.part.numeric_bar(own_mut(s), cx)
    }
    fn apply_numeric(
        &self,
        s: &mut dyn Any,
        cx: &mut dyn Cx,
        field: usize,
        current: f64,
        value: f64,
    ) {
        self.part
            .apply_numeric(own_mut(s), cx, field, current, value)
    }
    fn edited(&self, s: &mut dyn Any, cx: &mut dyn Cx, view: usize) -> Option<Edited> {
        self.part.edited(own_mut(s), cx, view)
    }
    fn tools(&self, s: &mut dyn Any, cx: &mut dyn ViewCx, ui: &mut dyn Ui) -> Tools {
        self.part.tools(own_mut(s), cx, ui)
    }
    fn own_targets(&self, s: &dyn Any, cx: &dyn Cx) -> Option<Polygons> {
        self.part.own_targets(own(s), cx)
    }
    fn overlay(&self, s: &mut dyn Any, cx: &mut dyn ViewCx, ui: &mut dyn Ui) {
        self.part.overlay(own_mut(s), cx, ui)
    }
    fn live_node(&self, s: &dyn Any, cx: &dyn Cx) -> Option<NodeRef> {
        self.part.live_node(own(s), cx)
    }
    fn own_gizmo(&self, s: &dyn Any, cx: &dyn Cx) -> bool {
        self.part.own_gizmo(own(s), cx)
    }
    fn transforms(&self, s: &dyn Any, cx: &dyn Cx) -> bool {
        self.part.transforms(own(s), cx)
    }
    fn transform(
        &self,
        s: &mut dyn Any,
        cx: &mut dyn Cx,
        phase: TransformPhase,
        step: &TransformStep,
    ) {
        self.part.transform(own_mut(s), cx, phase, step)
    }
    fn cards(&self, s: &mut dyn Any, cx: &mut dyn Cx, ui: &mut dyn Ui, rect: Rect) {
        self.part.cards(own_mut(s), cx, ui, rect)
    }
    fn controls(&self, s: &dyn Any, cx: &dyn Cx, view: Rect) -> Vec<Rect> {
        self.part.controls(own(s), cx, view)
    }
    fn time(&self, s: &mut dyn Any, cx: &mut dyn Cx) -> Option<TimeDomain> {
        self.part.time(own_mut(s), cx)
    }
    fn set_time(&self, s: &mut dyn Any, cx: &mut dyn Cx, t: f64) {
        self.part.set_time(own_mut(s), cx, t)
    }
}

impl<S: 'static> Inspector<dyn Any> for Erased<S, dyn Inspector<S>> {
    fn show(&self, s: &mut dyn Any, cx: &mut dyn InspectorCx, ui: &mut dyn Ui) {
        self.part.show(own_mut(s), cx, ui)
    }
    fn select_subject(&self, s: &mut dyn Any, cx: &mut dyn Cx, subject: &str) {
        self.part.select_subject(own_mut(s), cx, subject)
    }
    fn row_action(
        &self,
        s: &mut dyn Any,
        cx: &mut dyn Cx,
        node: NodeRef,
        row: &str,
        action: RowAction,
    ) {
        self.part.row_action(own_mut(s), cx, node, row, action)
    }
    fn typing(
        &self,
        s: &mut dyn Any,
        cx: &mut dyn Cx,
        node: NodeRef,
        row: &str,
        text: Option<&str>,
    ) {
        self.part.typing(own_mut(s), cx, node, row, text)
    }
}

impl<S: 'static> Vocabulary<dyn Any> for Erased<S, dyn Vocabulary<S>> {
    fn name(&self) -> &str {
        self.part.name()
    }
    fn run(&self, s: &mut dyn Any, cx: &mut dyn Cx, item: &str, how: Invoked) {
        self.part.run(own_mut(s), cx, item, how)
    }
    fn shown(&self, s: &dyn Any, cx: &dyn Cx, item: &str) -> bool {
        self.part.shown(own(s), cx, item)
    }
    fn label(&self, s: &dyn Any, cx: &dyn Cx, item: &str) -> Option<String> {
        self.part.label(own(s), cx, item)
    }
}

impl<S: 'static> ObjectRows<dyn Any> for Erased<S, dyn ObjectRows<S>> {
    fn kinds(&self) -> Vec<ObjectKindId> {
        self.part.kinds()
    }
    fn rows(&self, s: &dyn Any, cx: &dyn Cx, object: ObjectRef) -> Schema<u32> {
        self.part.rows(own(s), cx, object)
    }
    fn apply(
        &self,
        s: &mut dyn Any,
        cx: &mut dyn Cx,
        object: ObjectRef,
        rows: &Schema<u32>,
        edits: &Edits<u32>,
    ) {
        self.part.apply(own_mut(s), cx, object, rows, edits)
    }
    fn cooking(&self, s: &dyn Any, cx: &dyn Cx, object: ObjectRef) -> bool {
        self.part.cooking(own(s), cx, object)
    }
    fn opened(&self, s: &mut dyn Any, cx: &mut dyn Cx, object: ObjectRef) {
        self.part.opened(own_mut(s), cx, object)
    }
}

impl<S: 'static> OutlineRows<dyn Any> for Erased<S, dyn OutlineRows<S>> {
    fn kinds(&self) -> Vec<ObjectKindId> {
        self.part.kinds()
    }
    fn rows(
        &self,
        s: &dyn Any,
        cx: &dyn Cx,
        object: ObjectRef,
        searching: bool,
    ) -> Vec<OutlineRow> {
        self.part.rows(own(s), cx, object, searching)
    }
    fn pick(&self, s: &mut dyn Any, cx: &mut dyn Cx, row: &OutlineRow) {
        self.part.pick(own_mut(s), cx, row)
    }
}
