//! Session slots: package-owned state of an editing session.
//!
//! What the user has selected in a Mesh network, CAD's picks, an open
//! sketch: state a package keeps per session, outside the project file.
//! A package declares each slot as a value type implementing [`Slot`] and
//! registers it ([`SessionSlot::of`], [`Package::with_slot`]); the
//! environment holds every registered slot's value in one
//! [`SessionSlots`] table and undoes the undoable ones with the scene.
//!
//! Values are shared and copy-on-write, as object bodies are: an undo
//! snapshot is a clone of the table (one pointer a value), and changing a
//! value copies it only while a snapshot still shares it.
//!
//! [`Package::with_slot`]: crate::Package::with_slot
use crate::id::{ContextId, SlotId};
use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

/// A slot's value type, naming its slot.
///
/// The environment calls these on its UI thread: `initial` when a session
/// starts (or the package is installed into a running one), `restored`
/// after every undo, redo, cancelled gesture and edit, `estimated_bytes`
/// when it budgets the undo history. None may block.
pub trait Slot: Any + Clone + PartialEq + fmt::Debug + Send + Sync {
    /// The slot's id (`kinogaki.mesh.selection`), under its package's
    /// vendor.
    fn id() -> SlotId;

    /// The value a new session starts with.
    fn initial() -> Self;

    /// Whether undo snapshots keep it, so undo, redo and a cancelled
    /// gesture restore it with the document (a selection). A slot that
    /// isn't (a memory of the session, such as the selection each object
    /// had when it was left) keeps its value through them.
    const UNDOABLE: bool = true;

    /// Whether the value belongs to the level it was made at: showing
    /// another level (an object level, or another object) starts it over
    /// at [`Slot::initial`].
    const PLACE_BOUND: bool = false;

    /// The value in line with `doc` after an undo, a redo or an edit;
    /// `None` when this one already is. It reads only what `doc` offers
    /// and never changes anything else.
    fn restored(&self, doc: &dyn SlotDoc) -> Option<Self> {
        let _ = doc;
        None
    }

    /// Roughly how much memory the value holds, for the undo history's
    /// budget.
    fn estimated_bytes(&self) -> usize {
        0
    }
}

/// What a slot's [`restored`](Slot::restored) hook may read: where the
/// user is, as restored.
pub trait SlotDoc {
    /// The open level's context (the scene's for an object level).
    fn context(&self) -> ContextId;

    /// Whether the open level is a mode on one of its network's nodes
    /// (CAD's sketch mode on a Sketch node).
    fn in_mode(&self) -> bool;
}

/// A shared slot value.
type Value = Arc<dyn Any + Send + Sync>;

/// A registered slot: its id and flags, and what the environment does
/// with a value of its type without knowing the type. Made from the
/// value type with [`SessionSlot::of`]; cheap to copy.
#[derive(Clone, Copy)]
pub struct SessionSlot {
    id: SlotId,
    undoable: bool,
    place_bound: bool,
    type_name: &'static str,
    initial: fn() -> Value,
    copy: fn(&dyn Any) -> Value,
    eq: fn(&dyn Any, &dyn Any) -> bool,
    debug: fn(&dyn Any, &mut fmt::Formatter<'_>) -> fmt::Result,
    restored: fn(&dyn Any, &dyn SlotDoc) -> Option<Value>,
    bytes: fn(&dyn Any) -> usize,
}

/// `v` as a `T`; every value in a slot is of its slot's type.
fn typed<T: Slot>(v: &dyn Any) -> &T {
    v.downcast_ref::<T>()
        .expect("a slot value of its slot's type")
}

impl SessionSlot {
    /// The slot `T` names.
    pub fn of<T: Slot>() -> Self {
        fn initial<T: Slot>() -> Value {
            Arc::new(T::initial())
        }
        fn copy<T: Slot>(v: &dyn Any) -> Value {
            Arc::new(typed::<T>(v).clone())
        }
        fn eq<T: Slot>(a: &dyn Any, b: &dyn Any) -> bool {
            a.downcast_ref::<T>() == b.downcast_ref::<T>()
        }
        fn debug<T: Slot>(v: &dyn Any, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            fmt::Debug::fmt(typed::<T>(v), f)
        }
        fn restored<T: Slot>(v: &dyn Any, doc: &dyn SlotDoc) -> Option<Value> {
            typed::<T>(v).restored(doc).map(|r| Arc::new(r) as Value)
        }
        fn bytes<T: Slot>(v: &dyn Any) -> usize {
            typed::<T>(v).estimated_bytes()
        }
        Self {
            id: T::id(),
            undoable: T::UNDOABLE,
            place_bound: T::PLACE_BOUND,
            type_name: std::any::type_name::<T>(),
            initial: initial::<T>,
            copy: copy::<T>,
            eq: eq::<T>,
            debug: debug::<T>,
            restored: restored::<T>,
            bytes: bytes::<T>,
        }
    }

    /// Its id.
    pub fn id(&self) -> SlotId {
        self.id
    }

    /// Whether undo keeps it ([`Slot::UNDOABLE`]).
    pub fn undoable(&self) -> bool {
        self.undoable
    }

