//! References to what the host's document holds: scene objects and nodes,
//! by the host's ids for them.

pub use kinogaki_value::ObjectRef;

/// A node of the open network, by the host's id for it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct NodeRef(pub u64);
