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
use crate::id::{ContextId, CreationId, DataTypeId, NodeTypeId, ObjectKindId};
use crate::refs::{NodeRef, ObjectRef};
use crate::stored::{Stored, StoredValues};
use crate::{Diagnostic, Error};
use kinogaki_render_api::{
    Aov, Declared, EngineDescription, EngineId, Engines, PreviewSurface, SettingDescriptor,
    SettingGroup,
};
use kinogaki_value::{Number, RefFilter, Unit, Value, ValueKind};
use std::any::Any;
use std::fmt::Debug;
use std::sync::{Arc, LazyLock};

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
    /// and core reads it again when the body or the path changes (a
    /// printer's value naming its slots' Materials by path). With it come
    /// lines for what the value leaves out of the body (an engine's own
    /// parameters on a standard light), which Export reports. `None` by
    /// default: a reference to it names nothing.
    fn value(&self, body: &dyn ObjectBody, path: &str) -> Option<Valued> {
        let _ = (body, path);
        None
    }

    /// The Materials `body`'s object publishes (a printer's slots), each
    /// under the object's path by its key (`/X1C/AMS 1 · 2`): read-only,
    /// listed wherever Materials are picked and resolved like a Material
    /// Library's. Core asks again whenever the body changes. None by
    /// default.
    fn materials(&self, body: &dyn ObjectBody) -> Vec<PublishedMaterial> {
        let _ = body;
        Vec::new()
    }

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
    /// Studio's own creations use negative values (Mesh −1000 … Printer
    /// −100, lights −600) and Mesh's primitives 100 and up, so a plugin's
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
/// from a Stash); and the references the file's other items make that now
/// resolve inside it: `key` to `<the object's path>/<below>` (a file's
/// material key to a Material's name).
#[derive(Clone)]
pub struct Adoption {
    /// The body.
    pub body: Arc<dyn ObjectBody>,
    /// Its network's content, when the kind builds it.
    pub network: Option<crate::network::NetworkSeed>,
    /// References to `.0` resolve to the object's path and `.1` below it.
    pub renamed: Vec<(String, String)>,
}

impl Adoption {
    /// An object of `body`, its network (if any) showing the value.
    pub fn new(body: Arc<dyn ObjectBody>) -> Self {
        Self {
            body,
            network: None,
            renamed: Vec::new(),
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
}

/// A Material an object publishes ([`ObjectKindSpec::materials`]).
#[derive(Clone, Debug, PartialEq)]
pub struct PublishedMaterial {
    key: String,
    /// What pickers call it ("AMS 1 · 2 PLA Basic").
    pub label: String,
    /// How it looks: renders and the viewport show it through the active
    /// engine's preview surface.
    pub preview: PreviewSurface,
    /// Why it can't be used now ("the slot is empty"); a path naming it
    /// then names nothing, saying why.
    pub unavailable: Option<String>,
}

impl PublishedMaterial {
    /// Material `key` (its path step below the object), called `label`,
    /// looking as `preview`.
    ///
    /// # Errors
    /// When `key` is not an `is_path_step`.
    pub fn new(
        key: impl Into<String>,
        label: impl Into<String>,
        preview: PreviewSurface,
    ) -> Result<Self, crate::Error> {
        let key = key.into();
        if !is_path_step(&key) {
            return Err(crate::Error::invalid(format_args!(
                "{key:?} is not a path step"
            )));
        }
        Ok(Self {
            key,
            label: label.into(),
            preview,
            unavailable: None,
        })
    }

    /// Its path step below its object.
    pub fn key(&self) -> &str {
        &self.key
    }

    /// Unavailable, saying `why`.
    pub fn unavailable(mut self, why: impl Into<String>) -> Self {
        self.unavailable = Some(why.into());
        self
    }
}

/// Whether `text` may name one step of a scene path: not empty, without
/// `/`, the wildcards `*` and `?`, or space at either end, and neither `.`
/// nor `..`. Spaces and any other characters are allowed.
pub(crate) fn is_path_step(text: &str) -> bool {
    !text.is_empty()
        && text.trim() == text
        && text != "."
        && text != ".."
        && !text.contains(['/', '*', '?'])
}

/// What a kind's creations may depend on: the installed render engines,
/// the active Render Settings' engine, if any, and the user's stored
/// values.
#[derive(Clone, Copy)]
pub struct CreateCx<'a> {
    engines: &'a Engines,
    active: Option<&'a EngineDescription>,
    user: Option<&'a StoredValues>,
}

impl<'a> CreateCx<'a> {
    /// Creating with `engines` installed, `active` rendering, no user
    /// values.
    pub fn new(engines: &'a Engines, active: Option<&'a EngineDescription>) -> Self {
        Self {
            engines,
            active,
            user: None,
        }
    }

