//! Scene objects a package contributes: [`ObjectKindSpec`] describes a
//! kind (Mesh, CAD, a CAM setup), [`ObjectBody`] is one object's own state
//! (a Mesh object's network).
//!
//! The environment keeps every object's name, place in the hierarchy,
//! placement and flags; the body is the rest, owned and read by the
//! package. A project that names a kind no package provides still opens:
//! its objects are kept as placeholders and save back unchanged.
use crate::codec::ObjectCodec;
use crate::data::Data;
use crate::id::{ContextId, DataTypeId, ObjectKindId};
use std::any::Any;
use std::fmt::Debug;
use std::sync::Arc;

/// One object's package-owned state. Bodies are immutable and shared: an
/// edit makes a new body, so undo keeps the old one.
pub trait ObjectBody: Any + Send + Sync + Debug {
    /// The kind it is a body of.
    fn kind(&self) -> ObjectKindId;
}

impl dyn ObjectBody {
    /// The body as its package's concrete type, when it is a `T`.
    pub fn downcast_ref<T: ObjectBody>(&self) -> Option<&T> {
        (self as &dyn Any).downcast_ref()
    }
}

/// A kind of scene object.
pub trait ObjectKindSpec: Send + Sync {
    /// Its id (`kinogaki.mesh.object`).
    fn id(&self) -> ObjectKindId;

    /// What the outliner and menus call it ("Mesh").
    fn label(&self) -> &str;

    /// Its icon's name in the host's icon set.
    fn icon(&self) -> &str;

    /// Whether its objects have a placement in the scene (Mesh and CAD
    /// objects do; a Material Library doesn't).
    fn placed(&self) -> bool;

    /// The context of the network inside its objects, when they hold one.
    fn context(&self) -> Option<ContextId>;

    /// The data type of its objects' results (what the viewport draws and
    /// engines render), when they have one.
    fn result(&self) -> Option<DataTypeId>;

    /// The body of a new, empty object of this kind.
    fn new_body(&self) -> Arc<dyn ObjectBody>;

    /// How its bodies save into a project.
    fn codec(&self) -> &dyn ObjectCodec;

    /// A new body holding `data` (an imported value), when this kind takes
    /// values of its type: a Mesh object adopts a mesh as a network that
    /// yields it. `None`: this kind doesn't take such values.
    fn adopts(&self, data: &Data) -> Option<Arc<dyn ObjectBody>>;
}
