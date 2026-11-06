//! Kinogaki's render interface: what a host (Studio) sends a render engine
//! and what comes back, with no GPU or OS API.
//!
//! It is Hydra's split made small. The host pushes typed prims ([`Prim`]) in
//! batches ([`SceneUpdate`]) with dirty bits ([`Dirty`]) and point ranges
//! ([`Ranges`]); batches an engine has not applied yet coalesce
//! ([`SceneUpdate::merge`]). Engines implement [`RenderEngine`], publish
//! whole passes as [`Frame`]s and wake the host once per result ([`Wake`]).
//! Each engine describes itself without a device ([`EngineDescription`]):
//! its settings ([`SettingDescriptor`]s), the prim types it declares
//! (lights and fog, [`PrimType`]s; the [`standard`] ones shared), its
//! material nodes ([`NodeDef`]s) and outputs ([`Aov`]s, by id). The crate
//! holds no shading model: materials are networks of engine-declared nodes
//! ([`MaterialNetwork`]), which an engine translates to its shaders and
//! approximates for the viewport ([`MaterialRules`]), and renders a
//! [`PreviewImage`] of. Engines output linear ACEScg.
//! Hosts make shape prims from [`ShapeSource`]s: triangles or curves, from a
//! polygon value, a CAD tessellation or a toolpath preview alike.
//! [`RecordingEngine`] stores what it is sent, for hosts' tests.
//! Every value a setting, a declared prim or a material port holds is the
//! SDK's one value model ([`value`]: `Value`, `ValueKind`).
//!
//! The `wgpu` feature adds the `display` module: how a wgpu host
//! shows an engine's frames as textures on its own device and makes
//! engines there (wgpu 30.0.1, on every platform wgpu supports). Without
//! it the crate depends on the standard library and the SDK's standard
//! geometry values (`geometry`, which also holds [`Detail`]) alone.

pub mod aov;
pub mod declared;
pub mod description;
pub mod dirty;
#[cfg(feature = "wgpu")]
pub mod display;
pub mod engine;
pub mod id;
pub mod material;
pub mod prim;
pub mod ranges;
pub mod recording;
pub mod settings;
pub mod source;
pub mod standard;
pub mod trace;
pub mod update;

pub use aov::{Aov, AovClass, AovDesc, AovFormat, AovSpec};

/// The space engines take and output: linear ACEScg. Authored colors are
/// in the project's working space; the host converts them to this as it
/// sends them, through its color service.
pub const RENDER_SPACE: &str = "ACEScg";
pub use declared::{Declared, PrimType, Role};
pub use description::{
    Capabilities, EngineBuilder, EngineDescription, Engines, MaterialRules, Motion,
};
pub use dirty::Dirty;
pub use engine::{
    Deliver, FinalOutputs, Frame, Pending, RenderEngine, RenderStats, ViewCamera, ViewDesc, ViewId,
    ViewMode, Wake, pending,
};
pub use id::{Id, IdAllocator, PrimId};
/// The standard geometry values, which engines read prims' data as.
pub use kinogaki_geometry as geometry;
/// How fine to make geometry for a render: the standard geometry values'.
pub use kinogaki_geometry::Detail;
/// Half floats, as [`Frame::pixels`] and Texture prims hold them.
pub use kinogaki_geometry::image::{f32_to_half, half_to_f32};
pub use kinogaki_value as value;
pub use kinogaki_value::EngineId;
pub use material::preview::{
    PreviewImage, PreviewLighting, PreviewRequest, PreviewScene, PreviewShape,
};
pub use material::{
    DisplayTexture, Input, Link, MaterialDisplay, MaterialNetwork, NetworkBuilder, Node, NodeDef,
    NodeIndex, Output, Socket,
};
pub use prim::{
    Camera, Instancer, InstancerBuilder, Material, Prim, PrimKind, Projection, STANDARD_PRIMVARS,
    Shape, ShapeBuilder, Subsets, Topology, Visibility,
};
pub use ranges::Ranges;
pub use recording::{Counts, RecordingEngine};
pub use settings::{SceneSettings, SettingDescriptor, SettingGroup};
pub use source::{PlainTriangles, ShapeSource};
pub use update::{Change, SceneUpdate};

#[cfg(test)]
mod tests;
