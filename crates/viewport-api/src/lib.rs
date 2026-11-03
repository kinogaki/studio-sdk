//! Kinogaki's viewport packet contract: what a data type hands Studio's
//! viewport to draw, with no GPU or UI API.
//!
//! - [`SurfaceSource`]: the packet a kernel (meshes, CAD, SDF…) provides
//!   for drawing — shared point positions, vertex shading records,
//!   triangles, line classes ([`LineClass`]: edges, hulls, curves… each
//!   with its own look and ids) and point markers in point classes
//!   ([`PointClass`]: points, control vertices, B-rep vertices…), with
//!   sparse [`DrawUpdate`] ranges; a [`ShadingPlan`] lets the renderer
//!   derive vertex shading from positions; [`FaceRun`]s draw some faces
//!   see-through ([`FaceLook`]). A kernel reads its own packets back with
//!   [`packet_as`].
//! - [`pack_linear`]: a linear color as the RGB9E5 word surface colors
//!   are carried in ([`ColorLayout`]).
//! - [`ElementSet`]: the selected points, edges, faces, vertices or
//!   objects; [`ElementSelection`]: one set per drawn family (faces,
//!   lines, markers) for surfaces whose levels combine.
//! - [`PreviewUpdate`] patches and GPU motions for live transform previews.
//! - [`OverlayDraw`] / [`GuideDraw`] layers for diagnostics and tool guides,
//!   [`FaceNormals`], and [`SceneLines`] for glyphs placed in the world
//!   (cameras, lights).
//!
//! Points and directions are the SDK's [`Vec3`](kinogaki_geometry::Vec3),
//! placements its [`Xform`](kinogaki_geometry::Xform) and quaternions
//! `[x, y, z, w]`; the GPU's f32 records are made where they are uploaded.
//!
//! The `bytemuck` feature derives `bytemuck::Pod` on the records a renderer
//! uploads as bytes ([`Vertex`], [`OverlayVertex`], [`OverlayLine`]).
//! Besides it, the crate depends on the SDK's geometry vocabulary alone.
mod color;
mod lines;
mod looks;
mod overlay;
mod packet;
mod points;
mod preview;
mod selection;
mod shading;
mod updates;
pub use color::pack_linear;
pub use lines::{ACCENT, LineClass, LineClassId, LineHighlight, LineLook, LineStyle};
pub use looks::{FaceLook, FaceRun, check_runs};
pub use overlay::{
    FaceNormals, GuideDraw, LineLayer, OverlayDraw, OverlayLayer, OverlayLine, SceneLines,
};
pub use packet::{
    ColorLayout, Column, ElementKind, ElementSet, OverlayVertex, SurfaceSource, Vertex, packet_as,
};
pub use points::{PointClass, PointClassId, PointLook, PointShape, PointStyle};
pub use preview::{MotionMask, PartMotion, Patch, PointWeights, PreviewUpdate, WeightedMotion};
pub use selection::ElementSelection;
pub use shading::{
    AuthoredNormals, FaceVertices, Link, NormalClass, PointFans, ShadingPlan, Smoothing,
};
pub use updates::{BufferRanges, DrawUpdate, UpdateRanges};
