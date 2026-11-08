//! The host's capabilities, one trait each: what a package's UI may ask of
//! Studio. [`Cx`](super::Cx) reaches them all.
//!
//! **Rules every implementation follows.** Each method runs on the UI
//! thread, inside one call into a package, and returns at once (no
//! blocking, no waiting on workers). None calls back into a package:
//! whatever would (a cancelled gesture ending a package's live tool, an
//! action running a package's command) is queued and runs after the
//! current call returns, so a package never sees its own state taken
//! while it holds it.
use super::data::{NodeRef, ObjectRef, Place, ViewCamera};
use crate::gizmo::{TransformSpace, TransformTool};
use kinogaki_studio_api::Diagnostic;
use kinogaki_studio_api::geometry::Geometry;
use kinogaki_studio_api::id::StoredId;
use kinogaki_studio_api::session::Slot;
use kinogaki_studio_api::stored::Stored;
use kinogaki_studio_api::{
    Begun, Bounds, Components, Data, Drawn, Error, GestureMark, GestureToken, NodeTypeId,
    ObjectBody, ObjectKindId, Parameters, SlotId, Time, Vec3,
};
use std::any::Any;
use std::sync::Arc;

/// Undo, redo and gestures.
///
/// An edit through the host ([`HostSlotsExt::edit_slot`], a selection) is one
/// undo step of its own, unless a gesture is open: then every change up
/// to its finish is one step, named when the gesture began (a drag). The
/// gesture is its opener's: [`begin_gesture`](HostUndo::begin_gesture)
/// gives the [`GestureToken`] that alone finishes or cancels it.
pub trait HostUndo {
    /// Opens a gesture named `label`: [`Begun::Own`] with its token, or,
    /// while a gesture is open already (a live tool's run),
    /// [`Begun::Joined`]: what follows is part of that one, which its
    /// opener ends, and [`back_to`](Self::back_to) the mark puts back only
    /// what followed.
    fn begin_gesture(&mut self, label: &str) -> Begun;
    /// Closes `gesture` as one step (nothing when nothing changed, or
    /// when it already ended: an undo cancels it). A result that doesn't
    /// check is put back, and the error says why.
    ///
    /// # Errors
    /// What the gesture made doesn't check.
    fn finish_gesture(&mut self, gesture: GestureToken) -> Result<(), Error>;
    /// Puts back everything `gesture` changed, and records nothing.
    fn cancel_gesture(&mut self, gesture: GestureToken);
    /// Puts back everything the open gesture changed since `mark`,
    /// keeping it open: from [`GestureToken::start`], a live tool
    /// recomputing from the gesture's start each step (an adjustment, a
    /// pen stroke) stays one undo step. Nothing happens when `mark`'s
    /// gesture isn't open.
    fn back_to(&mut self, mark: GestureMark);
    /// Whether a gesture is open, anyone's.
    fn in_gesture(&self) -> bool;
    /// Undoes the last step (live tools end first, after this call).
    fn undo(&mut self);
    /// Redoes the last undone step.
    fn redo(&mut self);
    /// The step Undo would undo, by its label.
    fn undo_label(&self) -> Option<String>;
    /// The step Redo would redo, by its label.
    fn redo_label(&self) -> Option<String>;
}

/// The session's slots ([`Slot`]): package-owned state undone with the
/// document. Untyped here so the trait stays object-safe; packages use the
/// typed methods of [`HostSlotsExt`].
pub trait HostSlots {
    /// Slot `id`'s value; `None` when no package registered it.
    fn slot_any(&self, id: SlotId) -> Option<&(dyn Any + Send + Sync)>;
    /// Changes slot `id`'s value in place with `edit`, as an undo step
    /// named `label` (inside the open gesture, part of its step; nothing
    /// is recorded when nothing changed). The host copies a value a
    /// snapshot still shares first. `false`: no such slot.
    fn edit_slot_any(
        &mut self,
        id: SlotId,
        label: &str,
        edit: &mut dyn FnMut(&mut (dyn Any + Send + Sync)),
    ) -> bool;
}

