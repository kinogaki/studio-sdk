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
//! [`ramp`] is every ramp's encoding and maths: a node's ramp parameter,
//! the inspector's ramp widget and an engine's color ramps evaluate alike.
//!
//! The `wgpu` feature adds the `display` module: how a wgpu host
//! shows an engine's frames as textures on its own device and makes
//! engines there (wgpu 30.0.1, on every platform wgpu supports). Without
//! it the crate depends on the standard library alone.

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
pub mod ramp;
pub mod ranges;
pub mod recording;
pub mod settings;
pub mod source;
pub mod standard;
pub mod trace;
pub mod update;
pub mod vocabulary;

pub use aov::{Aov, AovClass, AovDesc, AovFormat, AovSpec, CryptomatteOf};
pub use declared::{AppliedSchema, Declared, PrimType, Role};
pub use description::{
    EngineDescription, Engines, MaterialPreset, MaterialRules, PreviewSetup, Tint,
};
pub use dirty::Dirty;
pub use engine::{
    Deliver, Frame, Outputs, Pending, RenderEngine, RenderStats, ViewCamera, ViewDesc, ViewId,
    ViewMode, Wake, pending,
};
pub use exchange::{Extra, PreviewSurface};
pub use id::{IdAllocator, MaterialId, PrimId, TextureId};
pub use material::{
    Constant, ImageUse, Link, MaterialNetwork, NetworkError, NodeIndex, ShaderNode,
};
pub use prim::{
    AttributeDomain, Camera, IDENTITY, Instancer, Material, Matrix, Mesh, MeshAttribute, Prim,
    PrimKind, Projection, TexelFormat, Texture, TextureColorSpace,
};
pub use ranges::Ranges;
pub use recording::{Counts, RecordingEngine};
pub use settings::{
    ColorStop, Condition, MAX_RAMP_STOPS, RenderSettings, Requirement, SettingDescriptor,
    SettingError, SettingGroup, SettingKind, SettingValue, SettingValues, Trailing, Unit,
    Visibility, ramp_color,
};
pub use source::{Look, PlainTriangles, TriangleSource};
pub use update::{Change, SceneUpdate};
pub use vocabulary::{
    Choice, InputEdit, Layout, NodeLayout, NodeType, ParamKind, ParamSpec, ParamValue, Params,
    SocketKind, SocketSpec,
};
