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
//! package provides still opens: its objects are kept as they were read,
//! shown grey, written back unchanged on save, and reported.
use crate::codec::ObjectCodec;
use crate::data::Data;
use crate::id::{ContextId, DataTypeId, NodeTypeId, ObjectKindId};
use crate::refs::{NodeRef, ObjectRef};
use kinogaki_render_api::{Aov, Declared, EngineDescription, Engines, SettingValues};
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

    /// Its icon's name: Studio's (`"cube"`) or a package's
    /// (`"acme.trees/leaf"`, [`icon`](crate::icon)).
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

    /// The lines its objects draw as where they are (a light's shape),
    /// when they draw so.
    fn glyphs(&self) -> Option<&dyn crate::facets::Glyphs> {
        None
    }

    /// The viewport handles `body`'s object shows while it is selected
    /// alone (a spot's cone, an area light's size), each bound by name to
    /// a number of its first declared prim's values ([`RenderedKind`]).
    /// Studio draws and drags them as it does node types' handles, each
    /// drag one undo step written through [`RenderedKind::with_declared`].
    /// None by default.
    fn handles(&self, body: &dyn ObjectBody) -> Vec<crate::cook::Handle> {
        let _ = body;
        Vec::new()
    }

    /// The ways an object level makes this kind's objects now (a Light of
    /// each type the active engine renders; a Pin Board), each listed in
    /// Create, Tab and the A menu by its id. Only what can be made now is
    /// listed (what doesn't apply is hidden); a kind that stays at the root
    /// ([`at_root`](Self::at_root)) is listed only there. None by default:
    /// the kind isn't offered.
    fn creations(&self, cx: &CreateCx<'_>) -> Vec<KindCreation> {
        let _ = cx;
        Vec::new()
    }

    /// A new body for creation `id` (one of [`creations`](Self::creations)'),
    /// which Studio adds selected, holding its creation's start and opened
    /// as it says, in one undo step; `Err` with why it can't be made now
    /// (a key or shelf run after the active engine changed).
    fn create(&self, id: &str, cx: &CreateCx<'_>) -> Result<Arc<dyn ObjectBody>, String> {
        let _ = cx;
        Err(format!("{id} isn't made here"))
    }
}

/// One way an object level makes a kind's objects
/// ([`ObjectKindSpec::creations`]).
#[derive(Clone, Debug, PartialEq)]
pub struct KindCreation {
    /// Its item id, which menus, keys and shelves name it by
    /// (`kinogaki.render.spot_light`, `example.pins.board`).
    pub id: String,
    /// What menus call it ("Spot Light").
    pub label: String,
    /// The menus' family it is listed in ("Lights", "Primitives"); empty:
    /// with the other object kinds.
    pub family: String,
    /// More words search finds it by (other applications' names).
    pub aliases: Vec<String>,
    /// The new object opens, ready for its first node.
    pub opens: bool,
    /// The node its network starts with, displayed (a Mesh object holding
    /// a Sphere); `None`: it starts empty.
    pub start: Option<NodeTypeId>,
    /// Where menus list it: by family (each family where its lowest member
    /// is), then by this, lowest first; equal values keep install order.
    /// Studio's own creations use negative values (Mesh −1000 … Printer
    /// −100, lights −600) and Mesh's primitives 100 and up, so a plugin's
    /// creation, 0 by default, lists after Studio's objects and before the
    /// primitives unless it says otherwise.
    pub order: i32,
}

impl KindCreation {
    /// Creation `id` called `label`, with the other kinds, starting empty,
    /// not opening, at order 0.
    pub fn new(id: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            id: id.into(),
            label: label.into(),
            family: String::new(),
            aliases: Vec::new(),
            opens: false,
            start: None,
            order: 0,
        }
    }
}

/// What a kind's creations may depend on: the installed render engines,
/// and the active Render Settings' engine, if any.
#[derive(Clone, Copy)]
pub struct CreateCx<'a> {
    engines: &'a Engines,
    active: Option<&'a EngineDescription>,
}

impl<'a> CreateCx<'a> {
    /// Creating with `engines` installed, `active` rendering.
    pub fn new(engines: &'a Engines, active: Option<&'a EngineDescription>) -> Self {
        Self { engines, active }
    }
    /// The installed engines.
    pub fn engines(&self) -> &'a Engines {
        self.engines
    }
    /// The engine the active Render Settings name, if any.
    pub fn active(&self) -> Option<&'a EngineDescription> {
        self.active
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
/// (the first at the scene's root with its Render flag on), and its
/// inspector draws the setup by the engine's description and writes an
/// edit back.
pub trait RenderSetupKind: Send + Sync {
    /// The render `body` sets up, when it is one of this kind's.
    fn setup(&self, body: &dyn ObjectBody) -> Option<RenderSetup>;

    /// `body` setting up `setup` instead (an edit of what
    /// [`setup`](Self::setup) answered, or a new object's engine and its
    /// defaults), when it is one of this kind's and `setup` fits it. The
    /// host checks the values and outputs against the engine's
    /// description; the kind checks what it can without one.
    fn with_setup(&self, body: &dyn ObjectBody, setup: RenderSetup) -> Option<Arc<dyn ObjectBody>>;
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
    /// What its render shows: a Comp network's result, or the raw render.
    pub processing: Processing,
}

/// What a render shows: the raw render, or a Comp network's result (the
/// host resolves the nodes against the network and compiles them).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Processing {
    /// A Comp network's displayed node; the raw render while it shows
    /// nothing (empty, or no display flag).
    Network {
        /// The Render Settings object holding the network (`None`: the
        /// object itself).
        source: Option<ObjectRef>,
    },
    /// One node of a Comp network, whatever its display flag.
    Node {
        /// As [`Processing::Network`]'s.
        source: Option<ObjectRef>,
        /// The node.
        node: NodeRef,
    },
    /// The raw render through the view transform.
    #[default]
    Raw,
}
