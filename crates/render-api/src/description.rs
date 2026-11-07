//! What an engine is, without a device: [`EngineDescription`]. A host
//! builds its UI, files, catalog and inspector from it, and holds the
//! engines it can make in [`Engines`]. A description is data an engine
//! may build at run time; what needs the engine's code (what its own
//! materials come to in the standard form, what it makes of that form) is
//! its [`MaterialRules`].
//!
//! Engines meet in MaterialX: each lists the MaterialX nodes it implements
//! ([`Capabilities::standard_nodes`]), and a material reaches it as it adopts
//! it ([`Engines::adopt`]): its own nodes and those it implements as they
//! are, the others expanded, translated or cut; another engine's material as
//! that engine's standard form, an OpenPBR Surface ([`Engines::standard`]).
//!
//! Every engine outputs linear ACEScg (the beauty and color AOVs): what a
//! host shows through its view transform and what compositing reads.

use std::sync::Arc;

use crate::aov::{Aov, AovSpec};
use crate::declared::PrimType;
use crate::material::{MaterialDisplay, MaterialNetwork, NodeDef, standard};
use crate::prim::PrimKind;
use kinogaki_value::NodeTypeId;
use kinogaki_value::{EngineId, PrimTypeId};
use kinogaki_value::{Parameter, Section};

/// What an engine's materials are to a host: the rules only its code knows.
pub trait MaterialRules: Send + Sync {
    /// One of its own materials in the standard form ([`standard`]), as
    /// close as OpenPBR Surface comes. None: nothing standard approximates
    /// it (a volume).
    fn standard(&self, own: &MaterialNetwork) -> Option<MaterialNetwork> {
        let _ = own;
        None
    }

    /// A material of its own from `standard`, a material in the standard form
    /// whose surface adoption cut: an engine of a vocabulary of its own
    /// translates it here. None: it renders its default material.
    fn translate(&self, standard: &MaterialNetwork) -> Option<MaterialNetwork> {
        let _ = standard;
        None
    }

    /// A simple approximation of one of its materials, for the shaded
    /// viewport: by default, its standard form's.
    fn display(&self, own: &MaterialNetwork) -> MaterialDisplay {
        (self.standard(own)).map_or(MaterialDisplay::color([0.5; 3]), |s| standard::display(&s))
    }
}

/// The rules of an engine bringing no material of its own.
struct NoMaterials;

impl MaterialRules for NoMaterials {}

/// The motion an engine blurs while a camera's shutter is open.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Motion {
    /// Placed prims moving: their placements across the shutter
    /// ([`Sampled`](crate::geometry::Sampled) with more than one sample).
    pub transforms: bool,
    /// Points and instances moving by their velocities (the `v` primvar,
    /// scene units a second; [`crate::SceneSettings::rate`] says how long
    /// a frame is).
    pub velocities: bool,
}

impl Motion {
    /// Whether it blurs anything.
    pub fn any(self) -> bool {
        self.transforms || self.velocities
    }
}

/// What an engine renders: hosts send it nothing else.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Capabilities {
    /// The structural prims it renders.
    pub prims: Vec<PrimKind>,
    /// The prim types it declares (lights, fog), the standard ones as
    /// [`crate::standard`] declares them.
    pub types: Vec<PrimType>,
    /// The outputs it can write, in the order a UI and a file list them
    /// (the beauty first).
    pub aovs: Vec<AovSpec>,
    /// Whether each pass is a whole frame, fast enough to follow a moving
    /// camera at full size: hosts then never shrink a moving view's film.
    pub realtime: bool,
    /// The motion it blurs (none: hosts send still placements).
    pub motion: Motion,
    /// The bytes it holds for a view a pixel of its film, every buffer and
    /// kept image counted: hosts budget the films rendering at a time by it.
    pub film_bytes: u32,
    /// The MaterialX nodes it implements as they are, each by its node
    /// (`open_pbr_surface`) or, where it implements only some of a node's
    /// signatures, by signature (`ND_image_color3`); a material holding
    /// others reaches it adopted ([`Engines::adopt`]).
    pub standard_nodes: Vec<Arc<str>>,
}

