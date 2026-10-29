//! A context's parts, one trait each, over the package's state `S`: the
//! selection levels, the viewport layer, the inspector, the vocabulary,
//! the network view, the object kinds' inspector and outliner rows, and
//! the object levels' creations.
//!
//! Each method gets the package's state and the call's context (`cx`);
//! a query gets them shared (`&S`, `&dyn Cx`) and cannot change anything.
//! Parts are stateless values: their state is `S`, which the host lends
//! for one call at a time ([`register`](super::register)). Every method
//! runs on the UI thread and returns at once.
//!
//! The host erases `S` when a package registers: [`Erased`] turns each
//! part over `S` into the same part over `dyn Any`, downcasting the state
//! the host lends back to `S`.
use super::data::{
    ContextView, Creation, FlagSpec, LevelSpec, NetworkNode, NetworkWire, NumericBar, OutlineRow,
    Status, TimeDomain, ToolbarItem, Tools, VocabularyItem,
};
use super::edited::{Edited, TemplateMark};
use crate::host::{Cx, InspectorCx, NodeRef, ObjectRef, ViewCx};
use crate::sheet::{Edits, Schema};
use crate::{Pos, Rect, Ui};
use kinogaki_studio_api::ObjectKindId;
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
    /// The pointer over view `rect` (`None`: off it): show what lies under
    /// it hovered.
    fn hover(&self, s: &mut S, cx: &mut dyn Cx, rect: Rect, pointer: Option<Pos>) {
        let _ = (s, cx, rect, pointer);
    }
    /// A click in view `rect`: select what lies under `pointer` (Shift
    /// adds, Command removes).
    fn click(
        &self,
        s: &mut S,
        cx: &mut dyn Cx,
        rect: Rect,
        pointer: Pos,
        shift: bool,
        command: bool,
    ) {
        let _ = (s, cx, rect, pointer, shift, command);
    }
}

