//! Scene objects a package contributes: [`ObjectKindSpec`] describes a
//! kind (Mesh, Camera, Render Output), [`ObjectBody`] is one object's own state
//! (a Mesh object's display settings). A kind whose objects are render prims
//! themselves (an engine's lights and fog) says so through its optional
//! [`RenderedKind`] facet.
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
use crate::id::{ContextId, CreationId, DataTypeId, NodeTypeId, ObjectKindId};
use crate::{Diagnostic, Error};
use kinogaki_render_api::{Declared, Engines};
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

    /// The data types an object of this kind can be made from (an
    /// imported value, [`adopts`](Self::adopts)): what Import offers formats
    /// to read into. None by default.
    fn adopted(&self) -> Vec<DataTypeId> {
        Vec::new()
    }

    /// What an object made from `data` (an imported value of one of its
    /// [`adopted`](Self::adopted) types) starts as, made with the engines
    /// `cx` gives: a Mesh object adopts a mesh, which its network then
    /// yields; a Material Library adopts a file's materials as Materials.
    /// `None` (the default): this kind doesn't take this value.
    fn adopts(&self, _data: &Data, _cx: &CreateCx<'_>) -> Option<Adoption> {
        None
    }

    /// The value of an object holding `body` at scene path `path`
    /// (`/Workshop/X1C`), when its kind holds no network
    /// ([`context`](Self::context) `None`) and has a
    /// [`result`](Self::result): a reference naming the object reads it,
    /// and core reads it again when the body or the path changes. With it come
    /// lines for what the value leaves out of the body (an engine's own
    /// parameters on a standard light), which Export reports. `None` by
    /// default: a reference to it names nothing.
    fn value(&self, body: &dyn ObjectBody, path: &str) -> Option<Valued> {
        let _ = (body, path);
        None
    }

    /// Whether its objects live only at the scene's root (an engine's fog,
    /// Render Output).
    fn at_root(&self) -> bool {
        false
    }

    /// How its objects are render prims themselves (lights, fog), when they
    /// are.
    fn renders(&self) -> Option<&dyn RenderedKind> {
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
    /// each type its engine renders; a Pin Board), each listed in
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
    /// (a key or shelf naming a creation that is no longer offered).
    fn create(&self, id: CreationId, cx: &CreateCx<'_>) -> Result<Arc<dyn ObjectBody>, Error> {
        let _ = cx;
        Err(Error::invalid(format_args!("{id} isn't made here")))
    }
}

/// One way an object level makes a kind's objects
/// ([`ObjectKindSpec::creations`]).
#[derive(Clone, Debug, PartialEq)]
pub struct KindCreation {
    /// Its item id, which menus, keys and shelves name it by
    /// (`kinogaki.render.spot_light`, `example.pins.board`).
    pub id: CreationId,
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
    /// Studio's own creations use negative values (Mesh −1000 … lights
    /// −600) and Mesh's primitives 100 and up, so a plugin's
    /// creation, 0 by default, lists after Studio's objects and before the
    /// primitives unless it says otherwise.
    pub order: i32,
}

impl KindCreation {
    /// Creation `id` called `label`, with the other kinds, starting empty,
    /// not opening, at order 0.
    pub fn new(id: CreationId, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            family: String::new(),
            aliases: Vec::new(),
            opens: false,
            start: None,
            order: 0,
        }
    }
}

/// A networkless object's value ([`ObjectKindSpec::value`]) and what of
/// its body the value leaves out, as warnings ("the gobo is left out"),
/// each about the object and without its path.
#[derive(Clone)]
pub struct Valued {
    /// The value.
    pub value: Data,
    /// What it leaves out.
    pub warnings: Vec<Diagnostic>,
}

impl Valued {
    /// `value`, leaving nothing out.
    pub fn new(value: Data) -> Self {
        Self {
            value,
            warnings: Vec::new(),
        }
    }
}

/// What an object made from an imported value starts as
/// ([`ObjectKindSpec::adopts`]): its body; its network's content when the
/// kind builds it from the value (otherwise the network shows the value
/// from a Stash); the references the file's other items make that now
/// resolve inside it: `key` to `<the object's path>/<below>` (a file's
/// material key to a Material's name, an image key its lights name by
/// `op:` to an Image node's); and what it approximates or leaves out, for
/// the user.
#[derive(Clone)]
pub struct Adoption {
    /// The body.
    pub body: Arc<dyn ObjectBody>,
    /// Its network's content, when the kind builds it.
    pub network: Option<crate::network::NetworkSeed>,
    /// References to `.0` resolve to the object's path and `.1` below it.
    pub renamed: Vec<(String, String)>,
    /// What it approximates or leaves out of the value ("Red takes its
    /// standard form: acme.engine isn't installed"): warnings.
    pub warnings: Vec<Diagnostic>,
}

impl Adoption {
    /// An object of `body`, its network (if any) showing the value.
    pub fn new(body: Arc<dyn ObjectBody>) -> Self {
        Self {
            body,
            network: None,
            renamed: Vec::new(),
            warnings: Vec::new(),
        }
    }

    /// Its network starting as `seed`.
    pub fn network(mut self, seed: crate::network::NetworkSeed) -> Self {
        self.network = Some(seed);
        self
    }

    /// References to `key` resolving to `below` the object.
    pub fn renamed(mut self, key: impl Into<String>, below: impl Into<String>) -> Self {
        self.renamed.push((key.into(), below.into()));
        self
    }

    /// Saying `message`, a warning.
    pub fn warning(mut self, message: impl Into<String>) -> Self {
        self.warnings.push(Diagnostic::warning(message));
        self
    }
}

/// What a kind's creations may depend on: the installed render engines.
#[derive(Clone, Copy)]
pub struct CreateCx<'a> {
    engines: &'a Engines,
}

impl<'a> CreateCx<'a> {
    /// Creating with `engines` installed.
    pub fn new(engines: &'a Engines) -> Self {
        Self { engines }
    }

    /// The installed engines.
    pub fn engines(&self) -> &'a Engines {
        self.engines
    }
}

/// A kind whose objects are render prims of their own: a light or fog,
/// prims of types engines declare. The host reads what an object is
/// through it and never reads the body: render sync keeps, for the active
/// engine, the values its type declares; the viewport
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
