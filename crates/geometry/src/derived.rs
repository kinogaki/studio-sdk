//! [`Derived`]: the typed cache slots of one shared part of a value (a
//! column, a topology). See the crate's cache rule.
use std::any::{Any, TypeId};
use std::fmt;
use std::sync::{Arc, Mutex};

/// A slot per cache type, keyed by the type's `TypeId`. It lives exactly as
/// long as the part that holds it.
#[derive(Default)]
pub(crate) struct Derived {
    slots: Mutex<Vec<(TypeId, Arc<dyn Any + Send + Sync>)>>,
}

impl Derived {
    /// The `D` kept here, built by `build` the first time it is asked for.
    /// `build` runs outside the lock, so it may itself ask this part for
    /// another cache; when two threads build at once, the first to finish
    /// is kept and the other's value is dropped.
    pub(crate) fn get_or_build<D: Send + Sync + 'static>(
        &self,
        build: impl FnOnce() -> D,
    ) -> Arc<D> {
        if let Some(kept) = self.get::<D>() {
            return kept;
        }
        let built: Arc<dyn Any + Send + Sync> = Arc::new(build());
        let mut slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        let id = TypeId::of::<D>();
        let kept = match slots.iter().find(|(t, _)| *t == id) {
            Some((_, first)) => first.clone(),
            None => {
                slots.push((id, built.clone()));
                built
            }
        };
        kept.downcast().expect("a slot holds its own type")
    }

    /// The `D` kept here, if built.
    pub(crate) fn get<D: Send + Sync + 'static>(&self) -> Option<Arc<D>> {
        let slots = self.slots.lock().unwrap_or_else(|e| e.into_inner());
        let id = TypeId::of::<D>();
        let (_, kept) = slots.iter().find(|(t, _)| *t == id)?;
        Some(kept.clone().downcast().expect("a slot holds its own type"))
    }
}

impl fmt::Debug for Derived {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let count = self.slots.lock().map_or(0, |s| s.len());
        write!(f, "Derived({count} slots)")
    }
}
