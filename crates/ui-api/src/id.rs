//! [`Id`]: what keeps a widget's state (focus, an open menu, a drag)
//! apart from every other's.
use std::hash::{DefaultHasher, Hash, Hasher};

/// A widget's identity, kept by the host from frame to frame.
///
/// Make one from anything hashable that names the widget for as long as
/// it lives (`Id::new("mesh-check")`, `parent.with(("row", key))`), not
/// from its place on screen or its index in a list that can shift. Ids
/// hash with the standard library's [`DefaultHasher`]: equal within a
/// build of Studio, which is all the host keeps them for.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub u64);

impl Id {
    /// The id of `source`.
    pub fn new(source: impl Hash) -> Self {
        let mut hasher = DefaultHasher::new();
        source.hash(&mut hasher);
        Id(hasher.finish())
    }

    /// The id of `child` under this one.
    pub fn with(self, child: impl Hash) -> Self {
        Id::new((self.0, child))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_stable_and_children_differ() {
        assert_eq!(Id::new("card"), Id::new("card"));
        assert_ne!(Id::new("card"), Id::new("cards"));
        let parent = Id::new("sheet");
        assert_eq!(parent.with(3), parent.with(3));
        assert_ne!(parent.with(3), parent.with(4));
        assert_ne!(parent.with(3), Id::new("other").with(3));
    }
}
