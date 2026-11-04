//! What an engine is, without a device: [`EngineDescription`]. A host
//! builds its UI, files, catalog and inspector from it, and holds the
//! engines it can make in [`Engines`]. A description is data an engine
//! may build at run time; what needs the engine's code (how its materials
//! look in a viewport, what it makes of another engine's) is its
//! [`MaterialRules`].
//!
//! Every engine outputs linear ACEScg (the beauty and color AOVs): what a
//! host shows through its view transform and what compositing reads.

use std::sync::Arc;

use crate::aov::{Aov, AovSpec};
use crate::declared::{AppliedSchema, PrimType};
use crate::id::EngineId;
use crate::material::{MaterialDisplay, MaterialNetwork, NodeDef};
use crate::prim::PrimKind;
use crate::settings::{SettingDescriptor, SettingGroup};
use kinogaki_value::NodeTypeId;

/// What an engine's materials are to a host: the rules only its code knows.
pub trait MaterialRules: Send + Sync {
    /// A simple approximation of one of its materials, for the shaded
    /// viewport: its color, and the primvar it reads, where it can say.
    fn display(&self, network: &MaterialNetwork) -> MaterialDisplay;

    /// A material of its own from `foreign`, a network holding nodes it
    /// doesn't declare (another engine's, a file's). None: it renders it as
    /// its default material.
    fn translate(&self, foreign: &MaterialNetwork) -> Option<MaterialNetwork> {
        let _ = foreign;
        None
    }
}

/// The rules of an engine bringing no material of its own.
struct NoMaterials;

impl MaterialRules for NoMaterials {
    fn display(&self, _: &MaterialNetwork) -> MaterialDisplay {
        MaterialDisplay::color([0.5; 3])
    }
}

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
    /// The parameters it adds to types (a gobo on distant lights).
    pub applied: Vec<AppliedSchema>,
    /// The outputs it can write, in the order a UI and a file list them
    /// (the beauty first).
    pub aovs: Vec<AovSpec>,
    /// Whether each pass is a whole frame, fast enough to follow a moving
    /// camera at full size: hosts then never shrink a moving view's film.
    pub realtime: bool,
    /// The motion it blurs (none: hosts send still placements).
    pub motion: Motion,
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
    setting_groups: Vec<SettingGroup>,
    settings: Vec<SettingDescriptor>,
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
    pub fn settings(mut self, groups: Vec<SettingGroup>, settings: Vec<SettingDescriptor>) -> Self {
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
    pub fn id(&self) -> &EngineId {
        &self.id
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
    pub fn setting_groups(&self) -> &[SettingGroup] {
        &self.setting_groups
    }

    /// Its settings ([`crate::ViewDesc::values`]).
    pub fn settings(&self) -> &[SettingDescriptor] {
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
    pub fn prim_type(&self, id: &str) -> Option<&PrimType> {
        self.capabilities.types.iter().find(|t| &*t.id == id)
    }

    /// The schemas it applies to type `id`.
    pub(crate) fn applied_to<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a AppliedSchema> {
        (self.capabilities.applied.iter()).filter(move |a| a.applies(id))
    }

    /// Type `id`'s groups and parameters as it renders them: the type's,
    /// then each applied schema's (none when it does not declare the
    /// type).
    pub fn schema(&self, id: &str) -> Option<(Vec<SettingGroup>, Vec<SettingDescriptor>)> {
        let ty = self.prim_type(id)?;
        let mut groups = ty.groups.clone();
        let mut params = ty.params.clone();
        for a in self.applied_to(id) {
            groups.extend(a.groups.iter().cloned());
            params.extend(a.params.iter().cloned());
        }
        Some((groups, params))
    }

    /// `network` as this engine renders it: as it is when it declares every
    /// node of it; else what it translates it to, or empty (its default
    /// material) when it can't.
    pub fn adopt(&self, network: &MaterialNetwork) -> MaterialNetwork {
        let own = network
            .nodes()
            .iter()
            .all(|n| self.node_def(n.def).is_some());
        if own {
            return network.clone();
        }
        (self.material_rules.translate(network)).unwrap_or_default()
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
    pub fn get(&self, id: &EngineId) -> Option<&Arc<EngineDescription>> {
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

    /// The engine whose material `network` is: the one declaring the node it
    /// shows.
    pub fn owner(&self, network: &MaterialNetwork) -> Option<&Arc<EngineDescription>> {
        let shown = network.output()?;
        self.iter().find(|e| e.node_def(shown.def).is_some())
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
