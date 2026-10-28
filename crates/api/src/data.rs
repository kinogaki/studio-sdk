//! Values on wires and in object results: [`Data`], a type-erased shared
//! value tagged with its data type, and [`Typed`], a kernel's typed handle
//! for wrapping and reading its own values.
//!
//! The environment never looks inside a [`Data`]: it reaches it only
//! through the [`DataType`](crate::DataType) registered for its id. A
//! kernel and the node packages over it read the value directly, through
//! the kernel's [`Typed`] handle. Downcasts work across packages because
//! the whole program holds one copy of each kernel crate, so a kernel
//! type has one `TypeId` everywhere.
use crate::id::DataTypeId;
use std::any::Any;
use std::fmt;
use std::marker::PhantomData;
use std::sync::Arc;

/// A shared, immutable value of a registered data type.
///
/// Cloning shares the value. Two `Data` that [`ptr_eq`](Self::ptr_eq) are
/// the same value: caches key on that identity (a node whose inputs are
/// the same allocations needn't cook again), so a kernel makes a new
/// `Data` only for a new value.
#[derive(Clone)]
pub struct Data {
    ty: DataTypeId,
    value: Arc<dyn Any + Send + Sync>,
}

impl Data {
    /// `value` as data of type `ty`. Prefer the kernel's
    /// [`Typed::wrap`], which can't pair a value with the wrong id.
    pub fn new<T: Any + Send + Sync>(ty: DataTypeId, value: T) -> Self {
        Self::from_arc(ty, Arc::new(value))
    }

    /// A shared `value` as data of type `ty`, without copying it.
    pub fn from_arc<T: Any + Send + Sync>(ty: DataTypeId, value: Arc<T>) -> Self {
        Self { ty, value }
    }

    /// Its data type's id.
    pub fn ty(&self) -> DataTypeId {
        self.ty
    }

    /// The value as a `T`, when it is one (whatever its id says; kernels
    /// read through [`Typed::get`], which checks the id too).
    pub fn downcast_ref<T: Any>(&self) -> Option<&T> {
        self.value.downcast_ref()
    }

    /// The shared value as an `Arc<T>`, when it is a `T`.
    pub fn downcast_arc<T: Any + Send + Sync>(&self) -> Option<Arc<T>> {
        self.value.clone().downcast().ok()
    }

    /// Whether `self` and `other` share one allocation: the same value,
    /// for certain, without comparing contents.
    pub fn ptr_eq(&self, other: &Data) -> bool {
        Arc::ptr_eq(&self.value, &other.value)
    }

    /// The allocation's address, for keying caches by identity. Equal
    /// keys of two live `Data` mean [`ptr_eq`](Self::ptr_eq).
    pub fn identity(&self) -> usize {
        Arc::as_ptr(&self.value) as *const () as usize
    }
}

impl fmt::Debug for Data {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Data({} @ {:#x})", self.ty, self.identity())
    }
}

/// A kernel's handle on one of its data types: the id paired with the Rust
/// type its values are, so wrapping and reading can't mix them up.
///
/// ```
/// use kinogaki_studio_api::{DataTypeId, Typed};
///
/// struct Points(Vec<[f64; 3]>);
/// let points = Typed::<Points>::new(DataTypeId::declared("acme.points.cloud"));
/// let data = points.wrap(Points(vec![[0.0; 3]]));
/// assert_eq!(points.get(&data).map(|p| p.0.len()), Some(1));
/// ```
pub struct Typed<T> {
    id: DataTypeId,
    // `fn() -> T`: the handle holds no `T`, so it is `Send`, `Sync` and
    // `Copy` whatever `T` is.
    marker: PhantomData<fn() -> T>,
}

impl<T> Clone for Typed<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for Typed<T> {}

impl<T> fmt::Debug for Typed<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Typed<{}>({})", std::any::type_name::<T>(), self.id)
    }
}

impl<T: Any + Send + Sync> Typed<T> {
    /// The handle for data type `id`, whose values are `T`s.
    pub fn new(id: DataTypeId) -> Self {
        Self {
            id,
            marker: PhantomData,
        }
    }

    /// The data type's id.
    pub fn id(self) -> DataTypeId {
        self.id
    }

    /// `value` as [`Data`] of this type.
    pub fn wrap(self, value: T) -> Data {
        Data::new(self.id, value)
    }

    /// A shared `value` as [`Data`] of this type, without copying it.
    pub fn wrap_arc(self, value: Arc<T>) -> Data {
        Data::from_arc(self.id, value)
    }

    /// Whether `data` is of this type.
    pub fn is(self, data: &Data) -> bool {
        data.ty() == self.id && data.downcast_ref::<T>().is_some()
    }

    /// `data`'s value, when it is of this type (its id and its Rust type).
    pub fn get(self, data: &Data) -> Option<&T> {
        (data.ty() == self.id)
            .then(|| data.downcast_ref())
            .flatten()
    }

    /// `data`'s shared value, when it is of this type.
    pub fn get_arc(self, data: &Data) -> Option<Arc<T>> {
        (data.ty() == self.id)
            .then(|| data.downcast_arc())
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, PartialEq)]
    struct Cloud(Vec<f64>);

    fn cloud() -> Typed<Cloud> {
        Typed::new(DataTypeId::declared("acme.points.cloud"))
    }

    #[test]
    fn wrapped_values_read_back() {
        let data = cloud().wrap(Cloud(vec![1.0, 2.0]));
        assert_eq!(data.ty(), cloud().id());
        assert_eq!(cloud().get(&data), Some(&Cloud(vec![1.0, 2.0])));
        assert!(cloud().is(&data));
        let shared = cloud().get_arc(&data).unwrap();
        assert!(std::ptr::eq(&*shared, cloud().get(&data).unwrap()));
    }

    #[test]
    fn another_type_reads_as_none() {
        let data = cloud().wrap(Cloud(vec![]));
        // The same Rust type under another id.
        let other = Typed::<Cloud>::new(DataTypeId::declared("acme.points.other"));
        assert!(other.get(&data).is_none() && !other.is(&data));
        // The same id over another Rust type.
        let wrong = Typed::<String>::new(cloud().id());
        assert!(wrong.get(&data).is_none() && wrong.get_arc(&data).is_none());
        assert!(data.downcast_ref::<String>().is_none());
    }

    #[test]
    fn clones_share_the_value() {
        let data = cloud().wrap(Cloud(vec![1.0]));
        let clone = data.clone();
        assert!(data.ptr_eq(&clone) && data.identity() == clone.identity());
        let equal_but_new = cloud().wrap(Cloud(vec![1.0]));
        assert!(!data.ptr_eq(&equal_but_new));
        let arc = Arc::new(Cloud(vec![]));
        let a = cloud().wrap_arc(arc.clone());
        let b = cloud().wrap_arc(arc);
        assert!(a.ptr_eq(&b));
    }
}
