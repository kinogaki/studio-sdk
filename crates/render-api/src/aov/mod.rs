//! Outputs (AOVs) a final frame can carry, known by id. An engine lists
//! the ones it writes ([`crate::Capabilities::aovs`]); the standard
//! ones ([`standard`]) are the contract hosts and compositing read by.
//! Colors (the beauty, albedo and the light passes) are linear ACEScg
//! whatever the engine. A final frame is one [`Image`], each output its
//! layers ([`image`]).
pub mod cryptomatte;
#[cfg(test)]
mod tests;

use crate::RENDER_SPACE;
use crate::geometry::Error;
use crate::geometry::image::{Encoding, Format, Image, Rect, Sample, f32_to_half};
use std::sync::Arc;

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

    /// Values a pixel, as an engine hands them to [`image`]: RGBA 4, other
    /// colors and vectors 3, Cryptomatte 2 a level, the rest 1.
    pub fn channels(&self) -> usize {
        match self.class {
            AovClass::Rgba => 4,
            AovClass::Color | AovClass::Vector => 3,
            AovClass::Cryptomatte { levels } => 2 * usize::from(levels),
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

    /// Its channels' names in its layer (Cryptomatte's are its own).
    fn channel_names(&self) -> &'static [&'static str] {
        match self.class {
            AovClass::Rgba => &["R", "G", "B", "A"],
            AovClass::Color => &["R", "G", "B"],
            AovClass::Vector => &["X", "Y", "Z"],
            AovClass::Scalar if &*self.id == standard::DEPTH => &["Z"],
            AovClass::Scalar => &["Y"],
            AovClass::Id => &["id"],
            AovClass::Cryptomatte { .. } => &["R", "G", "B", "A"],
        }
    }
}

/// The layer of a frame's Image output `id` fills: `""` the beauty's, else
/// the id itself (a Cryptomatte's numbered on from it: [`cryptomatte`]).
pub fn layer(id: &str) -> &str {
    if id == standard::BEAUTY { "" } else { id }
}

/// The output whose layer is `layer` ([`layer`]'s inverse).
pub fn output(layer: &str) -> &str {
    if layer.is_empty() {
        standard::BEAUTY
    } else {
        layer
    }
}

/// An output's values as an engine holds them: interleaved
/// [`Aov::channels`] a pixel, rows from the top. Floats (a Cryptomatte's
/// ids as their bits), or whole numbers (object and material ids).
#[derive(Clone, Debug, PartialEq)]
pub enum Values {
    /// Floats.
    Floats(Arc<[f32]>),
    /// Whole numbers.
    Whole(Arc<[u32]>),
}

impl Values {
    fn len(&self) -> usize {
        match self {
            Values::Floats(v) => v.len(),
            Values::Whole(v) => v.len(),
        }
    }
}