    /// Whether a new level starts it over ([`Slot::PLACE_BOUND`]).
    pub fn place_bound(&self) -> bool {
        self.place_bound
    }
}

impl fmt::Debug for SessionSlot {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionSlot")
            .field("id", &self.id)
            .field("type", &self.type_name)
            .field("undoable", &self.undoable)
            .field("place_bound", &self.place_bound)
            .finish()
    }
}

/// One slot's value with its slot.
#[derive(Clone)]
struct Entry {
    slot: SessionSlot,
    value: Value,
}

/// Slot values by slot id. Cloning shares every value (a snapshot); two
/// tables are equal when they hold the same slots and each value is the
/// same one or an equal one.
#[derive(Clone, Default)]
pub struct SessionSlots(BTreeMap<SlotId, Entry>);

impl SessionSlots {
    /// Each of `slots` at its initial value.
    pub fn new(slots: impl IntoIterator<Item = SessionSlot>) -> Self {
        let mut table = Self::default();
        for slot in slots {
            table.install(slot);
        }
        table
    }

    /// Adds `slot` at its initial value (a package installed into a
    /// running session); a slot already held keeps its value.
    pub fn install(&mut self, slot: SessionSlot) {
        self.0.entry(slot.id).or_insert_with(|| Entry {
            slot,
            value: (slot.initial)(),
        });
    }

    /// Takes `other`'s value of every slot both tables hold (shared, not
    /// copied): a table for a new set of packages keeps what the old one
    /// held.
    pub fn take_values(&mut self, other: &SessionSlots) {
        for (id, entry) in &mut self.0 {
            if let Some(kept) = other.0.get(id) {
                entry.value = kept.value.clone();
            }
        }
    }

    /// Whether slot `id` is one of these.
    pub fn contains(&self, id: SlotId) -> bool {
        self.0.contains_key(&id)
    }

    /// The slots held, in id order.
    pub fn slots(&self) -> impl Iterator<Item = SessionSlot> + '_ {
        self.0.values().map(|e| e.slot)
    }

    /// Slot `T`'s value, when it is one of these.
    pub fn get<T: Slot>(&self) -> Option<&T> {
        self.0.get(&T::id())?.value.downcast_ref()
    }

    /// Slot `T`'s value to change, copied first while something else (a
    /// snapshot) shares it; `None` when it isn't one of these.
    pub fn get_mut<T: Slot>(&mut self) -> Option<&mut T> {
        self.get_any_mut(T::id())?.downcast_mut()
    }

    /// Replaces slot `T`'s value; `false` (and nothing stored) when it
    /// isn't one of these.
    pub fn set<T: Slot>(&mut self, value: T) -> bool {
        self.set_any(T::id(), Arc::new(value))
    }

    /// Slot `id`'s value, untyped.
    pub fn get_any(&self, id: SlotId) -> Option<&(dyn Any + Send + Sync)> {
        self.0.get(&id).map(|e| &*e.value)
    }

    /// Slot `id`'s value to change, untyped (copied first while shared).
    pub fn get_any_mut(&mut self, id: SlotId) -> Option<&mut (dyn Any + Send + Sync)> {
        let entry = self.0.get_mut(&id)?;
        if Arc::get_mut(&mut entry.value).is_none() {
            entry.value = (entry.slot.copy)(&*entry.value);
        }
        Arc::get_mut(&mut entry.value)
    }

    /// Replaces slot `id`'s value with `value`; `false` (and nothing
    /// stored) when the slot isn't one of these or `value` is not of its
    /// type.
    pub fn set_any(&mut self, id: SlotId, value: Arc<dyn Any + Send + Sync>) -> bool {
        match self.0.get_mut(&id) {
            Some(entry) if (*entry.value).type_id() == (*value).type_id() => {
                entry.value = value;
                true
            }
            _ => false,
        }
    }

    /// Every place-bound value back at its initial one
    /// ([`Slot::PLACE_BOUND`]): another level shows.
    pub fn reset_place_bound(&mut self) {
        for entry in self.0.values_mut() {
            if entry.slot.place_bound {
                entry.value = (entry.slot.initial)();
            }
        }
    }

    /// Each value in line with `doc` ([`Slot::restored`]).
    pub fn keep(&mut self, doc: &dyn SlotDoc) {
        for entry in self.0.values_mut() {
            if let Some(value) = (entry.slot.restored)(&*entry.value, doc) {
                entry.value = value;
            }
        }
    }

    /// Roughly how much memory the values hold.
    pub fn estimated_bytes(&self) -> usize {
        self.0.values().map(|e| (e.slot.bytes)(&*e.value)).sum()
    }
}

impl PartialEq for SessionSlots {
    fn eq(&self, other: &Self) -> bool {
        self.0.len() == other.0.len()
            && self.0.iter().zip(&other.0).all(|((ia, a), (ib, b))| {
                ia == ib && (Arc::ptr_eq(&a.value, &b.value) || (a.slot.eq)(&*a.value, &*b.value))
            })
    }
}

impl fmt::Debug for SessionSlots {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        struct Shown<'a>(&'a Entry);
        impl fmt::Debug for Shown<'_> {
            fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                (self.0.slot.debug)(&*self.0.value, f)
            }
        }
        f.debug_map()
            .entries(self.0.iter().map(|(id, e)| (id, Shown(e))))
            .finish()
    }
}
