//! Scene objects a package contributes: [`ObjectKindSpec`] describes a
//! kind (Mesh, CAD, a CAM setup), [`ObjectBody`] is one object's own state
//! (a Mesh object's display settings). A kind whose objects take part in a render
//! themselves says how through its optional facets: [`RenderedKind`] (a
//! light, fog: prims of types engines declare) and [`RenderSetupKind`]
//! (Render Settings: which engine renders, and how).
//!
//! The environment keeps every object's name, place in the hierarchy,
//! placement and flags, and, for a kind that names a context
//! ([`ObjectKindSpec::context`]), the network inside the object: core
//! creates, edits, cooks, undoes and saves it beside the body. The body is
//! the rest, owned and read by the package. A project that names a kind no
//! package provides still opens: its objects are skipped and reported.
use crate::codec::ObjectCodec;
use crate::data::Data;
use crate::id::{ContextId, DataTypeId, ObjectKindId};
use crate::refs::ObjectRef;
use kinogaki_render_api::{Aov, Declared, SettingValues};
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

    /// The context of the network inside its objects, when they hold one:
    /// core keeps that network beside each object's body (the body never
    /// holds it).
    fn context(&self) -> Option<ContextId>;

    /// The data type of its objects' results (what the viewport draws and
    /// engines render), when they have one.
    fn result(&self) -> Option<DataTypeId>;

    /// The body of a new, empty object of this kind.
    fn new_body(&self) -> Arc<dyn ObjectBody>;

    /// How its bodies save into a project.
    fn codec(&self) -> &dyn ObjectCodec;

    /// A new body for an object holding `data` (an imported value), when
    /// this kind takes values of its type: a Mesh object adopts a mesh,
    /// which its network then yields. `None`: this kind doesn't take such
    /// values.
    fn adopts(&self, data: &Data) -> Option<Arc<dyn ObjectBody>>;

    /// Whether its objects live only at the scene's root (Fog, Render
    /// Settings).
    fn at_root(&self) -> bool {
        false
    }

    /// How its objects are render prims themselves (lights, fog), when they
    /// are.
    fn renders(&self) -> Option<&dyn RenderedKind> {
        None
    }

    /// How its objects set up a render (Render Settings), when they do.
    fn render_setup(&self) -> Option<&dyn RenderSetupKind> {
        None
    }
}

/// A kind whose objects are render prims of their own: a light or fog,
/// prims of types engines declare. The host reads what an object is
/// through it and never reads the body: render sync keeps, for the active
/// engine, the values its type and applied schemas declare; the viewport
/// and exports read the standard ones (UsdLux lights); the inspector draws
/// the values by their types' schemas and writes an edit back.
pub trait RenderedKind: Send + Sync {
    /// What `body` is: its prims, with every value it holds, placed at the
    /// origin (the host places them at the object).
    fn declared(&self, body: &dyn ObjectBody) -> Vec<Declared>;

    /// `body` holding `declared` instead (an edit of what
    /// [`declared`](Self::declared) answered), when it is one of this
    /// kind's and `declared` fits it.
    fn with_declared(
        &self,
        body: &dyn ObjectBody,
        declared: &[Declared],
    ) -> Option<Arc<dyn ObjectBody>>;
}

/// A kind whose objects set up a render: which engine renders, at what
/// size, through which camera, with which of the engine's settings and
/// outputs. The host's Render mode and Render Image read the active one
/// (the first at the scene's root with its Render flag on).
pub trait RenderSetupKind: Send + Sync {
    /// The render `body` sets up, when it is one of this kind's.
    fn setup(&self, body: &dyn ObjectBody) -> Option<RenderSetup>;
}

/// What a render is set up as: what [`RenderSetupKind::setup`] answers.
#[derive(Clone, Debug, PartialEq)]
pub struct RenderSetup {
    /// The engine that renders (its id).
    pub engine: Arc<str>,
    /// The final image's width and height, pixels.
    pub resolution: [u32; 2],
    /// The Camera object a final render looks through (none: the view's).
    pub camera: Option<ObjectRef>,
    /// Where a final render writes its image, relative to the project's
    /// folder unless absolute.
    pub output: String,
    /// The outputs a final render writes beside the beauty, by the
    /// engine's ids (`EngineDescription::aovs`).
    pub outputs: Vec<Aov>,
    /// The engine's own settings, by its descriptors' names.
    pub values: SettingValues,
}
