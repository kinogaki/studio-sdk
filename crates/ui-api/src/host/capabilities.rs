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
use crate::gizmo::TransformTool;
use kinogaki_studio_api::session::Slot;
use kinogaki_studio_api::stored::Stored;
use kinogaki_studio_api::{Bounds, ContextId, Data, Drawn, ObjectBody, ObjectKindId, SlotId};
use std::any::Any;
use std::sync::Arc;

/// Undo, redo and gestures.
///
/// An edit through the host ([`HostSlotsExt::set_slot`], a selection) is one
/// undo step of its own, unless a gesture is open: then every change up
/// to [`finish_gesture`](HostUndo::finish_gesture) is one step, named
/// when the gesture began (a drag).
pub trait HostUndo {
    /// Opens a gesture named `label`; a gesture already open is kept.
    fn begin_gesture(&mut self, label: &str);
    /// Closes the open gesture as one step (nothing when nothing changed).
    fn finish_gesture(&mut self);
    /// Puts back everything the open gesture changed, and records nothing.
    fn cancel_gesture(&mut self);
    /// Whether a gesture is open.
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
    /// Slot `id`'s value to change in place, within whatever step is open
    /// (none: the change rides along with the next step's snapshot). The
    /// host copies a value a snapshot still shares first.
    fn slot_any_mut(&mut self, id: SlotId) -> Option<&mut (dyn Any + Send + Sync)>;
    /// Sets slot `id` to `value` as an undo step named `label` (inside the
    /// open gesture, if any; nothing is recorded when nothing changed).
    /// `false`: no such slot, or `value` is not of its type.
    fn set_slot_any(&mut self, id: SlotId, label: &str, value: Arc<dyn Any + Send + Sync>) -> bool;
}