/// Typed access to the session's slots.
pub trait HostSlotsExt: HostSlots {
    /// Slot `T`'s value.
    ///
    /// # Panics
    /// When no package registered `T` (a programming error: a package
    /// registers its slots with [`Package::with_slot`](kinogaki_studio_api::Package::with_slot)).
    fn slot<T: Slot>(&self) -> &T {
        self.slot_any(T::id())
            .and_then(|v| v.downcast_ref())
            .unwrap_or_else(|| unregistered::<T>())
    }
    /// Changes slot `T` with `edit`, as an undo step named `label` (or
    /// part of the open gesture's): the one way a slot changes, so every
    /// change is undone with the document.
    ///
    /// # Panics
    /// When no package registered `T`.
    fn edit_slot<T: Slot>(&mut self, label: &str, edit: impl FnOnce(&mut T)) {
        let mut edit = Some(edit);
        let found = self.edit_slot_any(T::id(), label, &mut |value| {
            if let (Some(value), Some(edit)) = (value.downcast_mut::<T>(), edit.take()) {
                edit(value);
            }
        });
        if !found {
            unregistered::<T>()
        }
    }
}

impl<H: HostSlots + ?Sized> HostSlotsExt for H {}

fn unregistered<T: Slot>() -> ! {
    panic!(
        "session slot {} is not registered ({})",
        T::id(),
        std::any::type_name::<T>()
    )
}

/// What is selected: objects at an object level, the components of one
/// object, nodes in the open network.
pub trait HostSelection {
    /// The objects selected at the object levels.
    fn selected_objects(&self) -> Vec<ObjectRef>;
    /// Selects `objects` (only those), as a click does.
    fn select_objects(&mut self, objects: &[ObjectRef]);
    /// The components selected on `object`, one set per element kind, as
    /// the open context's levels picked them, in the representation of the
    /// value they were picked on (a set picked on another topology is
    /// still held: the context checks it fits what shows). Empty when
    /// none are, or the selection is another object's.
    fn selected_components(&self, object: ObjectRef) -> Vec<Components>;
    /// Makes `sets` the components selected on `object` (empty: none), as
    /// one undo step called "Select", or part of the open gesture.
    fn select_components(&mut self, object: ObjectRef, sets: Vec<Components>);
    /// The nodes picked on the open level.
    fn picked_nodes(&self) -> Vec<NodeRef>;
    /// The active node (the inspected one).
    fn active_node(&self) -> Option<NodeRef>;
    /// Picks `node`: alone, or added to the picked ones with `add`.
    fn pick_node(&mut self, node: NodeRef, add: bool);
}

