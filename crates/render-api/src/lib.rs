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
//! material vocabulary ([`vocabulary`]), presets and outputs ([`Aov`]s,
//! by id). Materials are networks of one
//! engine's nodes ([`MaterialNetwork`]); an engine renders another's flat
//! in its tint ([`EngineDescription::adopt`]). Engines output linear ACEScg.
//! Hosts make mesh prims from [`TriangleSource`]s: plain triangles, from a
//! polygon mesh, a CAD tessellation or a CAM preview alike.
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
pub mod exchange;
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
pub mod vocabulary;

pub use aov::{Aov, AovClass, AovDesc, AovFormat, AovSpec};

/// The space engines take and output: linear ACEScg. Authored colors are
/// in the project's working space; the host converts them to this as it
/// sends them, through its color service.
pub const RENDER_SPACE: &str = "ACEScg";
pub use declared::{AppliedSchema, Declared, PrimType, Role};
pub use description::{
    EngineDescription, Engines, MaterialPreset, MaterialRules, MotionSupport, PreviewSetup, Tint,
};
pub use dirty::Dirty;
pub use engine::{
    Deliver, Frame, Outputs, Pending, RenderEngine, RenderStats, ViewCamera, ViewDesc, ViewId,
    ViewMode, Wake, pending,
};
pub use exchange::{Extra, ImportedMaterial, PreviewSurface};
pub use id::{EngineId, IdAllocator, MaterialId, PrimId, TextureId};
/// The standard geometry values, which engines read prims' data as.
pub use kinogaki_geometry as geometry;
/// How fine to make geometry for a render: the standard geometry values'.
pub use kinogaki_geometry::Detail;
pub use kinogaki_value as value;
pub use material::{ImageUse, Link, MaterialNetwork, NodeIndex, ShaderNode};
pub use prim::{
    AttributeDomain, Camera, Instancer, Material, Mesh, MeshAttribute, Numbers, Prim, PrimKind,
    Projection, TexelFormat, Texture, TextureColorSpace,
};
pub use ranges::Ranges;
pub use recording::{Counts, RecordingEngine};
pub use settings::{RenderSettings, SettingDescriptor, SettingGroup, Textures, Trailing};
pub use source::{Look, PlainTriangles, TriangleSource};
pub use update::{Change, SceneUpdate};
pub use vocabulary::{Layout, NodeLayout, NodeType, ParamSpec, SocketKind, SocketSpec};

#[cfg(test)]
mod tests;
