//! The standard fog: exponential height fog, a schema and a typed value.
use super::FOG;
use crate::declared::{Declared, PrimType, Role};
use kinogaki_value::{Condition, Parameter, Schema, Section, ValueKind};

/// Exponential height fog: its density `density × e^(−falloff × (h −
/// height))` at height `h` along the project's up axis, from
/// `start` along each view ray on (falloff 0: even distance fog). A scene
/// has at most one.
pub fn fog() -> PrimType {
    const GROUP: &str = "fog";
    let params = [
        Parameter::slider("density", "Density", (0.0, 1e6), (0.0, 1.0), 3, 0.1).group(GROUP),
        Parameter::new("color", "Color", ValueKind::Color, [0.3, 0.34, 0.42]).group(GROUP),
        Parameter::length("start", "Start", (0.0, 1e6), None, 3, 0.0).group(GROUP),
        Parameter::length("height", "Height", (-1e6, 1e6), None, 3, 0.0).group(GROUP),
        Parameter::slider("falloff", "Falloff", (0.0, 1e6), (0.0, 4.0), 3, 0.5).group(GROUP),
        Parameter::toggle("inscatter", "In-scatter", false).group(GROUP),
        Parameter::slider("anisotropy", "Anisotropy", (-0.9, 0.9), (-0.9, 0.9), 3, 0.3)
            .group(GROUP)
            .requires(Condition::when("inscatter", true)),
    ];
    let schema = Schema::default().section(Section::new(GROUP, "Fog", true));
    PrimType::new(
        FOG,
        "Fog",
        Role::Atmosphere,
        params.into_iter().fold(schema, Schema::parameter),
    )
}

/// A standard fog ([`FOG`]) as a typed value: exponential height fog, its
/// values within [`fog`]'s limits as a host sends them.
#[derive(Clone, Debug, PartialEq)]
pub struct Fog {
    /// Extinction per scene unit at [`height`](Self::height).
    pub density: f32,
    /// Its color, linear (ACEScg in a prim an engine receives).
    pub color: [f32; 3],
    /// How far from the camera it begins.
    pub start: f32,
    /// Where it has its density; above, it thins by
    /// [`falloff`](Self::falloff) per unit.
    pub height: f32,
    /// How fast it thins with height.
    pub falloff: f32,
    /// It scatters the lights' light toward the camera.
    pub inscatter: bool,
    /// Its phase's anisotropy, with in-scatter (−0.9 back to 0.9 forward).
    pub anisotropy: f32,
}

impl Fog {
    /// `prim` as a standard fog (none when it is of another type).
    pub fn decode(prim: &Declared) -> Option<Fog> {
        (prim.type_id == FOG).then(|| Fog {
            density: prim.float("density"),
            color: prim.color("color"),
            start: prim.float("start"),
            height: prim.float("height"),
            falloff: prim.float("falloff"),
            inscatter: prim.bool("inscatter"),
            anisotropy: prim.float("anisotropy"),
        })
    }
}
