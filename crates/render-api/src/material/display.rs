//! [`MaterialDisplay`]: what the shaded viewport shows of a material, a simple
//! approximation its engine makes of its own material
//! ([`MaterialRules::display`](crate::MaterialRules::display)): a color, a
//! primvar and an image, read through its texture's transform and sampling.
//! Render views show the real look.

use kinogaki_geometry::image::Sampling;
use kinogaki_geometry::names;
use std::sync::Arc;

/// The image a material's color shows: its file, as a node of the material
/// holds it, the coordinates it is read through and how.
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayTexture {
    /// The file, as typed: relative paths read from the project's folder. The
    /// host reads it as a color (the color service's file rules) and shows it
    /// once read.
    pub file: Arc<str>,
    /// The primvar holding its coordinates.
    pub uv: Arc<str>,
    /// Where a point of coordinates `(u, v)` reads the image: `transform` ·
    /// `(u, v, 1)`, by rows.
    pub transform: [[f32; 3]; 2],
    /// How it is read past its edges and between its texels.
    pub sampling: Sampling,
}

impl DisplayTexture {
    /// `file`, read through the standard `uv` as it is.
    pub fn new(file: &str) -> DisplayTexture {
        DisplayTexture {
            file: Arc::from(file),
            uv: Arc::from(names::UV),
            transform: [[1.0, 0.0, 0.0], [0.0, 1.0, 0.0]],
            sampling: Sampling::default(),
        }
    }

    /// Where a point of coordinates `uv` reads the image.
    pub fn at(&self, [u, v]: [f32; 2]) -> [f32; 2] {
        self.transform.map(|[a, b, c]| a * u + b * v + c)
    }
}

/// A material as the shaded viewport shows it: `color` times `primvar` times
/// `texture`, all linear.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialDisplay {
    /// Its color, linear in the working space, as the network it approximates
    /// is authored.
    pub color: [f32; 3],
    /// The primvar its color is multiplied by, where a shape has it (`Cd`).
    pub primvar: Option<Arc<str>>,
    /// The image its color multiplies by, where a shape has the coordinates
    /// and once the file is read; before that, its color alone.
    pub texture: Option<DisplayTexture>,
    /// Whether it fills its shape as a volume (a participating medium) rather
    /// than shading its surface: the viewport shows such a shape as a
    /// see-through ghost of its bounds.
    pub volume: bool,
}

impl MaterialDisplay {
    /// A surface of one color.
    pub fn color(color: [f32; 3]) -> MaterialDisplay {
        MaterialDisplay {
            color,
            primvar: None,
            texture: None,
            volume: false,
        }
    }
}
