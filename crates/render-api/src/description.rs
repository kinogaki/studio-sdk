//! What an engine is, without a device: [`EngineDescription`]. A host
//! builds its UI, files, catalog and inspector from it, and holds the
//! engines it can make in [`Engines`]. A description is data an engine
//! may build at run time; what needs the engine's code (how its materials
//! look elsewhere) is its [`MaterialRules`].
//!
//! Every engine outputs linear ACEScg (the beauty and color AOVs): what a
//! host shows through its view transform and what compositing reads.

use std::sync::Arc;

use crate::aov::{Aov, AovSpec};
use crate::declared::{AppliedSchema, PrimType};
use crate::exchange::PreviewSurface;
use crate::material::MaterialNetwork;
use crate::prim::PrimKind;
use crate::settings::{SettingDescriptor, SettingGroup, SettingValues};
use crate::vocabulary::{NodeType, Params};

/// The one look a raster view and other engines give a material: a color
/// (linear ACEScg), the mesh attribute its base color reads, or a color
/// times an attribute.
#[derive(Clone, Debug, PartialEq)]
pub enum Tint {
    /// One color.
    Color([f32; 3]),
    /// A mesh attribute, read as a color (0, black, on a mesh without it).
    Attribute(Arc<str>),
    /// `color` times a mesh attribute read as a color; `color` alone on a
    /// mesh without it (a Pixel Surface's Color and Color Attribute).
    Modulated {
        /// The material's own color.
        color: [f32; 3],
        /// The attribute that scales it.
        attribute: Arc<str>,
    },
}

impl Tint {
    /// Its color where no attribute is read: the one color, a modulated
    /// tint's own color; none for a bare attribute.
    pub fn color(&self) -> Option<[f32; 3]> {
        match self {
            Tint::Color(c) | Tint::Modulated { color: c, .. } => Some(*c),
            Tint::Attribute(_) => None,
        }
    }

    /// The attribute it reads, if any.
    pub fn attribute(&self) -> Option<&Arc<str>> {
        match self {
            Tint::Color(_) => None,
            Tint::Attribute(name)
            | Tint::Modulated {
                attribute: name, ..
            } => Some(name),
        }
    }
}

/// A material to start from: a surface with settings and inputs set.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialPreset {
    /// Its name, and the new material's.
    pub name: Arc<str>,
    /// Words search accepts for it.
    pub aliases: Vec<Arc<str>>,
    /// Its surface node type's name.
    pub surface: Arc<str>,
    /// The surface's settings.
    pub params: Params,
    /// The surface's inputs set, by socket name (a number fills all three).
    pub values: Vec<(Arc<str>, [f64; 3])>,
}

/// How an engine renders material previews: its settings for them and the
/// floor the preview sphere stands on.
#[derive(Clone, Debug, PartialEq)]
pub struct PreviewSetup {
    /// Settings over the defaults.
    pub settings: SettingValues,
    /// The floor's material.
    pub floor: MaterialNetwork,
}

/// What an engine's materials are to others: the rules only its code
/// knows.
pub trait MaterialRules: Send + Sync {
    /// What one of its materials shows as elsewhere (none: no one look,
    /// such as a texture in its base color).
    fn tint(&self, network: &MaterialNetwork) -> Option<Tint>;
    /// Whether one of its materials fills its mesh as a volume (a
    /// participating medium such as fog) rather than shading its surface:
    /// raster views show such meshes as see-through bounds.
    fn volume(&self, network: &MaterialNetwork) -> bool;
    /// A material of its own showing `tint`: what it renders another
    /// engine's material as.
    fn flat(&self, tint: &Tint) -> MaterialNetwork;
    /// The material it renders what names no material with.
    fn default_material(&self) -> MaterialNetwork;
    /// One of its materials as a preview surface (for export).
    fn preview_surface(&self, network: &MaterialNetwork) -> PreviewSurface;
    /// A material of its own from a preview surface (on import).
    fn import_preview_surface(&self, surface: &PreviewSurface) -> MaterialNetwork;
}

