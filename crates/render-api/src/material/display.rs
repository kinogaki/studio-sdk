//! [`MaterialDisplay`]: what the shaded viewport shows of a material, a simple
//! approximation its engine makes of its own material
//! ([`MaterialRules::display`](crate::MaterialRules::display)). Render views
//! show the real look.

use std::sync::Arc;

/// A material as the shaded viewport shows it: `color` times `primvar`.
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialDisplay {
    /// Its color, linear in the working space, as the network it approximates
    /// is authored.
    pub color: [f32; 3],
    /// The primvar its color is multiplied by, where a shape has it (`Cd`).
    pub primvar: Option<Arc<str>>,
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
            volume: false,
        }
    }
}