/// The document: where the user is and the objects' own data.
pub trait HostDoc {
    /// Where the user is.
    fn place(&self) -> Place;
    /// Shows `place` (navigation, not an undo step) after the current call
    /// returns, as everything that could reach a package does: the open
    /// context's live tools keep what they hold first
    /// ([`LiveTools::accept`](crate::package::LiveTools::accept)), the
    /// location still theirs. `Err` says why it can't (the object is
    /// locked or gone), and nothing moves.
    fn go_to(&mut self, place: Place) -> Result<(), Error>;
    /// Bumped by every change to a network: what a cook would see.
    fn revision(&self) -> u64;
    /// Object `object`'s name.
    fn object_name(&self, object: ObjectRef) -> Option<String>;
    /// Object `object`'s kind.
    fn object_kind(&self, object: ObjectRef) -> Option<ObjectKindId>;
    /// Object `object`'s package-owned body, when its kind is a
    /// package's (read it with `downcast_ref`).
    fn object_body(&self, object: ObjectRef) -> Option<Arc<dyn ObjectBody>>;
    /// Gives object `object` `body` instead, as one undo step called
    /// `label` (a printer's trays read again); `Err` says why not (the
    /// object is gone, locked, or `body` is of another kind).
    fn set_object_body(
        &mut self,
        object: ObjectRef,
        label: &str,
        body: Arc<dyn ObjectBody>,
    ) -> Result<(), Error>;
    /// Every object of kind `kind`, in the scene's order.
    fn objects_of_kind(&self, kind: ObjectKindId) -> Vec<ObjectRef>;
    /// Stored value `id` (a package's [`Stored`]): the project's or the
    /// user's. `None` when no package registered it for either.
    fn stored_any(&self, id: StoredId) -> Option<&(dyn Any + Send + Sync)>;
    /// Sets stored value `id` (the project's or the user's) to `value`: a
    /// project's value is a change to the project, saved with it; a user's
    /// is saved with the user's settings. `false`: no such value, or
    /// `value` is not of its type.
    fn set_stored_any(&mut self, id: StoredId, value: Arc<dyn Any + Send + Sync>) -> bool;
    /// Object `object`'s shown result: the value its network yields (the
    /// open object: what it displays) and its draw pieces, as the host
    /// last published them.
    fn shown(&self, object: ObjectRef) -> Option<(Data, Drawn)>;
    /// The polygons the host draws object `object`'s shown result as,
    /// sharing the drawn columns (no copy): a polygons value itself; a
    /// B-rep's bodies meshed by the installed mesher and joined at their
    /// placements, `face` and `edge` naming the value's faces and edges
    /// (in the value's numbering), `N` and `uv` on the polygons' corners
    /// as the mesher made them. `None` while it isn't meshed
    /// yet, or for a value drawn otherwise (curves).
    fn shown_polygons(&self, object: ObjectRef) -> Option<Geometry>;
    /// Whether the node the open network displays is a held edit (an Edit
    /// Mesh): the library's entries for editing show, and a key store
    /// active while editing applies.
    fn editing(&self) -> bool;
    /// The scene's current time: what cooks and keys are at.
    fn time(&self) -> Time;
    /// Shows the scene at `time`, kept inside its frame range (navigation,
    /// not an undo step): a package's "go to key" button.
    fn set_time(&mut self, time: Time);
}

/// Typed access to the stored values.
pub trait HostDocExt: HostDoc {
    /// The project's value `T`, or the user's for a
    /// [`Scope::User`](kinogaki_studio_api::Scope::User) one.
    ///
    /// # Panics
    /// When no package registered `T` for the project or user (a programming
    /// error: a package registers its values with
    /// [`Package::with_stored`](kinogaki_studio_api::Package::with_stored)).
    fn stored<T: Stored>(&self) -> &T {
        self.stored_any(T::id())
            .and_then(|v| v.downcast_ref())
            .unwrap_or_else(|| unstored::<T>())
    }
}

impl<H: HostDoc + ?Sized> HostDocExt for H {}

fn unstored<T: Stored>() -> ! {
    panic!(
        "stored value {} is not registered ({})",
        T::id(),
        std::any::type_name::<T>()
    )
}

/// The viewport's views and cameras.
pub trait HostView {
    /// The views that show, by index: all four, or the active one alone.
    fn shown_views(&self) -> std::ops::Range<usize>;
    /// The active view's index.
    fn active_view(&self) -> usize;
    /// View `index`'s camera; `None` for no such view.
    fn camera(&self, index: usize) -> Option<ViewCamera>;
    /// Frames the active view on `bounds` (in the edited object's space),
    /// as F frames the selection.
    fn frame_bounds(&mut self, bounds: Bounds);
    /// Asks for one more frame (a package's own state changed outside an
    /// event: a worker's result it just adopted). Never a timer.
    fn redraw(&mut self);
    /// Whether view `view` renders now (an engine draws it, not the
    /// viewport's shading): a package keeps render-only work (a preview
    /// copy of a drag) for while one does. `false` for no such view.
    fn rendering(&self, view: usize) -> bool;
    /// Display option `id`'s value in view `view`: as the user set it
    /// (that view's, or the context's for a
    /// [`Context`](crate::package::DisplayScope::Context) option), else its
    /// declaration's default ([`DisplayOption`](crate::package::DisplayOption)):
    /// a toggle's 1 or 0, a choice's index. 0 for an option the open
    /// context doesn't declare, or no such view. Any call reads it: a
    /// view's (its own, `ViewCx::view().index`), and `settle`, which
    /// works out before any view draws what the views show.
    fn displayed(&self, view: usize, id: &str) -> usize;
}