/// An engine, as a host sees it before making one.
#[derive(Clone)]
pub struct EngineDescription {
    /// Its id in files (`pixel`).
    pub id: Arc<str>,
    /// What a UI calls it (`Pixel`).
    pub label: Arc<str>,
    /// The structural prims it renders (hosts send no others).
    pub prims: Vec<PrimKind>,
    /// The prim types it declares (lights, fog), the standard ones as
    /// [`crate::standard`] declares them; hosts send prims of no others.
    pub types: Vec<PrimType>,
    /// The parameters it adds to types (a gobo on distant lights).
    pub applied: Vec<AppliedSchema>,
    /// Its settings' groups, in order.
    pub setting_groups: Vec<SettingGroup>,
    /// Its settings beyond [`crate::RenderSettings`]'s own fields.
    pub settings: Vec<SettingDescriptor>,
    /// Its material vocabulary, in the order a UI lists it (by category).
    pub material_nodes: Vec<Arc<NodeType>>,
    /// Materials to start from.
    pub material_presets: Vec<MaterialPreset>,
    /// The outputs it can write, in the order a UI and a file list them
    /// (the beauty first).
    pub aovs: Vec<AovSpec>,
    /// Whether each pass is a whole frame, fast enough to follow a moving
    /// camera at full size: hosts then never shrink a moving view's film.
    pub realtime: bool,
    /// How it renders material previews.
    pub preview: PreviewSetup,
    /// What its materials are to others.
    pub materials: Arc<dyn MaterialRules>,
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

impl EngineDescription {
    /// Its node type `name`.
    pub fn node_type(&self, name: &str) -> Option<&Arc<NodeType>> {
        self.material_nodes.iter().find(|t| &*t.name == name)
    }

    /// Its surface node types (a material's types), in order.
    pub fn surfaces(&self) -> impl Iterator<Item = &Arc<NodeType>> {
        self.material_nodes.iter().filter(|t| t.surface.is_some())
    }

    /// Its output `id` ([`Aov::id`]).
    pub fn aov(&self, id: &str) -> Option<&Aov> {
        self.aovs.iter().map(|a| &a.aov).find(|a| &*a.id == id)
    }

    /// The outputs new Render Settings ask for.
    pub fn default_aovs(&self) -> Vec<Aov> {
        self.aovs
            .iter()
            .filter(|a| a.default)
            .map(|a| a.aov.clone())
            .collect()
    }

    /// Its setting `name`'s descriptor.
    pub fn setting(&self, name: &str) -> Option<&SettingDescriptor> {
        self.settings.iter().find(|d| &*d.name == name)
    }

    /// Its prim type `id`, when it declares it.
    pub fn prim_type(&self, id: &str) -> Option<&PrimType> {
        self.types.iter().find(|t| &*t.id == id)
    }

    /// The schemas it applies to type `id`.
    pub fn applied_to<'a>(&'a self, id: &'a str) -> impl Iterator<Item = &'a AppliedSchema> {
        self.applied.iter().filter(move |a| a.applies(id))
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

    /// What one of its materials shows as elsewhere.
    pub fn tint(&self, network: &MaterialNetwork) -> Option<Tint> {
        self.materials.tint(network)
    }

    /// Whether one of its materials fills its mesh as a volume.
    pub fn volume(&self, network: &MaterialNetwork) -> bool {
        self.materials.volume(network)
    }

    /// A material of its own showing `tint`.
    pub fn flat(&self, tint: &Tint) -> MaterialNetwork {
        self.materials.flat(tint)
    }

    /// The material it renders what names no material with.
    pub fn default_material(&self) -> MaterialNetwork {
        self.materials.default_material()
    }

    /// One of its materials as a preview surface.
    pub fn preview_surface(&self, network: &MaterialNetwork) -> PreviewSurface {
        self.materials.preview_surface(network)
    }

    /// A material of its own from a preview surface.
    pub fn import_preview_surface(&self, surface: &PreviewSurface) -> MaterialNetwork {
        self.materials.import_preview_surface(surface)
    }

    /// `network` (a material of engine `owner`) as this engine renders
    /// it: as it is when it is this engine's, else flat in the tint its
    /// owner gives it, or this engine's default material when that gives
    /// none.
    pub fn adopt(&self, network: MaterialNetwork, owner: &EngineDescription) -> MaterialNetwork {
        if network.engine == self.id {
            return network;
        }
        owner
            .tint(&network)
            .map_or_else(|| self.default_material(), |tint| self.flat(&tint))
    }
}

/// The engines a host can make, in the order it lists them; the first is
/// its default.
#[derive(Clone, Debug)]
pub struct Engines(Arc<[Arc<EngineDescription>]>);

impl Engines {
    /// The engines `descriptions` describe (at least one).
    pub fn new(descriptions: Vec<EngineDescription>) -> Engines {
        assert!(!descriptions.is_empty(), "a host has an engine");
        Engines(descriptions.into_iter().map(Arc::new).collect())
    }

    /// Engine `id`.
    pub fn get(&self, id: &str) -> Option<&Arc<EngineDescription>> {
        self.0.iter().find(|e| &*e.id == id)
    }

    /// The default engine: the first.
    pub fn default_engine(&self) -> &Arc<EngineDescription> {
        &self.0[0]
    }

    /// Every engine, in order.
    pub fn iter(&self) -> impl Iterator<Item = &Arc<EngineDescription>> {
        self.0.iter()
    }
}