impl Capabilities {
    /// Whether it renders prims of `kind`.
    pub fn renders(&self, kind: PrimKind) -> bool {
        self.prims.contains(&kind)
    }
}

/// An engine, as a host sees it before making one. Made by
/// [`EngineDescription::builder`].
#[derive(Clone)]
pub struct EngineDescription {
    id: EngineId,
    label: Arc<str>,
    capabilities: Capabilities,
    setting_groups: Vec<Section>,
    settings: Vec<Parameter>,
    node_defs: Vec<Arc<NodeDef>>,
    material_rules: Arc<dyn MaterialRules>,
}

impl std::fmt::Debug for EngineDescription {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "EngineDescription({:?})", self.id)
    }
}

impl PartialEq for EngineDescription {
    /// Descriptions are equal when they describe the same engine.
    fn eq(&self, other: &EngineDescription) -> bool {
        self.id == other.id
    }
}

/// Makes an [`EngineDescription`].
#[derive(Clone)]
pub struct EngineBuilder(EngineDescription);

impl EngineBuilder {
    /// Rendering what `capabilities` say.
    pub fn capabilities(mut self, capabilities: Capabilities) -> Self {
        self.0.capabilities = capabilities;
        self
    }

    /// Its settings ([`crate::ViewDesc::values`]), and their
    /// groups in order.
    pub fn settings(mut self, groups: Vec<Section>, settings: Vec<Parameter>) -> Self {
        self.0.setting_groups = groups;
        self.0.settings = settings;
        self
    }

    /// Bringing `defs`, its material nodes, under `rules`.
    pub fn materials(mut self, defs: Vec<NodeDef>, rules: Arc<dyn MaterialRules>) -> Self {
        self.0.node_defs = defs.into_iter().map(Arc::new).collect();
        self.0.material_rules = rules;
        self
    }

    /// With the id `id`.
    pub fn id(mut self, id: EngineId) -> Self {
        self.0.id = id;
        self
    }

    /// The description.
    pub fn build(self) -> EngineDescription {
        self.0
    }
}

impl EngineDescription {
    /// Engine `id` (`kinogaki.pixel.engine`, what files name it by) that a UI
    /// calls `label` (`Pixel`): rendering nothing yet, bringing no material.
    pub fn builder(id: EngineId, label: &str) -> EngineBuilder {
        EngineBuilder(EngineDescription {
            id,
            label: Arc::from(label),
            capabilities: Capabilities::default(),
            setting_groups: Vec::new(),
            settings: Vec::new(),
            node_defs: Vec::new(),
            material_rules: Arc::new(NoMaterials),
        })
    }

    /// A builder starting from this description.
    pub fn rebuilt(&self) -> EngineBuilder {
        EngineBuilder(self.clone())
    }

    /// Its id in files.
    pub fn id(&self) -> EngineId {
        self.id
    }

    /// What a UI calls it.
    pub fn label(&self) -> &Arc<str> {
        &self.label
    }

    /// What it renders.
    pub fn capabilities(&self) -> &Capabilities {
        &self.capabilities
    }

    /// Its settings' groups, in order.
    pub fn setting_groups(&self) -> &[Section] {
        &self.setting_groups
    }

    /// Its settings ([`crate::ViewDesc::values`]).
    pub fn settings(&self) -> &[Parameter] {
        &self.settings
    }

    /// The material nodes it brings, in the order a UI lists them (by family).
    pub fn node_defs(&self) -> &[Arc<NodeDef>] {
        &self.node_defs
    }

    /// Its material node `id`, when it declares it.
    pub fn node_def(&self, id: NodeTypeId) -> Option<&Arc<NodeDef>> {
        self.node_defs.iter().find(|d| d.id == id)
    }

    /// Its output `id` ([`Aov::id`]).
    pub fn aov(&self, id: &str) -> Option<&Aov> {
        let aovs = self.capabilities.aovs.iter();
        aovs.map(|a| &a.aov).find(|a| &*a.id == id)
    }

