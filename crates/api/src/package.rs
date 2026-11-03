//! [`Package`]: what one package registers with the environment, and
//! [`PackageUi`], the UI it carries when it has one.
use crate::cook::NodeType;
use crate::data_type::DataType;
use crate::format::FileFormat;
use crate::held::HeldEdit;
use crate::icon::Icon;
use crate::id::{ContextId, PackageId};
use crate::mesher::Mesher;
use crate::network::{NetworkCompiler, NetworkContext, NodeCreation};
use crate::object::ObjectKindSpec;
use crate::session::{SessionSlot, Slot};
use crate::stored::{Stored, StoredValue};
use kinogaki_render_api::EngineDescription;
use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// Everything one package contributes through the headless API: its data
/// types (a kernel's), held edits over them,
/// B-rep meshers (a kernel's),
/// node types' cooks, the network contexts its object kinds hold, object
/// kinds, file formats, session slots, stored values and icons, the render
/// engines it describes (made, with the `wgpu` feature, by their makers);
/// and its UI, when it has one
/// ([`PackageUi`], which the host UI interface makes).
///
/// Every id it declares has its vendor (the first segment of its id):
/// `acme.trees` registers `acme.trees.branch` or `acme.format.tree`, never
/// `kinogaki.*`.
///
/// ```
/// use kinogaki_studio_api::{Package, PackageId};
/// let package = Package::new(PackageId::declared("acme.trees"), "Trees");
/// assert!(package.data_types.is_empty());
/// ```
#[derive(Clone)]
pub struct Package {
    /// Its id (`kinogaki.mesh`).
    pub id: PackageId,
    /// What a UI calls it ("Mesh").
    pub label: String,
    /// The data types it defines.
    pub data_types: Vec<Arc<dyn DataType>>,
    /// How authored deltas apply to its data types.
    pub held_edits: Vec<Arc<dyn HeldEdit>>,
    /// How it meshes B-rep bodies for display, rendering and export.
    pub meshers: Vec<Arc<dyn Mesher>>,
    /// What its node types compute.
    pub node_types: Vec<Arc<dyn NodeType>>,
    /// The network contexts it declares, whose networks core keeps in the
    /// objects of kinds naming them.
    pub contexts: Vec<NetworkContext>,
    /// The compilers of its compiled contexts, one each.
    pub compilers: Vec<Arc<dyn NetworkCompiler>>,
    /// The further ways it makes nodes, of its own types or others'
    /// (an engine's material presets).
    pub creations: Vec<NodeCreation>,
    /// The object kinds it contributes.
    pub object_kinds: Vec<Arc<dyn ObjectKindSpec>>,
    /// The file formats it reads and writes.
    pub formats: Vec<Arc<dyn FileFormat>>,
    /// Its session state.
    pub slots: Vec<SessionSlot>,
    /// The values it keeps in projects.
    pub stored: Vec<StoredValue>,
    /// The icons it adds to Studio's icon set, named under its id
    /// ([`icon`](crate::icon)).
    pub icons: Vec<Icon>,
    /// The render engines it brings, by their descriptions (headless):
    /// what declares the values its object kinds hold beyond their
    /// placement (a light's prim values, a Render Settings' engine
    /// settings), and what a host lists and renders by.
    pub engines: Vec<Arc<EngineDescription>>,
    /// How each of its engines is made on a device, by engine id (the
    /// `wgpu` feature, which a host that renders enables): each one's
    /// description is among [`engines`](Self::engines).
    ///
    /// A field behind a feature changes the struct's layout. That is sound
    /// because a host and its plugins share one build of this crate (one
    /// feature set, so one layout): a plugin built against another build
    /// is refused before it is loaded.
    #[cfg(feature = "wgpu")]
    pub makers: Vec<Arc<dyn kinogaki_render_api::display::Engine>>,
    /// The runners of its compiled contexts' plans, by context (the `wgpu`
    /// feature): how the host shows what a compiled network makes.
    #[cfg(feature = "wgpu")]
    pub runners: Vec<(ContextId, Arc<dyn crate::network::run::PlanRunner>)>,
    /// Its UI: its contexts' parts and settings sections, made by the
    /// host UI interface (`kinogaki-studio-ui-api`'s `WithUi::with_ui`).
    /// `None`: a headless package (a kernel's data types, a render
    /// engine), shown through Studio's own UI alone.
    pub ui: Option<PackageUi>,
}

/// A package's UI as the headless API carries it: the contexts it gives
/// parts to, which the [`Registry`](crate::Registry) checks like every
/// other id, and the parts themselves, opaque here. The host UI interface
/// (`kinogaki-studio-ui-api`) makes it and reads the parts back; this
/// crate can't name its types.
#[derive(Clone)]
pub struct PackageUi {
    contexts: Vec<ContextId>,
    parts: Arc<dyn Any + Send + Sync>,
}

impl PackageUi {
    /// The UI giving parts to `contexts`, the parts being `parts`.
    pub fn new(contexts: Vec<ContextId>, parts: impl Any + Send + Sync) -> Self {
        Self {
            contexts,
            parts: Arc::new(parts),
        }
    }

    /// The contexts it gives parts to.
    pub(crate) fn contexts(&self) -> &[ContextId] {
        &self.contexts
    }

    /// Its parts, as the UI interface made them.
    pub fn parts(&self) -> &(dyn Any + Send + Sync) {
        &*self.parts
    }
}

impl fmt::Debug for PackageUi {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("PackageUi")
            .field("contexts", &self.contexts)
            .finish_non_exhaustive()
    }
}

