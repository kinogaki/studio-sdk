//! Persistent names ([`Name`]) and an operation's record of what it did
//! to them ([`History`]): data any kernel makes, compares and saves.
//!
//! Which names answer a reference after splits and merges is a kernel's
//! policy, not part of the value.
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

/// Which end of a sweep a cap is.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum End {
    /// Where the sweep starts.
    Start,
    /// Where it ends.
    End,
}

/// What an entity is to the operation that made it.
///
/// New roles may be added. A name with a role a reader doesn't know stays
/// a valid name: it compares, orders and saves as any other.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum Role {
    /// The `index`-th face, edge or vertex an operation made, in creation
    /// order, when nothing more specific applies.
    Local {
        /// Its creation index.
        index: u32,
    },
    /// A primitive's face, edge or vertex by its fixed index.
    Primitive {
        /// The primitive's own index for it.
        index: u32,
    },
    /// An entity read from a file: entity `index` (file order) of the
    /// file's `body`-th body (file order).
    Imported {
        /// The body's index in the file.
        body: u32,
        /// The entity's index in the body, in file order.
        index: u32,
    },
    /// The file's `body`-th body itself.
    ImportedBody {
        /// The body's index in the file.
        body: u32,
    },
    /// A sweep's cap at one end, from the profile keyed `profile` (text
    /// opaque to all but the kernel that made it).
    Cap {
        /// The profile's key.
        profile: Arc<str>,
        /// Which end.
        end: End,
    },
    /// A sweep's side from source curve `curve` of the profile keyed
    /// `profile`; `segment` tells apart the pieces of one source curve.
    Side {
        /// The profile's key.
        profile: Arc<str>,
        /// The source curve's index in the profile.
        curve: u32,
        /// The piece of that curve, 0 for the first in loop order.
        segment: u32,
    },
    /// Generated along the named entity (a fillet along an edge).
    Along(Name),
    /// Generated at the named vertex (a corner patch).
    At(Name),
    /// Offset from the named entity.
    Offset(Name),
    /// The `index`-th copy of the named entity (a pattern's instance, a
    /// mirror's image).
    Copy {
        /// What it copies.
        of: Name,
        /// Which copy.
        index: u32,
    },
    /// An edge or vertex named by the faces it bounds, and its ordinal
    /// among the entities bounding the same faces.
    Between {
        /// The faces' names.
        faces: Arc<[Name]>,
        /// Its ordinal among entities between the same faces.
        index: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Node {
    Created { by: OperationId, role: Role },
    Part { of: Name, index: u32 },
    Merged { of: Arc<[Name]> },
}

/// A persistent name: who made an entity and in what role, or which piece
/// of a split entity it is, or which entities it merges. Cheap to clone;
/// compared and ordered structurally (by node, then fields, in the order
/// they are declared).
///
/// ```
/// use kinogaki_geometry::brep::{Name, NameNode, OperationId, Role};
///
/// let face = Name::created(OperationId(7), Role::Primitive { index: 2 });
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
    /// Made by an operation in a role.
    Created {
        /// The operation.
        by: OperationId,
        /// The role.
        role: &'a Role,
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
    /// An entity made by `by` in `role`.
    pub fn created(by: OperationId, role: Role) -> Self {
        Self(Arc::new(Node::Created { by, role }))
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
            Node::Created { by, role } => NameNode::Created { by: *by, role },
            Node::Part { of, index } => NameNode::Part { of, index: *index },
            Node::Merged { of } => NameNode::Merged { of },
        }
    }
}

impl fmt::Debug for Name {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0.as_ref() {
            Node::Created { by, role } => write!(f, "Name({}, {role:?})", by.0),
            Node::Part { of, index } => write!(f, "Part({of:?}, {index})"),
            Node::Merged { of } => f.debug_tuple("Merged").field(&of).finish(),
        }
    }
}

/// What the operation that made a value did to the names of its input:
/// generated entities carry new names, modified ones keep theirs, split
/// ones became parts, merged ones one name, and deleted ones are listed so
/// references to them fail naming them. Plain data, in the operation's
/// own order.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct History {
    /// New entities.
    pub generated: Vec<Name>,
    /// Entities changed in place, keeping their names.
    pub modified: Vec<Name>,
    /// Each split entity and its pieces.
    pub split: Vec<(Name, Vec<Name>)>,
    /// Each set of merged entities and the name standing for them.
    pub merged: Vec<(Vec<Name>, Name)>,
    /// Entities gone.
    pub deleted: Vec<Name>,
}
