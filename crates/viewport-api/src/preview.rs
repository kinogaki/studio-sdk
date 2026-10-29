//! Sparse, reversible preview patches over an uploaded packet: a transform
//! drag rewrites only affected positions, vertices and triangles. A rigid
//! move (rotation, translation, uniform scale) is drawn by the GPU instead:
//! an affine transform of the whole surface, or of the points flagged in a
//! [`MotionMask`], with patches only for the faces that stretch between moved
//! and fixed points.
use crate::{SurfaceSource, Vertex};
use std::sync::Arc;

/// Values replacing `start..start + values.len()` of a column.
#[derive(Debug)]
pub struct Patch<T> {
    /// The first element replaced.
    pub start: usize,
    /// The new values.
    pub values: Vec<T>,
}
/// Two bits per point, built once when a gesture starts: bit 0 marks a point
/// the preview matrix moves; bit 1 marks a moved point whose surrounding
/// faces all move with it, so its vertices' normals and curvature turn with
/// the matrix. Vertices of other affected points arrive as patches, already
/// in their final space.
#[derive(Debug, Default)]
pub struct MotionMask {
    words: Vec<u32>,
}
impl MotionMask {
    /// The point moves with the preview matrix.
    pub const MOVED: u32 = 1;
    /// The point moves, and every face around it moves with it.
    pub const RIGID: u32 = 2;
    /// A mask of `points` points, none flagged.
    pub fn new(points: usize) -> Self {
        Self {
            words: vec![0; points.div_ceil(16)],
        }
    }
    /// Adds `bits` (at most 3) to `point`'s flags.
    pub fn set(&mut self, point: usize, bits: u32) {
        debug_assert!(bits <= 3);
        self.words[point / 16] |= bits << (point % 16 * 2);
    }
    /// `point`'s two bits.
    pub fn get(&self, point: usize) -> u32 {
        self.words
            .get(point / 16)
            .map_or(0, |w| (w >> (point % 16 * 2)) & 3)
    }
    /// The packed words the renderer uploads.
    pub fn words(&self) -> &[u32] {
        &self.words
    }
}
/// A rigid move of the points flagged in `mask`.
#[derive(Clone, Debug)]
pub struct PartMotion {
    /// The move, column-major.
    pub matrix: [[f64; 4]; 4],
    /// Which points it moves.
    pub mask: Arc<MotionMask>,
}
/// One word per point for a weighted (soft or symmetric) move, built once
/// when a gesture starts and also shown as the soft-selection tint:
/// bits 0–1 as in [`MotionMask`] (moved, rigid), bit 2 [`Self::MIRRORED`],
/// bit 3 [`Self::FACETED`], bit 4 [`Self::ON_PLANE`], bits 16–31 the weight
/// in 1/65535 steps. The preview quantizes weights; committed geometry does not.
#[derive(Debug, Default)]
pub struct PointWeights {
    words: Vec<u32>,
}
impl PointWeights {
    /// Moves by the mirror image of the transform (the other side of the plane).
    pub const MIRRORED: u32 = 4;
    /// Shade with screen-space (flat) normals while the preview lasts: its
    /// faces bend and no CPU patch carries their normals.
    pub const FACETED: u32 = 8;
    /// Lies on the symmetry plane: transformed, then put back onto it.
    pub const ON_PLANE: u32 = 16;
    /// Weights of `points` points, none moved.
    pub fn new(points: usize) -> Self {
        Self {
            words: vec![0; points],
        }
    }
    /// Set `point`'s flags and weight (clamped to `[0, 1]`). A positive
    /// weight marks the point moved.
    pub fn set(&mut self, point: usize, flags: u32, weight: f64) {
        let quantized = (weight.clamp(0.0, 1.0) * 65535.0).round() as u32;
        let moved = u32::from(quantized > 0) * MotionMask::MOVED;
        self.words[point] = (quantized << 16) | (flags & 0xfffe) | moved;
    }
    /// Adds `flags` (the low 16 bits) to `point`'s.
    pub fn add_flags(&mut self, point: usize, flags: u32) {
        self.words[point] |= flags & 0xffff;
    }
    /// `point`'s flags.
    pub fn flags(&self, point: usize) -> u32 {
        self.words.get(point).map_or(0, |w| w & 0xffff)
    }
    /// The weight as the GPU sees it.
    pub fn weight(&self, point: usize) -> f64 {
        self.words
            .get(point)
            .map_or(0.0, |w| f64::from(w >> 16) / 65535.0)
    }
    /// How many points.
    pub fn len(&self) -> usize {
        self.words.len()
    }
    /// Whether there are no points.
    pub fn is_empty(&self) -> bool {
        self.words.is_empty()
    }
    /// The words the renderer uploads.
    pub fn words(&self) -> &[u32] {
        &self.words
    }
}
/// A weighted move drawn by the GPU: each point flagged in `weights` moves
/// by the transform scaled by its weight (translation, rotation angle about
/// `axis`, scale towards 1), mirrored points by its mirror image across the
/// plane `p[mirror.0] == mirror.1`.
#[derive(Clone, Debug)]
pub struct WeightedMotion {
    /// Translation at full weight.
    pub translate: [f64; 3],
    /// Rotation axis, unit length.
    pub axis: [f64; 3],
    /// Rotation angle at full weight, radians.
    pub angle: f64,
    /// Scale at full weight, per axis of `frame`.
    pub scale: [f64; 3],
    /// The rotation and scale pivot.
    pub pivot: [f64; 3],
    /// The frame the scale acts in (identity: the mesh's own axes).
    pub frame: [f64; 4],
    /// Mirrored points' plane: axis index and position along it.
    pub mirror: Option<(usize, f64)>,
    /// Each point's flags and weight.
    pub weights: Arc<PointWeights>,
}
#[derive(Debug, Default)]
/// A live preview over the displayed packet: GPU motions and CPU patches.
pub struct PreviewUpdate {
    /// Whole-surface transform, when the preview moves everything rigidly.
    pub affine: Option<[[f64; 4]; 4]>,
    /// A rigid move of part of the surface; positions are then never patched.
    pub motion: Option<PartMotion>,
    /// A weighted (soft or symmetric) move; positions are then never patched.
    pub weighted: Option<WeightedMotion>,
    /// Vertex records replaced.
    pub vertices: Vec<Patch<Vertex>>,
    /// Triangle indices replaced.
    pub indices: Vec<Patch<u32>>,
    /// Point positions replaced.
    pub positions: Vec<Patch<[f32; 3]>>,
}
impl PreviewUpdate {
    /// Logical upload size of the patches.
    pub fn upload_bytes(&self) -> usize {
        self.vertices
            .iter()
            .map(|p| p.values.len() * std::mem::size_of::<Vertex>())
            .sum::<usize>()
            + self
                .positions
                .iter()
                .map(|p| p.values.len() * 12)
                .sum::<usize>()
            + self
                .indices
                .iter()
                .map(|p| p.values.len() * 4)
                .sum::<usize>()
    }
}

/// Live preview patches over `base`, which must be the displayed surface.
#[derive(Clone, Copy)]
pub struct Preview<'a> {
    /// The displayed packet.
    pub base: &'a dyn SurfaceSource,
    /// The patches over it.
    pub update: &'a Arc<PreviewUpdate>,
}