impl Package {
    /// Package `id` called `label`, contributing nothing yet.
    pub fn new(id: PackageId, label: impl Into<String>) -> Self {
        Self {
            id,
            label: label.into(),
            data_types: Vec::new(),
            held_edits: Vec::new(),
            meshers: Vec::new(),
            node_types: Vec::new(),
            contexts: Vec::new(),
            compilers: Vec::new(),
            creations: Vec::new(),
            object_kinds: Vec::new(),
            formats: Vec::new(),
            slots: Vec::new(),
            stored: Vec::new(),
            icons: Vec::new(),
            engines: Vec::new(),
            #[cfg(feature = "wgpu")]
            makers: Vec::new(),
            #[cfg(feature = "wgpu")]
            runners: Vec::new(),
            ui: None,
        }
    }

    /// With stored value `T` too.
    pub fn with_stored<T: Stored>(mut self) -> Self {
        self.stored.push(StoredValue::of::<T>());
        self
    }

    /// With data type `ty` too.
    pub fn with_data_type(mut self, ty: impl DataType + 'static) -> Self {
        self.data_types.push(Arc::new(ty));
        self
    }

    /// With held edit `held` too.
    pub fn with_held_edit(mut self, held: impl HeldEdit + 'static) -> Self {
        self.held_edits.push(Arc::new(held));
        self
    }

    /// With node type `node` too.
    pub fn with_node_type(mut self, node: impl NodeType + 'static) -> Self {
        self.node_types.push(Arc::new(node));
        self
    }

    /// With network context `context` too.
    pub fn with_context(mut self, context: NetworkContext) -> Self {
        self.contexts.push(context);
        self
    }

    /// With `compiler`, the compiler of a compiled context it or a package
    /// registered before it declares (Studio's Comp and Shading): one a
    /// context.
    pub fn with_compiler(mut self, compiler: impl NetworkCompiler + 'static) -> Self {
        self.compilers.push(Arc::new(compiler));
        self
    }

    /// With `creation` too, listed wherever its node type is (an
    /// engine's material preset, a Material of its surface).
    pub fn with_creation(mut self, creation: NodeCreation) -> Self {
        self.creations.push(creation);
        self
    }

    /// With B-rep mesher `mesher` too.
    pub fn with_mesher(mut self, mesher: impl Mesher + 'static) -> Self {
        self.meshers.push(Arc::new(mesher));
        self
    }

    /// With object kind `kind` too.
    pub fn with_object_kind(mut self, kind: impl ObjectKindSpec + 'static) -> Self {
        self.object_kinds.push(Arc::new(kind));
        self
    }

    /// With file format `format` too.
    pub fn with_format(mut self, format: impl FileFormat + 'static) -> Self {
        self.formats.push(Arc::new(format));
        self
    }

    /// With session slot `T` too.
    pub fn with_slot<T: Slot>(mut self) -> Self {
        self.slots.push(SessionSlot::of::<T>());
        self
    }

    /// With icon `icon` too: others name it `"<package id>/<icon name>"`
    /// (`icon::qualified`).
    pub fn with_icon(mut self, icon: Icon) -> Self {
        self.icons.push(icon);
        self
    }

    /// With render engine `engine` too: its description and its maker.
    #[cfg(feature = "wgpu")]
    pub fn with_engine(
        mut self,
        engine: impl kinogaki_render_api::display::Engine + 'static,
    ) -> Self {
        self.engines.push(Arc::new(engine.description()));
        self.makers.push(Arc::new(engine));
        self
    }

    /// With the description of render engine `engine` too, which this
    /// build doesn't make (a headless one, or tests reading the values it
    /// declares).
    pub fn with_engine_description(mut self, engine: EngineDescription) -> Self {
        self.engines.push(Arc::new(engine));
        self
    }

    /// With `runner`, which runs compiled context `context`'s plans (a
    /// context it or a package registered before it declares): one a
    /// context.
    #[cfg(feature = "wgpu")]
    pub fn with_runner(
        mut self,
        context: ContextId,
        runner: impl crate::network::run::PlanRunner + 'static,
    ) -> Self {
        self.runners.push((context, Arc::new(runner)));
        self
    }
}

impl fmt::Debug for Package {
    /// The package by its id and the ids of what it contributes.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fn ids<T: ?Sized>(items: &[Arc<T>], id: impl Fn(&T) -> &'static str) -> Vec<&'static str> {
            items.iter().map(|item| id(item)).collect()
        }
        let mut out = f.debug_struct("Package");
        out.field("id", &self.id)
            .field("label", &self.label)
            .field("data_types", &ids(&self.data_types, |t| t.id().as_str()))
            .field("held_edits", &ids(&self.held_edits, |h| h.delta().as_str()))
            .field("node_types", &ids(&self.node_types, |n| n.id().as_str()))
            .field(
                "contexts",
                &self
                    .contexts
                    .iter()
                    .map(|c| c.id.as_str())
                    .collect::<Vec<_>>(),
            )
            .field(
                "object_kinds",
                &ids(&self.object_kinds, |k| k.id().as_str()),
            )
            .field("formats", &ids(&self.formats, |f| f.id().as_str()))
            .field(
                "slots",
                &self
                    .slots
                    .iter()
                    .map(|s| s.id().as_str())
                    .collect::<Vec<_>>(),
            )
            .field(
                "stored",
                &self.stored.iter().map(|s| s.id()).collect::<Vec<_>>(),
            )
            .field(
                "icons",
                &self.icons.iter().map(|i| &i.name).collect::<Vec<_>>(),
            )
            .field(
                "engines",
                &self.engines.iter().map(|e| &e.id).collect::<Vec<_>>(),
            )
            .field("ui", &self.ui);
        out.finish()
    }
}
