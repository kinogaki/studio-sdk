//! Persistent names ([`Name`]): data any kernel makes, compares and saves.
//!
//! What an entity is to the operation that made it is the kernel's own
//! tag, opaque to everything else; which names answer a reference after
//! splits and merges, and what an operation did to its input's names, are
//! a kernel's policy, not part of the value.
use std::fmt;
use std::sync::Arc;

/// The operation that made an entity: a node or Edit CAD step id, assigned
/// by the host. [`OperationId::NONE`] is geometry made outside any
/// operation (tests, tools, an import's own entities).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct OperationId(pub u64);

impl OperationId {
    /// No operation.
    pub const NONE: OperationId = OperationId(0);
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Node {
    Created { by: OperationId, tag: Arc<str> },
    Part { of: Name, index: u32 },
    Merged { of: Arc<[Name]> },
}

/// A persistent name: who made an entity and the kernel's tag for it, or which piece
/// of a split entity it is, or which entities it merges. Cheap to clone;
/// compared and ordered structurally (by node, then fields, in the order
/// they are declared).
///
/// ```
/// use kinogaki_geometry::brep::{Name, NameNode, OperationId};
///
/// let face = Name::created(OperationId(7), "box.face.2");
/// let piece = Name::part(&face, 1);
/// let a = Name::merged(&[piece.clone(), face.clone()]);
/// assert_eq!(a, Name::merged(&[face.clone(), piece.clone(), face]));
/// assert!(matches!(piece.node(), NameNode::Part { index: 1, .. }));
/// ```
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Name(Arc<Node>);

/// A name's top node, read structurally: what [`Name::created`],
/// [`Name::part`] and [`Name::merged`] made it from.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum NameNode<'a> {
    /// Made by an operation.
    Created {
        /// The operation.
        by: OperationId,
        /// What the entity is to it, in the kernel's own terms.
        tag: &'a str,
    },
    /// Piece `index` of a split entity.
    Part {
        /// The entity split.
        of: &'a Name,
        /// Which piece.
        index: u32,
    },
    /// The entity standing for several merged ones.
    Merged {
        /// The merged names, sorted and without repeats.
        of: &'a [Name],
    },
}

impl Name {
    /// An entity made by `by`, `tag` saying what it is to it (text opaque
    /// to all but the kernel that made it: a primitive's face index, a
    /// sweep's cap, an edge between two named faces).
    pub fn created(by: OperationId, tag: impl Into<Arc<str>>) -> Self {
        Self(Arc::new(Node::Created {
            by,
            tag: tag.into(),
        }))
    }

    /// Piece `index` of the split entity `of`.
    pub fn part(of: &Name, index: u32) -> Self {
        Self(Arc::new(Node::Part {
            of: of.clone(),
            index,
        }))
    }

    /// The entity standing for the merged `of`: sorted and without
    /// repeats, so their order doesn't matter.
    pub fn merged(of: &[Name]) -> Self {
        let mut of = of.to_vec();
        of.sort();
        of.dedup();
        Self(Arc::new(Node::Merged { of: of.into() }))
    }

    /// The top node.
    pub fn node(&self) -> NameNode<'_> {
        match self.0.as_ref() {
            Node::Created { by, tag } => NameNode::Created { by: *by, tag },
            Node::Part { of, index } => NameNode::Part { of, index: *index },
            Node::Merged { of } => NameNode::Merged { of },
        }
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.as_ref() {
            Node::Created { by, tag } => write!(f, "Name({}, {tag:?})", by.0),
            Node::Part { of, index } => write!(f, "Part({of:?}, {index})"),
            Node::Merged { of } => f.debug_tuple("Merged").field(&of).finish(),
        }
    }
}