/// A final frame's outputs as one Image of `size` pixels (rows from the top):
/// each of `planes` (an output, and its values) as its layers, in order,
/// tiled once from the engine's own values (half floats made where they are
/// asked for), and the Cryptomatte manifests naming those of `names` its
/// layers hold as metadata. The layers:
///
/// | Output | Layer | Channels | Sample, encoding |
/// |---|---|---|---|
/// | the beauty, volume (RGBA) | `""`, `volume` | `R G B A` | as asked; colors in [`RENDER_SPACE`], premultiplied |
/// | albedo, the light passes (color) | their ids | `R G B` | as asked; colors in [`RENDER_SPACE`] |
/// | normal, position (vectors) | their ids | `X Y Z` | as asked; raw |
/// | depth, and other numbers | `depth`, their ids | `Z`, `Y` | as asked; raw |
/// | object and material ids | their ids | `id` | U32; raw |
/// | Cryptomatte | `<id>00`, `<id>01`… | `R G B A` | F32; raw |
///
/// # Errors
/// Values not of the size's pixels, ids that aren't whole numbers or a
/// Cryptomatte that isn't floats, or a size of none.
pub fn image(
    size: [u32; 2],
    planes: &[(AovDesc, Values)],
    names: &[Arc<str>],
) -> Result<Image, Error> {
    let wide = |v: u32| i32::try_from(v).map_err(|_| Error::invalid("a frame too large"));
    let window = Rect::new([0, 0], [wide(size[0])?, wide(size[1])?])?;
    let pixels = size[0] as usize * size[1] as usize;
    let mut builder = Image::builder(window);
    let mut metadata = crate::geometry::Attributes::new();
    for (desc, values) in planes {
        let aov = &desc.aov;
        if values.len() != pixels * aov.channels() {
            return Err(Error::invalid(format_args!(
                "output {} holds {} values over {pixels} pixels",
                aov.id,
                values.len()
            )));
        }
        let unlike = || Error::invalid(format_args!("output {} holds other values", aov.id));
        if let AovClass::Cryptomatte { levels } = aov.class {
            let Values::Floats(values) = values else {
                return Err(unlike());
            };
            for set in 0..usize::from(levels).div_ceil(2) {
                let format = Format {
                    name: format!("{}{set:02}", aov.id).into(),
                    channels: names_of(aov),
                    sample: Sample::F32,
                    encoding: Encoding::Raw,
                };
                let ranks = 2 * usize::from(levels);
                let set: Vec<f32> = (values.chunks_exact(ranks))
                    .flat_map(|px| (0..4).map(move |k| px.get(set * 4 + k).copied().unwrap_or(0.0)))
                    .collect();
                builder = builder.plane(format, &set);
            }
            metadata = cryptomatte::manifest(metadata, &aov.id, names, values)?;
            continue;
        }
        let sample = match (aov.class, desc.format) {
            (AovClass::Id, _) | (_, AovFormat::U32) => Sample::U32,
            (_, AovFormat::F16) => Sample::F16,
            (_, AovFormat::F32) => Sample::F32,
        };
        let format = Format {
            name: layer(&aov.id).into(),
            channels: names_of(aov),
            sample,
            encoding: match aov.class {
                AovClass::Rgba => Encoding::color(RENDER_SPACE, true),
                AovClass::Color => Encoding::color(RENDER_SPACE, false),
                _ => Encoding::Raw,
            },
        };
        builder = match (sample, values) {
            (Sample::U32, Values::Whole(v)) => builder.plane(format, &v[..]),
            (Sample::F32, Values::Floats(v)) => builder.plane(format, &v[..]),
            (Sample::F16, Values::Floats(v)) => {
                let halves: Vec<u16> = v.iter().map(|v| f32_to_half(*v)).collect();
                builder.plane(format, &halves)
            }
            _ => return Err(unlike()),
        };
    }
    builder.attributes(metadata).build()
}

/// `aov`'s channel names, as a layer's format holds them.
fn names_of(aov: &Aov) -> Arc<[Arc<str>]> {
    aov.channel_names().iter().map(|c| Arc::from(*c)).collect()
}

/// The standard outputs: their ids, and each one's [`Aov`]. An engine that
/// writes one of these writes it as described here.
pub mod standard {
    use super::{Aov, AovClass};

    /// The image.
    pub const BEAUTY: &str = "beauty";
    /// Albedo at the first non-specular hit.
    pub const ALBEDO: &str = "albedo";
    /// Light arriving straight from the lights, off a white diffuse surface.
    pub const DIRECT: &str = "direct";
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
    /// The shape's id at the first hit (the engine's own, one a shape or
    /// instance); `u32::MAX` where the camera ray hits nothing.
    pub const OBJECT_ID: &str = "objectId";
    /// The material at the first hit, by its Material prim: its prim
    /// slot plus one, one id a material across every mesh, never a mesh's
    /// own slot index; 0 for the engine's default material, `u32::MAX`
    /// where the camera ray hits nothing.
    pub const MATERIAL_ID: &str = "materialId";
    /// Samples each pixel took.
    pub const SAMPLES: &str = "samples";
    /// Each pixel's estimated variance.
    pub(crate) const VARIANCE: &str = "variance";

    /// The beauty.
    pub fn beauty() -> Aov {
        Aov::new(BEAUTY, "Beauty", AovClass::Rgba)
    }

    /// Every standard output, in the order a UI lists them.
    pub fn all() -> Vec<Aov> {
        [
            (BEAUTY, "Beauty", AovClass::Rgba),
            (ALBEDO, "Albedo", AovClass::Color),
            (DIRECT, "Direct Light", AovClass::Color),
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

    /// Standard output `id` (none for an id no standard output has).
    pub fn get(id: &str) -> Option<Aov> {
        all().into_iter().find(|a| &*a.id == id)
    }
}

/// An output an engine writes, and whether a new Render Output asks for it.
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
