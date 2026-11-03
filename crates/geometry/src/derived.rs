//! [`Derived`]: the typed cache slots of one shared part of a value (a
//! column, a topology). See the crate's cache rule.
use std::any::{Any, TypeId};
use std::fmt;
use std::sync::{Arc, Mutex, OnceLock};

/// A slot per cache type, keyed by the type's `TypeId`: each an
/// `OnceLock<Arc<D>>`, so a `D` is built once however many threads ask. It
/// lives exactly as long as the part that holds it.
#[derive(Default)]
pub(crate) struct Derived {
    slots: Mutex<Vec<(TypeId, Arc<dyn Any + Send + Sync>)>>,
}

impl Derived {
    /// The `D` kept here, built by `build` the first time it is asked for.
    /// `build` runs outside the slots' lock, so it may itself ask this
    /// part for another cache (never for a `D`); a thread asking while
    /// another builds waits for that build, so each `D` is built once.
    pub(crate) fn get_or_build<D: Send + Sync + 'static>(
        &self,
        build: impl FnOnce() -> D,
    ) -> Arc<D> {
        let slot = {
            let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
            let id = TypeId::of::<D>();
            match slots.iter().find(|(t, _)| *t == id) {
                Some((_, slot)) => slot.clone(),
                None => {
                    let slot: Arc<dyn Any + Send + Sync> = Arc::new(OnceLock::<Arc<D>>::new());
                    slots.push((id, slot.clone()));
                    slot
                }
            }
        };
        let slot = typed::<D>(&slot);
        slot.get_or_init(|| Arc::new(build())).clone()
    }
}

/// A slot as the `OnceLock` of its own type.
fn typed<D: Send + Sync + 'static>(slot: &Arc<dyn Any + Send + Sync>) -> &OnceLock<Arc<D>> {
    slot.downcast_ref().expect("a slot holds its own type")
}

impl fmt::Debug for Derived {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.slots.lock().map_or(0, |s| s.len());
        write!(f, "Derived({count} slots)")
    }
}