/// Typed access to the session's slots.
pub trait HostSlotsExt: HostSlots {
    /// Slot `T`'s value.
    ///
    /// # Panics
    /// When no package registered `T` (a programming error: a package
    /// registers its slots in [`UiPackage::slots`](crate::package::UiPackage::slots)).
    fn slot<T: Slot>(&self) -> &T {
        self.slot_any(T::id())
            .and_then(|v| v.downcast_ref())
            .unwrap_or_else(|| unregistered::<T>())
    }
    /// Slot `T`'s value to change, not recorded (see
    /// [`HostSlots::slot_any_mut`]).
    ///
    /// # Panics
    /// When no package registered `T`.
    fn slot_mut<T: Slot>(&mut self) -> &mut T {
        self.slot_any_mut(T::id())
            .and_then(|v| v.downcast_mut())
            .unwrap_or_else(|| unregistered::<T>())
    }
    /// Sets slot `T` to `value` as an undo step named `label`.
    ///
    /// # Panics
    /// When no package registered `T`.
    fn set_slot<T: Slot>(&mut self, label: &str, value: T) {
        if !self.set_slot_any(T::id(), label, Arc::new(value)) {
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

/// What is selected: objects at an object level, nodes in the open
/// network. Component selections are packages' own slots.
pub trait HostSelection {
    /// The objects selected at the object levels.
    fn selected_objects(&self) -> Vec<ObjectRef>;
    /// Selects `objects` (only those), as a click does.
    fn select_objects(&mut self, objects: &[ObjectRef]);
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
    /// Shows `place` (navigation, not an undo step); `Err` says why it
    /// can't (the object is locked or gone).
    fn go_to(&mut self, place: Place) -> Result<(), String>;
    /// Bumped by every change to a network: what a cook would see.
    fn revision(&self) -> u64;
    /// Object `object`'s name.
    fn object_name(&self, object: ObjectRef) -> Option<String>;
    /// Object `object`'s kind.
    fn object_kind(&self, object: ObjectRef) -> Option<ObjectKindId>;
    /// Object `object`'s package-owned body, when its kind is a
    /// package's (read it with `downcast_ref`).
    fn object_body(&self, object: ObjectRef) -> Option<Arc<dyn ObjectBody>>;
    /// Every object of kind `kind`, in the scene's order.
    fn objects_of_kind(&self, kind: ObjectKindId) -> Vec<ObjectRef>;
    /// Stored value `id` (a package's [`Stored`]): the project's (`view`
    /// `None`), or view `view`'s. `None` when no package registered it in
    /// that scope.
    fn stored_any(&self, id: &str, view: Option<usize>) -> Option<&(dyn Any + Send + Sync)>;
    /// Sets stored value `id` (the project's, or view `view`'s) to
    /// `value`: a project's value is a change to the project, saved with
    /// it; a view's is not undone. `false`: no such value, or `value` is
    /// not of its type.
    fn set_stored_any(
        &mut self,
        id: &str,
        view: Option<usize>,
        value: Arc<dyn Any + Send + Sync>,
    ) -> bool;
    /// Object `object`'s shown result: the value its network yields (the
    /// open object: what it displays) and its draw pieces, as the host
    /// last published them.
    fn shown(&self, object: ObjectRef) -> Option<(Data, Drawn)>;
    /// Whether the node the open network displays is a held edit (an Edit
    /// Mesh): the library's entries for editing show, and a key store
    /// active while editing applies.
    fn editing(&self) -> bool;
}

/// Typed access to the stored values.
pub trait HostDocExt: HostDoc {
    /// The project's value `T`.
    ///
    /// # Panics
    /// When no package registered `T` for the project (a programming
    /// error: a package registers its values with
    /// [`Package::with_stored`](kinogaki_studio_api::Package::with_stored)).
    fn stored<T: Stored>(&self) -> &T {
        self.stored_any(T::id(), None)
            .and_then(|v| v.downcast_ref())
            .unwrap_or_else(|| unstored::<T>())
    }
    /// View `view`'s value `T`.
    ///
    /// # Panics
    /// When no package registered `T` for views, or there is no such view.
    fn view_stored<T: Stored>(&self, view: usize) -> &T {
        self.stored_any(T::id(), Some(view))
            .and_then(|v| v.downcast_ref())
            .unwrap_or_else(|| unstored::<T>())
    }
    /// Sets the project's value `T`.
    ///
    /// # Panics
    /// When no package registered `T` for the project.
    fn set_stored<T: Stored>(&mut self, value: T) {
        if !self.set_stored_any(T::id(), None, Arc::new(value)) {
            unstored::<T>()
        }
    }
    /// Sets view `view`'s value `T`.
    ///
    /// # Panics
    /// When no package registered `T` for views, or there is no such view.
    fn set_view_stored<T: Stored>(&mut self, view: usize, value: T) {
        if !self.set_stored_any(T::id(), Some(view), Arc::new(value)) {
            unstored::<T>()
        }
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
    /// How many views show (1 or 4).
    fn view_count(&self) -> usize;
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
}

/// The status bar, the error line and the Console.
pub trait HostStatus {
    /// Shows `message` on the error line and logs it to the Console as an
    /// error of the calling package.
    fn fail(&mut self, message: &str);
    /// Logs `message` to the Console as information.
    fn info(&mut self, message: &str);
}

/// The shell around the views: actions, menus and the keyboard.
pub trait HostShell {
    /// Runs item `item` of vocabulary `vocabulary` (a menu entry's action),
    /// after the current call returns.
    fn run_action(&mut self, vocabulary: &str, item: &str);
    /// Whether a text field has the keyboard (keys are not commands now).
    fn keyboard_taken(&self) -> bool;
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
}

/// The user's settings sections of the installed packages (a package's
/// own [`SettingsSection`](crate::package::SettingsSection)s), as applied.
/// Untyped here; packages use [`HostSettingsExt`].
pub trait HostSettings {
    /// The applied value of the section of context `context`.
    fn section_any(&self, context: ContextId) -> Option<&(dyn Any + Send + Sync)>;
    /// Applies `value` to that section and saves the settings (a toggle a
    /// package offers outside the Settings window). `false`: no such
    /// section, or `value` is not of its type.
    fn set_section_any(&mut self, context: ContextId, value: Arc<dyn Any + Send + Sync>) -> bool;
}

/// Typed access to the settings sections.
pub trait HostSettingsExt: HostSettings {
    /// The applied value of `context`'s section, as a `T`.
    fn section<T: Any>(&self, context: ContextId) -> Option<&T> {
        self.section_any(context)?.downcast_ref()
    }
}

impl<H: HostSettings + ?Sized> HostSettingsExt for H {}