/// The status bar, the error line and the Console.
pub trait HostStatus {
    /// Reports `diagnostic` as the calling package's: a Console record of
    /// its severity, about `subject` inside the open object when it names
    /// one (its own spelling: a node path, an element); an error also
    /// shows on the error line.
    fn report(&mut self, diagnostic: Diagnostic, subject: Option<&str>);
}

/// The shell around the views: actions, menus and the keyboard.
pub trait HostShell {
    /// Runs item `item` of vocabulary `vocabulary` (a menu entry's action),
    /// after the current call returns.
    fn run_action(&mut self, vocabulary: &str, item: &str);
    /// Whether something other than the panes has the keyboard (a text
    /// field, a menu, a dialog, a drag in the views): keys are not
    /// commands now.
    fn keyboard_taken(&self) -> bool;
    /// Puts `text` on the system clipboard (a copied value or selection).
    fn copy_text(&mut self, text: &str);
    /// The system clipboard's text, when it holds text.
    fn paste_text(&self) -> Option<String>;
}

/// The viewport's transform tool and snapping.
pub trait HostTransform {
    /// The transform mode (Q, W, E, R, T).
    fn transform_tool(&self) -> TransformTool;
    /// Chooses the transform mode.
    fn set_transform_tool(&mut self, tool: TransformTool);
    /// Whether snapping is on.
    fn snapping(&self) -> bool;
    /// Turns snapping on or off (saved with the user's settings).
    fn set_snapping(&mut self, on: bool);
    /// The space the transform tool works in (each tool's own).
    fn transform_space(&self) -> TransformSpace;
    /// Chooses the current tool's space (saved with the user's settings).
    fn set_transform_space(&mut self, space: TransformSpace);
    /// The transform pivot, in the open context's space: the user's choice
    /// (kept per context, starting at
    /// [`ViewportSpec::default_pivot`](crate::package::ViewportSpec::default_pivot))
    /// resolved against the frame the context's
    /// [`Levels::pivot`](crate::package::Levels::pivot) last gave; `None`
    /// while that gives none. A live tool reads it to turn or scale about
    /// it, and again when told the choice changed
    /// ([`Levels::pivot_changed`](crate::package::Levels::pivot_changed)).
    fn pivot(&self) -> Option<Vec3>;
}

/// The tools' memory and the tool a card still adjusts, the same for every
/// context's tools (the application's, saved with the user's settings).
pub trait HostTools {
    /// The parameters node type `ty`'s tool starts from: its declared
    /// defaults with the last values of the parameters its library item
    /// remembers ([`VocabularyItem::remembered`](crate::package::VocabularyItem::remembered))
    /// written in.
    fn starting(&self, ty: NodeTypeId) -> Parameters;
    /// Remembers `parameters`' remembered values for `ty` (the others are
    /// left out).
    fn remember(&mut self, ty: NodeTypeId, parameters: &Parameters);
    /// The node the last tool run added or changed, while its card may
    /// still adjust it: until any other edit, an undo or a change of place.
    fn adjustable(&self) -> Option<NodeRef>;
    /// A tool just ran on `node`: it becomes the adjustable one.
    fn ran(&mut self, node: NodeRef);
}
