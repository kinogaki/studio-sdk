//! References to what the host's document holds: scene objects and nodes,
//! by the host's ids for them.

/// A scene object, by the host's id for it. Valid while the object
/// exists; the host answers `None` for one that is gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectRef(pub u64);

/// A node of the open network, by the host's id for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeRef(pub u64);