    /// Its prim type `id`, when it declares it.
    pub fn prim_type(&self, id: PrimTypeId) -> Option<&PrimType> {
        self.capabilities.types.iter().find(|t| t.id == id)
    }

    /// Type `id`'s groups and parameters as it renders them (none when it
    /// does not declare the type).
    pub fn schema(&self, id: PrimTypeId) -> Option<(Vec<Section>, Vec<Parameter>)> {
        let ty = self.prim_type(id)?;
        Some((ty.groups.clone(), ty.params.clone()))
    }

    /// What the shaded viewport shows of `network`, one of its materials.
    pub fn display(&self, network: &MaterialNetwork) -> MaterialDisplay {
        self.material_rules.display(network)
    }
}

/// The engines a host can make, in the order it lists them; the first is
/// its default. A host may have none (no render engine is installed, the
/// default): it then renders nothing.
#[derive(Clone, Debug, Default)]
pub struct Engines(Arc<[Arc<EngineDescription>]>);

impl Engines {
    /// The engines `descriptions` describe.
    pub fn new(descriptions: Vec<EngineDescription>) -> Engines {
        Engines(descriptions.into_iter().map(Arc::new).collect())
    }

    /// Engine `id`.
    pub fn get(&self, id: EngineId) -> Option<&Arc<EngineDescription>> {
        self.0.iter().find(|e| e.id() == id)
    }

    /// The default engine: the first, when there is one.
    pub fn default_engine(&self) -> Option<&Arc<EngineDescription>> {
        self.0.first()
    }

    /// Whether there is no engine.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// The engine whose material `network` is: the one declaring the surface
    /// it shows (none for a MaterialX one).
    pub fn owner(&self, network: &MaterialNetwork) -> Option<&Arc<EngineDescription>> {
        let shown = &network.nodes()[network.surface()? as usize];
        self.iter().find(|e| e.node_def(shown.def.id).is_some())
    }

    /// `network` in the standard form, an OpenPBR Surface ([`standard`]): a
    /// MaterialX material's surface translated; an engine's own material as
    /// its rules say. None: its surface translates to none, its owner isn't
    /// installed, or its owner says nothing standard approximates it.
    pub fn standard(&self, network: &MaterialNetwork) -> Option<MaterialNetwork> {
        let own = network.nodes().iter().any(|n| n.def.mtlx().is_none());
        match own {
            true => self.owner(network)?.material_rules.standard(network),
            false => crate::material::adopt::standard(network),
        }
    }

    /// `network` as engine `to` renders it: its MaterialX nodes adopted
    /// ([`material`](crate::material): as they are, expanded, translated or
    /// cut, then folded) and its own as they are; another engine's material
    /// adopted as its standard form. Where adoption cut the surface and `to`
    /// translates the standard form itself ([`MaterialRules::translate`]),
    /// its translation. Empty, `to`'s default material, when nothing
    /// renders.
    pub fn adopt(&self, network: &MaterialNetwork, to: EngineId) -> MaterialNetwork {
        let Some(engine) = self.get(to) else {
            return MaterialNetwork::default();
        };
        let foreign = (network.nodes().iter())
            .any(|n| n.def.mtlx().is_none() && engine.node_def(n.def.id).is_none());
        let standard = match foreign {
            true => self.standard(network),
            false => None,
        };
        let source = match (foreign, &standard) {
            (true, None) => return MaterialNetwork::default(),
            (true, Some(s)) => s,
            (false, _) => network,
        };
        let adopted = crate::material::adopt::adopt(source, engine);
        if adopted.surface().is_some() {
            return adopted;
        }
        let standard = standard.or_else(|| self.standard(network));
        let translated = standard.and_then(|s| engine.material_rules.translate(&s));
        translated.unwrap_or(adopted)
    }

    /// Every engine, in order.
    pub fn iter(&self) -> impl Iterator<Item = &Arc<EngineDescription>> {
        self.0.iter()
    }
}

impl FromIterator<Arc<EngineDescription>> for Engines {
    /// The engines described, in order.
    fn from_iter<I: IntoIterator<Item = Arc<EngineDescription>>>(descriptions: I) -> Self {
        Engines(descriptions.into_iter().collect())
    }
}
