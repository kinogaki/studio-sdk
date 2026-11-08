//! [`RefFilter`]: what a reference parameter may name. The parameter says
//! what fits; the host lists the scene's objects, nodes and Materials that
//! do, lets the user pick one (from a searchable list, or dragged from the
//! outliner or a network) and resolves the path, written from the root or
//! from the node holding the parameter, when the node cooks. [`ObjectRef`]
//! names an object by the host's id; [`ImageRef`] an image a row reads
//! where no wire reaches it.
use crate::id::{ContextId, DataTypeId, ObjectKindId};
use kinogaki_geometry::image::{ImageFile, ImageSource};
use std::sync::Arc;

/// A scene object, by the host's id for it: it names the object whatever
/// the object is called or wherever it moves. Valid while the object
/// exists; the host answers `None` for one that is gone.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ObjectRef(pub u64);

/// An image a row reads where no wire reaches it (a dome's): a file, on
/// disk or held embedded in the project, with how its colors read; or the
/// host's Image node by its path from the root, read as that node reads.
/// The host follows a node through renames and moves, and refuses one
/// reading a render (which renders what reads it).
#[derive(Clone, Debug, PartialEq)]
pub enum ImageRef {
    /// A file (its path, relative to the project's folder unless absolute,
    /// or the file's bytes), and how its colors read.
    File(ImageSource),
    /// The Image node at this path from the root (`/Library/Sky`).
    Node(Arc<str>),
}

impl ImageRef {
    /// Why it names nothing, when it doesn't: a blank path, a file of no
    /// name, a node path not from the root.
    pub fn refusal(&self) -> Option<&'static str> {
        match self {
            ImageRef::File(source) => match source.file() {
                ImageFile::Path(path) if path.trim().is_empty() => Some("names no file"),
                ImageFile::Embedded(file) if file.name().is_empty() => Some("holds no file"),
                _ => None,
            },
            ImageRef::Node(path) if !path.starts_with('/') => {
                Some("names a node by a path not from the root")
            }
            ImageRef::Node(_) => None,
        }
    }
}

/// What a reference may name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RefFilter {
    /// What fits.
    pub fits: Fits,
}

/// What fits a reference.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Fits {
    /// Objects of these kinds (cameras: the camera object kind); any object
    /// when empty.
    Objects(Vec<ObjectKindId>),
    /// Objects and nodes whose result is of one of these data types
    /// (geometry: a Mesh object, a Part, a node in a Mesh network).
    Data(Vec<DataTypeId>),
    /// Any Material: a Material node, a Material Library's or one in any
    /// other network.
    Materials,
    /// An object holding a network of this context, or a node of one.
    Networks(ContextId),
    /// What writes a picture: an object that renders (its Output), or a node
    /// with a [`SaveFile`](crate::ValueKind::SaveFile) row. The row resolves
    /// to the file's path, so a node reads what was last written there.
    Written,
}

impl RefFilter {
    /// One object of `kinds` (any object when empty).
    pub fn objects(kinds: impl IntoIterator<Item = ObjectKindId>) -> Self {
        Self::of(Fits::Objects(kinds.into_iter().collect()))
    }

    /// One object or node whose result is of `types`.
    pub fn data(types: impl IntoIterator<Item = DataTypeId>) -> Self {
        Self::of(Fits::Data(types.into_iter().collect()))
    }

    /// One Material.
    pub fn material() -> Self {
        Self::of(Fits::Materials)
    }

    /// One network of `context`, by the object holding it, or one of its nodes.
    pub fn network(context: ContextId) -> Self {
        Self::of(Fits::Networks(context))
    }

    /// One object or node that writes a picture.
    pub fn written() -> Self {
        Self::of(Fits::Written)
    }

    fn of(fits: Fits) -> Self {
        RefFilter { fits }
    }
}
