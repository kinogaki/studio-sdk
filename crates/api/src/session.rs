//! [`SessionSlot`]: package-owned state of an editing session.
//!
//! What the user has selected in a Mesh network, CAD's picks, an open
//! sketch: state a package keeps per session, outside the project file.
//! The environment stores each slot's value in [`SessionSlots`] and undoes
//! it with the scene: values are immutable and replaced whole, so an undo
//! step keeps the previous value and costs one shared pointer.
use crate::id::SlotId;
use std::any::Any;
use std::collections::BTreeMap;
use std::fmt;
use std::sync::Arc;

/// A slot a package registers.
pub trait SessionSlot: Send + Sync {
    /// Its id (`kinogaki.mesh.selection`).
    fn id(&self) -> SlotId;

    /// The value a new session starts with.
    fn initial(&self) -> SlotValue;

    /// Whether changes are undo steps (a selection is; a panel's scroll
    /// position would not be).
    fn undoable(&self) -> bool {
        true
    }
}

/// A slot's value: immutable, shared, of the package's own type.
#[derive(Clone)]
pub struct SlotValue(Arc<dyn Any + Send + Sync>);

impl SlotValue {
    /// `value` as a slot value.
    pub fn new<T: Any + Send + Sync>(value: T) -> Self {
        Self(Arc::new(value))
    }

    /// The value, when it is a `T`.
    pub fn get<T: Any>(&self) -> Option<&T> {
        self.0.downcast_ref()
    }

    /// Whether `self` and `other` are the same value.
    pub fn ptr_eq(&self, other: &SlotValue) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl fmt::Debug for SlotValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "SlotValue({:p})", Arc::as_ptr(&self.0))
    }
}

/// Every slot's current value in one session. Cloning it is a snapshot
/// (shared values, no copies).
#[derive(Clone, Debug, Default)]
pub struct SessionSlots {
    values: BTreeMap<SlotId, SlotValue>,
}

impl SessionSlots {
    /// Each of `slots` at its initial value.
    pub fn new<'a>(slots: impl IntoIterator<Item = &'a dyn SessionSlot>) -> Self {
        Self {
            values: slots.into_iter().map(|s| (s.id(), s.initial())).collect(),
        }
    }

    /// Slot `id`'s value; `None` when no package registered it.
    pub fn get(&self, id: SlotId) -> Option<&SlotValue> {
        self.values.get(&id)
    }

    /// Slot `id`'s value as a `T`, when it is one.
    pub fn get_as<T: Any>(&self, id: SlotId) -> Option<&T> {
        self.get(id).and_then(SlotValue::get)
    }

    /// Replaces slot `id`'s value with `value`, returning the previous one
    /// (what an undo step keeps); `None` and nothing stored when no package
    /// registered the slot.
    pub fn set(&mut self, id: SlotId, value: SlotValue) -> Option<SlotValue> {
        self.values
            .get_mut(&id)
            .map(|kept| std::mem::replace(kept, value))
    }
}
