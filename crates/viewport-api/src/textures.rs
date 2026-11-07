//! [`SurfaceTextures`]: the images a surface's polygons show in Shaded, each
//! an image (a [`TextureImage`]: the file's own samples and how its colors
//! become linear light in the working space) a polygon's color multiplies
//! by, read through a [`TextureCorner`] per polygon vertex. A renderer picks
//! its own GPU form of each image and runs its decoding after it fetches
//! texels.
use crate::packet::{Column, SurfaceSource};
use kinogaki_geometry::image::{Sampling, TextureImage};
use std::sync::Arc;

/// What a polygon vertex reads: its texture coordinate, its image and how.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextureCorner {
    /// Its coordinates in the image: `v` 0 its first row.
    pub uv: [f32; 2],
    /// Its image in [`SurfaceTextures::images`], plus one (0: none, the
    /// polygon shows its color alone). All of a polygon's vertices name one.
    pub image: u32,
    /// How coordinates outside the image and between its texels read it; one
    /// for all of a polygon's vertices.
    pub sampling: Sampling,
}

/// The images a surface's polygons show, each multiplying a polygon's color.
/// Allocations are their identities, as a packet's: two are equal when they
/// are the same allocations, and a renderer uploads what is new.
#[derive(Clone)]
pub struct SurfaceTextures {
    /// One a polygon vertex, as a packet numbers them (`Vertex::component`).
    pub corners: Arc<[TextureCorner]>,
    /// The images the corners name, each at most [`MOST`](Self::MOST)
    /// pixels a side, its colors' decoding into the working space (an image
    /// without one shows as its values); `None`: not read yet, or
    /// unreadable, so its polygons show their color alone. An image is its
    /// allocation: the same one uploads once.
    pub images: Arc<[Option<Arc<TextureImage>>]>,
}

impl SurfaceTextures {
    /// The most pixels a side of an image the views show: a host reduces a
    /// larger file's image by whole-pixel boxes to fit.
    pub const MOST: u32 = 2048;

    /// Whether they were made for `surface` (not for another packet, a cook
    /// on its way).
    pub fn fits(&self, surface: &dyn SurfaceSource) -> bool {
        self.corners.len() == surface.column_len(Column::Vertices)
    }

    /// Their allocations, which name them.
    pub fn key(&self) -> [usize; 2] {
        [
            Arc::as_ptr(&self.corners).cast::<TextureCorner>() as usize,
            Arc::as_ptr(&self.images).cast::<Option<Arc<TextureImage>>>() as usize,
        ]
    }
}

impl PartialEq for SurfaceTextures {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl std::fmt::Debug for SurfaceTextures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SurfaceTextures({} corners, {} images)",
            self.corners.len(),
            self.images.len()
        )
    }
}