    /// Reading the user's stored values ([`Scope::User`](crate::Scope::User))
    /// from `user`.
    pub fn with_user(mut self, user: &'a StoredValues) -> Self {
        self.user = Some(user);
        self
    }

    /// The user's value `T`, when the host lent the user's values and a
    /// package registered `T` for the user.
    pub fn user<T: Stored>(&self) -> Option<&'a T> {
        self.user?.get::<T>()
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
    pub engine: EngineId,
    /// Its own settings ([`RenderSetup::settings`]) and the engine's, by
    /// their descriptors' names; one missing takes its descriptor's
    /// default.
    pub values: crate::Parameters,
    /// The outputs a final render writes beside the beauty, by the
    /// engine's ids (`Capabilities::aovs`).
    pub outputs: Vec<Aov>,
    /// What its render shows: a Comp network's result, or the raw render.
    pub processing: Processing,
}

impl RenderSetup {
    /// The final image's width and height, pixels.
    pub const RESOLUTION: &str = "resolution";
    /// The Camera object a final render looks through (none: the view's
    /// camera).
    pub const CAMERA: &str = "camera";
    /// Where a final render writes its image, relative to the project's
    /// folder unless absolute.
    pub const OUTPUT: &str = "output";
    /// How many placements across the shutter a moving prim sends an
    /// engine that blurs motion: core samples them. The host offers it
    /// only while the engine blurs motion and the camera's shutter is open.
    pub const MOTION_SAMPLES: &str = "motionSamples";

    /// A setup's own settings, every engine's alike, in the Image group
    /// before the engine's: Resolution, Camera, Output and Motion Samples.
    /// They are the setup's structure, so none animates.
    pub fn settings() -> &'static [SettingDescriptor] {
        static SETTINGS: LazyLock<[SettingDescriptor; 4]> = LazyLock::new(|| {
            let pixels = Number::new(Unit::Suffix(" px".into()))
                .range(1.0, 16_384.0)
                .decimals(0)
                .speed(1.0);
            let size = ValueKind::Vector {
                components: 2,
                number: pixels,
            };
            let cameras = RefFilter::objects([ObjectKindId::declared(crate::standard::CAMERA)]);
            let own = |name, label, kind, default: Value| {
                SettingDescriptor::new(name, label, IMAGE, kind, default).animatable(false)
            };
            [
                own(
                    RenderSetup::RESOLUTION,
                    "Resolution",
                    size,
                    vec![1920.0, 1080.0].into(),
                ),
                own(
                    RenderSetup::CAMERA,
                    "Camera",
                    ValueKind::Object(cameras),
                    Value::Object(None),
                ),
                own(
                    RenderSetup::OUTPUT,
                    "Output",
                    ValueKind::File,
                    "render.exr".into(),
                ),
                own(
                    RenderSetup::MOTION_SAMPLES,
                    "Motion Samples",
                    ValueKind::integer(2, 16),
                    Value::Integer(3),
                ),
            ]
        });
        &*SETTINGS
    }

    /// The group a setup's own settings sit in.
    pub fn group() -> SettingGroup {
        SettingGroup::new(IMAGE, "Image", true)
    }

    /// The final image's width and height, pixels.
    pub fn resolution(&self) -> [u32; 2] {
        match self.own(Self::RESOLUTION).numbers() {
            Some(&[w, h]) => [w, h].map(|side| side.round() as u32),
            _ => unreachable!("its kind takes two numbers"),
        }
    }

    /// The Camera object a final render looks through (none: the view's).
    pub fn camera(&self) -> Option<ObjectRef> {
        self.own(Self::CAMERA).object()
    }

    /// Where a final render writes its image.
    pub fn output(&self) -> &str {
        self.own(Self::OUTPUT).text().unwrap_or_default()
    }

    /// The placements across the shutter a moving prim sends.
    pub fn motion_samples(&self) -> u32 {
        match self.own(Self::MOTION_SAMPLES) {
            &Value::Integer(n) => n as u32,
            _ => unreachable!("its kind takes an integer"),
        }
    }

    /// Own setting `name`'s value: the one held when its kind takes it,
    /// else its default.
    fn own(&self, name: &str) -> &Value {
        let d = (Self::settings().iter()).find(|d| &*d.name == name);
        let d = d.expect("a setup's own setting");
        (self.values.get(name))
            .filter(|v| d.accepts(v))
            .unwrap_or(&d.default)
    }
}

/// The name of the group a setup's own settings sit in.
const IMAGE: &str = "image";

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
