//! Component selections: which points, edges, faces or vertices of one
//! value are selected, in the data type's own terms.
//!
//! The environment keeps, undoes, shows and counts a selection without
//! knowing what it means: a polygon mesh selects by element index, a CAD
//! body by persistent topology names that survive a recook. Only the data
//! type that made a [`Components`] reads its inside (through
//! [`Components::get`]); the environment asks the type's
//! [`Displayable`](crate::Displayable) to draw it and its
//! [`Pickable`](crate::Pickable) to make one from a pick.
use crate::id::DataTypeId;
use crate::viewport::ElementKind;
use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// An immutable component selection of one data type at one level.
///
/// Cloning shares it. A change makes a new `Components`, so undo keeps the
/// old one as it was and [`ptr_eq`](Self::ptr_eq) tells whether a
/// selection changed without comparing its members.
#[derive(Clone)]
pub struct Components {
    ty: DataTypeId,
    level: ElementKind,
    count: usize,
    set: Arc<dyn Any + Send + Sync>,
}

impl Components {
    /// `set`, a selection of `count` components at `level` of data of type
    /// `ty`, in that type's own representation.
    pub fn new<T: Any + Send + Sync>(
        ty: DataTypeId,
        level: ElementKind,
        count: usize,
        set: T,
    ) -> Self {
        Self {
            ty,
            level,
            count,
            set: Arc::new(set),
        }
    }

    /// The data type whose components these are.
    pub fn ty(&self) -> DataTypeId {
        self.ty
    }

    /// What is selected: points, edges, faces, vertices or whole objects.
    pub fn level(&self) -> ElementKind {
        self.level
    }

    /// How many components are selected (what the status bar shows).
    pub fn len(&self) -> usize {
        self.count
    }

    /// Whether nothing is selected.
    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// The selection in its type's representation, when it is a `T`.
    pub fn get<T: Any>(&self) -> Option<&T> {
        self.set.downcast_ref()
    }

    /// Whether `self` and `other` are the same selection value.
    pub fn ptr_eq(&self, other: &Components) -> bool {
        Arc::ptr_eq(&self.set, &other.set)
    }
}

impl fmt::Debug for Components {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "Components({}, {:?}, {} selected)",
            self.ty, self.level, self.count
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_selection_reads_back_only_as_its_own_representation() {
        let ty = DataTypeId::declared("acme.points.cloud");
        let picked = Components::new(ty, ElementKind::Point, 2, vec![3u32, 7]);
        assert_eq!(picked.get::<Vec<u32>>(), Some(&vec![3, 7]));
        assert!(picked.get::<Vec<u64>>().is_none());
        assert_eq!(
            (picked.ty(), picked.level(), picked.len()),
            (ty, ElementKind::Point, 2)
        );
        assert!(!picked.is_empty() && picked.ptr_eq(&picked.clone()));
        let none = Components::new(ty, ElementKind::Point, 0, Vec::<u32>::new());
        assert!(none.is_empty() && !none.ptr_eq(&picked));
    }
}