/// What a context adds to the viewport: its toolbar, status, numeric bar,
/// its own drawing in the views (line layers and shaded objects), its live
/// tools' pointer handling and overlays, and its time.
pub trait ViewportLayer<S: ?Sized>: Send + Sync {
    /// The context's name for the viewport ("Mesh", "CAD sketch"): the
    /// host calls [`leave`](Self::leave) and [`enter`](Self::enter) when it
    /// changes.
    fn name(&self) -> &str;
    /// The location became this layer's (once, before its first frame).
    fn enter(&self, s: &mut S, cx: &mut dyn Cx) {
        let _ = (s, cx);
    }
    /// The location stops being this layer's.
    fn leave(&self, s: &mut S, cx: &mut dyn Cx) {
        let _ = (s, cx);
    }
    /// Escape, or an undo: end live drags and gestures.
    fn cancel(&self, s: &mut S, cx: &mut dyn Cx) {
        let _ = (s, cx);
    }
    /// A drag in a view holds the views: no other view takes the pointer.
    fn holds_views(&self, s: &S, cx: &dyn Cx) -> bool {
        let _ = (s, cx);
        false
    }
    /// The left toolbar, in order; empty: no toolbar.
    fn toolbar(&self, s: &S, cx: &dyn Cx) -> Vec<ToolbarItem> {
        let _ = (s, cx);
        Vec::new()
    }
    /// A toolbar item pressed.
    fn run_tool(&self, s: &mut S, cx: &mut dyn Cx, id: &str) {
        let _ = (s, cx, id);
    }
    /// The status bar's pieces.
    fn status(&self, s: &mut S, cx: &mut dyn Cx) -> Status {
        let _ = (s, cx);
        Status::default()
    }
    /// The numeric bar for the current tool and selection.
    fn numeric_bar(&self, s: &mut S, cx: &mut dyn Cx) -> Option<NumericBar> {
        let _ = (s, cx);
        None
    }
    /// A value typed into field `field` (from `current`).
    fn apply_numeric(&self, s: &mut S, cx: &mut dyn Cx, field: usize, current: f64, value: f64) {
        let _ = (s, cx, field, current, value);
    }
    /// Frames the active view on what F frames here.
    fn frame(&self, s: &mut S, cx: &mut dyn Cx) {
        let _ = (s, cx);
    }
    /// Once a frame before the views: refresh the context's view services.
    fn prepare(&self, s: &mut S, cx: &mut dyn Cx) {
        let _ = (s, cx);
    }
    /// What the views draw of the context's own.
    fn layers(&self, s: &mut S, cx: &mut dyn Cx) -> Option<ContextView> {
        let _ = (s, cx);
        None
    }
    /// What the open location's edited surface shows in view `view` this
    /// frame (`None`: no edited surface). Called once per drawn view
    /// after the views' input; core draws it with the rest of the scene.
    fn edited(&self, s: &mut S, cx: &mut dyn Cx, view: usize) -> Option<Edited> {
        let _ = (s, cx, view);
        None
    }
    /// How the open object's templated piece of node `node` is marked
    /// (`key`: its data type's name for the piece, a CAD body's name).
    fn template_mark(
        &self,
        s: &S,
        cx: &dyn Cx,
        node: NodeRef,
        key: Option<&(dyn Any + Send + Sync)>,
    ) -> TemplateMark {
        let _ = (s, cx, node, key);
        TemplateMark::default()
    }
    /// This frame's live tools in a view, before camera navigation.
    fn tools(&self, s: &mut S, cx: &mut dyn ViewCx, ui: &mut dyn Ui) -> Tools {
        let _ = (s, cx, ui);
        Tools::default()
    }
    /// A view's gizmo, hover, picking and marquee, after the camera and the
    /// view cube had the pointer; `tools` is what [`Self::tools`] said.
    fn pointer(&self, s: &mut S, cx: &mut dyn ViewCx, ui: &mut dyn Ui, tools: Tools) {
        let _ = (s, cx, ui, tools);
    }
    /// Draws over a view after its pointer handling: labels, handles,
    /// heads-up value fields.
    fn overlay(&self, s: &mut S, cx: &mut dyn ViewCx, ui: &mut dyn Ui) {
        let _ = (s, cx, ui);
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

/// The inspector panel while the context is open.
pub trait Inspector<S: ?Sized>: Send + Sync {
    /// Draws the panel (its sheets through [`Ui::sheet`]).
    fn show(&self, s: &mut S, cx: &mut dyn InspectorCx, ui: &mut dyn Ui);
    /// Selects what a Console record's subject names inside the open
    /// object (its own spelling, as its reports wrote it).
    fn select_subject(&self, s: &mut S, cx: &mut dyn Cx, subject: &str) {
        let _ = (s, cx, subject);
    }
}

/// Every tool and command a context offers, by stored name: what its
/// menus, shelves and Tab hold.
pub trait Vocabulary<S: ?Sized>: Send + Sync {
    /// Its name, which actions carry ("acme.trees").
    fn name(&self) -> &str;
    /// Its items, in the order Tab and the Settings window list them.
    fn items(&self) -> Vec<VocabularyItem>;
    /// Runs `item`.
    fn run(&self, s: &mut S, cx: &mut dyn Cx, item: &str);
    /// Why `item` can't run now; `None` when it can.
    fn unavailable(&self, s: &S, cx: &dyn Cx, item: &str) -> Option<String> {
        let _ = (s, cx, item);
        None
    }
}

/// The context's network as the one network view draws it: its tiles,
/// wires and flags, and what picking and flagging do.
pub trait NetworkView<S: ?Sized>: Send + Sync {
    /// The flags its tiles can light, by index. Read once.
    fn flags(&self) -> Vec<FlagSpec>;
    /// The open level's tiles.
    fn nodes(&self, s: &mut S, cx: &mut dyn Cx) -> Vec<NetworkNode>;
    /// The open level's wires.
    fn wires(&self, s: &mut S, cx: &mut dyn Cx) -> Vec<NetworkWire>;
    /// Picks `node` (`None` clears); `add` is Shift, `toggle` Command.
    fn pick(&self, s: &mut S, cx: &mut dyn Cx, node: Option<NodeRef>, add: bool, toggle: bool);
    /// Toggles flag `flag` of `node` (one undo step).
    fn flag(&self, s: &mut S, cx: &mut dyn Cx, node: NodeRef, flag: usize);
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

/// The object kinds a context adds to an object level's Create, and the
/// starts it offers with them.
pub trait Creations<S: ?Sized>: Send + Sync {
    /// Its creations. Read once.
    fn creations(&self) -> Vec<Creation>;
    /// Makes a new object of `kind` starting with `start` (one of its
    /// creation's [`starts`](Creation::starts)) at the open object level,
    /// selected, in one step.
    fn start(
        &self,
        s: &mut S,
        cx: &mut dyn Cx,
        kind: ObjectKindId,
        start: &str,
    ) -> Result<(), String>;
}

/// A part over `S` as the same part over `dyn Any`: the host's state,
/// lent as `dyn Any`, is downcast back to `S` for each call.
pub struct Erased<S, P: ?Sized> {
    part: Box<P>,
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
fn own<S: 'static>(s: &dyn Any) -> &S {
    s.downcast_ref()
        .expect("the host lends each package its own state")
}

fn own_mut<S: 'static>(s: &mut dyn Any) -> &mut S {
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
    fn hover(&self, s: &mut dyn Any, cx: &mut dyn Cx, rect: Rect, pointer: Option<Pos>) {
        self.part.hover(own_mut(s), cx, rect, pointer)
    }
    fn click(
        &self,
        s: &mut dyn Any,
        cx: &mut dyn Cx,
        rect: Rect,
        pointer: Pos,
        shift: bool,
        command: bool,
    ) {
        self.part
            .click(own_mut(s), cx, rect, pointer, shift, command)
    }
}

impl<S: 'static> ViewportLayer<dyn Any> for Erased<S, dyn ViewportLayer<S>> {
    fn name(&self) -> &str {
        self.part.name()
    }
    fn enter(&self, s: &mut dyn Any, cx: &mut dyn Cx) {
        self.part.enter(own_mut(s), cx)
    }
    fn leave(&self, s: &mut dyn Any, cx: &mut dyn Cx) {
        self.part.leave(own_mut(s), cx)
    }
    fn cancel(&self, s: &mut dyn Any, cx: &mut dyn Cx) {
        self.part.cancel(own_mut(s), cx)
    }
    fn holds_views(&self, s: &dyn Any, cx: &dyn Cx) -> bool {
        self.part.holds_views(own(s), cx)
    }
    fn toolbar(&self, s: &dyn Any, cx: &dyn Cx) -> Vec<ToolbarItem> {
        self.part.toolbar(own(s), cx)
    }
    fn run_tool(&self, s: &mut dyn Any, cx: &mut dyn Cx, id: &str) {
        self.part.run_tool(own_mut(s), cx, id)
    }
    fn status(&self, s: &mut dyn Any, cx: &mut dyn Cx) -> Status {
        self.part.status(own_mut(s), cx)
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
    fn frame(&self, s: &mut dyn Any, cx: &mut dyn Cx) {
        self.part.frame(own_mut(s), cx)
    }
    fn prepare(&self, s: &mut dyn Any, cx: &mut dyn Cx) {
        self.part.prepare(own_mut(s), cx)
    }
    fn layers(&self, s: &mut dyn Any, cx: &mut dyn Cx) -> Option<ContextView> {
        self.part.layers(own_mut(s), cx)
    }
    fn edited(&self, s: &mut dyn Any, cx: &mut dyn Cx, view: usize) -> Option<Edited> {
        self.part.edited(own_mut(s), cx, view)
    }
    fn template_mark(
        &self,
        s: &dyn Any,
        cx: &dyn Cx,
        node: NodeRef,
        key: Option<&(dyn Any + Send + Sync)>,
    ) -> TemplateMark {
        self.part.template_mark(own(s), cx, node, key)
    }
    fn tools(&self, s: &mut dyn Any, cx: &mut dyn ViewCx, ui: &mut dyn Ui) -> Tools {
        self.part.tools(own_mut(s), cx, ui)
    }
    fn pointer(&self, s: &mut dyn Any, cx: &mut dyn ViewCx, ui: &mut dyn Ui, tools: Tools) {
        self.part.pointer(own_mut(s), cx, ui, tools)
    }
    fn overlay(&self, s: &mut dyn Any, cx: &mut dyn ViewCx, ui: &mut dyn Ui) {
        self.part.overlay(own_mut(s), cx, ui)
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
}

impl<S: 'static> Vocabulary<dyn Any> for Erased<S, dyn Vocabulary<S>> {
    fn name(&self) -> &str {
        self.part.name()
    }
    fn items(&self) -> Vec<VocabularyItem> {
        self.part.items()
    }
    fn run(&self, s: &mut dyn Any, cx: &mut dyn Cx, item: &str) {
        self.part.run(own_mut(s), cx, item)
    }
    fn unavailable(&self, s: &dyn Any, cx: &dyn Cx, item: &str) -> Option<String> {
        self.part.unavailable(own(s), cx, item)
    }
}

impl<S: 'static> NetworkView<dyn Any> for Erased<S, dyn NetworkView<S>> {
    fn flags(&self) -> Vec<FlagSpec> {
        self.part.flags()
    }
    fn nodes(&self, s: &mut dyn Any, cx: &mut dyn Cx) -> Vec<NetworkNode> {
        self.part.nodes(own_mut(s), cx)
    }
    fn wires(&self, s: &mut dyn Any, cx: &mut dyn Cx) -> Vec<NetworkWire> {
        self.part.wires(own_mut(s), cx)
    }
    fn pick(
        &self,
        s: &mut dyn Any,
        cx: &mut dyn Cx,
        node: Option<NodeRef>,
        add: bool,
        toggle: bool,
    ) {
        self.part.pick(own_mut(s), cx, node, add, toggle)
    }
    fn flag(&self, s: &mut dyn Any, cx: &mut dyn Cx, node: NodeRef, flag: usize) {
        self.part.flag(own_mut(s), cx, node, flag)
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

impl<S: 'static> Creations<dyn Any> for Erased<S, dyn Creations<S>> {
    fn creations(&self) -> Vec<Creation> {
        self.part.creations()
    }
    fn start(
        &self,
        s: &mut dyn Any,
        cx: &mut dyn Cx,
        kind: ObjectKindId,
        start: &str,
    ) -> Result<(), String> {
        self.part.start(own_mut(s), cx, kind, start)
    }
}
