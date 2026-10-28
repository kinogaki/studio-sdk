//! Outputs (AOVs) a final frame can carry, known by id. An engine lists
//! the ones it writes ([`crate::EngineDescription::aovs`]); the standard
//! ones ([`standard`]) are the contract hosts and compositing read by.
//! Colors (the beauty, albedo and the light passes) are linear ACEScg
//! whatever the engine.

use std::sync::Arc;

/// Which prims a Cryptomatte output separates.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CryptomatteOf {
    /// Objects (Mesh prim paths).
    Object,
    /// Materials (Material prim paths).
    Material,
}

/// What an output's values are, and so how many a pixel takes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AovClass {
    /// A color and its alpha (RGBA, linear ACEScg).
    Rgba,
    /// A color (RGB, linear ACEScg).
    Color,
    /// A vector (a normal, a position).
    Vector,
    /// One number.
    Scalar,
    /// One integer id, stored as `u32` bits.
    Id,
    /// Cryptomatte with `levels` rank slots a pixel (an ID and a coverage
    /// each).
    Cryptomatte {
        /// What it separates.
        of: CryptomatteOf,
        /// Rank slots a pixel.
        levels: u8,
    },
}

/// An output: its id in files (a project's outputs, an EXR's part), what a
/// UI calls it and what its values are.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Aov {
    /// Its id (`beauty`, `CryptoObject`).
    pub id: Arc<str>,
    /// What a UI calls it.
    pub label: Arc<str>,
    /// What its values are.
    pub class: AovClass,
}

impl Aov {
    /// Output `id`, labelled `label`, of `class`.
    pub fn new(id: &str, label: &str, class: AovClass) -> Aov {
        Aov {
            id: Arc::from(id),
            label: Arc::from(label),
            class,
        }
    }

    /// Values a pixel ([`crate::Outputs`]' interleaving): RGBA 4, other
    /// colors and vectors 3, Cryptomatte 2 a level, the rest 1.
    pub fn channels(&self) -> usize {
        match self.class {
            AovClass::Rgba => 4,
            AovClass::Color | AovClass::Vector => 3,
            AovClass::Cryptomatte { levels, .. } => 2 * usize::from(levels),
            AovClass::Scalar | AovClass::Id => 1,
        }
    }

    /// Whether it holds colors (linear ACEScg; its first three channels).
    pub fn is_color(&self) -> bool {
        matches!(self.class, AovClass::Rgba | AovClass::Color)
    }

    /// Whether it holds integers (ids).
    pub fn is_id(&self) -> bool {
        self.class == AovClass::Id
    }

    /// Whether it is the beauty.
    pub fn is_beauty(&self) -> bool {
        &*self.id == standard::BEAUTY
    }
}

/// The standard outputs: their ids, and each one's [`Aov`]. An engine that
/// writes one of these writes it as described here.
pub mod standard {
    use super::{Aov, AovClass, CryptomatteOf};

    /// The image.
    pub const BEAUTY: &str = "beauty";
    /// Albedo at the first non-specular hit.
    pub const ALBEDO: &str = "albedo";
    /// Light arriving straight from the lights, off a white diffuse surface.
    pub const DIRECT: &str = "direct";
    /// Ambient light (a dome's, an ambient color's), off a white diffuse
    /// surface.
    pub const AMBIENT: &str = "ambient";
    /// Indirect light (global illumination), off a white diffuse surface.
    pub const INDIRECT: &str = "indirect";
    /// Light scattered toward the camera by volumes and lit fog (R G B) and
    /// their opacity (A: 1 minus their transmittance), premultiplied: what
    /// lies behind shows as itself × (1 − A) + R G B.
    pub const VOLUME: &str = "volume";
    /// Ambient occlusion: the open share of the hemisphere (1 open, 0
    /// closed).
    pub const OCCLUSION: &str = "occlusion";
    /// Shading normal at the first non-specular hit.
    pub const NORMAL: &str = "normal";
    /// Camera distance at the first hit.
    pub const DEPTH: &str = "depth";
    /// World position at the first hit.
    pub const POSITION: &str = "position";
    /// The Mesh prim's `id` at the first hit.
    pub const OBJECT_ID: &str = "objectId";
    /// The material's slot at the first hit.
    pub const MATERIAL_ID: &str = "materialId";
    /// Cryptomatte of objects.
    pub const CRYPTO_OBJECT: &str = "CryptoObject";
    /// Cryptomatte of materials.
    pub const CRYPTO_MATERIAL: &str = "CryptoMaterial";
    /// Samples each pixel took.
    pub const SAMPLES: &str = "samples";
    /// Each pixel's estimated variance.
    pub const VARIANCE: &str = "variance";

    /// The beauty.
    pub fn beauty() -> Aov {
        Aov::new(BEAUTY, "Beauty", AovClass::Rgba)
    }

    /// Every standard output but Cryptomatte, whose levels are an
    /// engine's ([`cryptomatte`]), in the order a UI lists them.
    pub fn all() -> Vec<Aov> {
        [
            (BEAUTY, "Beauty", AovClass::Rgba),
            (ALBEDO, "Albedo", AovClass::Color),
            (DIRECT, "Direct Light", AovClass::Color),
            (AMBIENT, "Ambient Light", AovClass::Color),
            (INDIRECT, "Indirect Light", AovClass::Color),
            (VOLUME, "Volume", AovClass::Rgba),
            (OCCLUSION, "Ambient Occlusion", AovClass::Scalar),
            (NORMAL, "Normal", AovClass::Vector),
            (DEPTH, "Depth", AovClass::Scalar),
            (POSITION, "Position", AovClass::Vector),
            (OBJECT_ID, "Object ID", AovClass::Id),
            (MATERIAL_ID, "Material ID", AovClass::Id),
            (SAMPLES, "Sample Count", AovClass::Scalar),
            (VARIANCE, "Variance", AovClass::Scalar),
        ]
        .into_iter()
        .map(|(id, label, class)| Aov::new(id, label, class))
        .collect()
    }

    /// Standard output `id` (none for Cryptomatte and ids no standard
    /// output has).
    pub fn get(id: &str) -> Option<Aov> {
        all().into_iter().find(|a| &*a.id == id)
    }

    /// Cryptomatte of `of` with `levels` rank slots a pixel.
    pub fn cryptomatte(of: CryptomatteOf, levels: u8) -> Aov {
        let (id, label) = match of {
            CryptomatteOf::Object => (CRYPTO_OBJECT, "Cryptomatte Object"),
            CryptomatteOf::Material => (CRYPTO_MATERIAL, "Cryptomatte Material"),
        };
        Aov::new(id, label, AovClass::Cryptomatte { of, levels })
    }
}

/// An output an engine writes, and whether new Render Settings ask for it.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AovSpec {
    /// The output.
    pub aov: Aov,
    /// Asked for by default.
    pub default: bool,
}

impl AovSpec {
    /// Output `aov`, asked for by default when `default`.
    pub fn new(aov: Aov, default: bool) -> AovSpec {
        AovSpec { aov, default }
    }
}

/// How an output's values are stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum AovFormat {
    /// Half floats.
    F16,
    /// Floats.
    F32,
    /// Unsigned integers.
    U32,
}

/// An output and its storage.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct AovDesc {
    /// Which output.
    pub aov: Aov,
    /// Its storage.
    pub format: AovFormat,
}
