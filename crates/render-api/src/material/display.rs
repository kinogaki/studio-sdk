//! [`MaterialDisplay`]: what the shaded viewport shows of a material, a simple
//! approximation its engine makes of its own material
//! ([`MaterialRules::display`](crate::MaterialRules::display)). Render views
//! show the real look.

use kinogaki_geometry::names;
use std::sync::Arc;

/// The image a material's color shows: its file, as a node of the material
/// holds it, and the coordinates it is read through.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisplayTexture {
    /// The file, as typed: relative paths read from the project's folder. The
    /// host reads it as a color (the color service's file rules) and shows it
    /// once read.
    pub file: Arc<str>,
    /// The primvar holding its coordinates.
    pub uv: Arc<str>,
}

impl DisplayTexture {
    /// `file`, read through the standard `uv`.
    pub fn new(file: &str) -> DisplayTexture {
        DisplayTexture {
            file: Arc::from(file),
            uv: Arc::from(names::UV),
        }
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
