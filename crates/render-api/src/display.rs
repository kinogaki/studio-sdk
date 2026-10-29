//! The display contract between a wgpu host and its render engines.
//!
//! An engine for a wgpu host renders on the host's own device and shows
//! its frames as textures there: [`Display::acquire`] gives the image
//! holding a frame as a [`Shown`], which the engine does not write while it
//! lives. The host drops it once the GPU has finished the frame that
//! sampled it (`wgpu::Queue::on_submitted_work_done` after that submit).
//! Images hold linear ACEScg, RGBA.
//!
//! A compositing host asks for the film instead ([`Display::acquire_film`]):
//! the frame's images at the engine's own film size (one pixel a film
//! pixel, before any upscale to the view), the beauty and the outputs it
//! asks for, pinned the same way. By default an engine gives its shown
//! image as the beauty, with no other outputs.
//!
//! A host holds each engine it can make as an [`Engine`]: its description,
//! and how to make it on the host's device and queue, waking the host once
//! per result, with the [`Options`] it gives.
//!
//! This module is the crate's `wgpu` feature (wgpu 30.0.1).

use std::path::PathBuf;
use std::sync::Arc;

use crate::{Aov, EngineDescription, Frame, RenderEngine, Wake};

/// The images an engine shows its frames in.
pub trait Display: Send {
    /// The image holding `frame` (the engine's newest for its view),
    /// pinned; none when that image no longer holds `frame` (a newer pass
    /// is writing it, or the view's film changed size since): the host
    /// keeps showing what it showed.
    fn acquire(&mut self, frame: &Arc<Frame>) -> Option<Shown>;

    /// `frame`'s film, pinned as [`Display::acquire`] pins: the beauty and
    /// those of `aovs` the engine keeps for its views (it may start keeping
    /// one asked for from a later frame on). By default the shown image is
    /// the beauty and nothing else is kept.
    fn acquire_film(&mut self, frame: &Arc<Frame>, aovs: &[Aov]) -> Option<ShownFilm> {
        let _ = aovs;
        self.acquire(frame).map(ShownFilm::beauty)
    }
}

/// An image the host shows. The engine does not write it while this
/// lives; drop it when the GPU is done sampling it.
pub struct Shown {
    frame: Arc<Frame>,
    key: (u64, u32),
    view: wgpu::TextureView,
    filter: wgpu::FilterMode,
    _retire: Box<dyn Send>,
}

impl Shown {
    /// `frame`'s image `view`, known by `key`, held until `retire` drops
    /// (the engine's pin on it), sampled with linear filtering.
    pub fn new(
        frame: Arc<Frame>,
        key: (u64, u32),
        view: wgpu::TextureView,
        retire: impl Send + 'static,
    ) -> Shown {
        Shown {
            frame,
            key,
            view,
            filter: wgpu::FilterMode::Linear,
            _retire: Box::new(retire),
        }
    }

    /// The image sampled with `filter` instead: nearest for images whose
    /// texels must stay crisp (pixel art) when the host scales them.
    pub fn filtered(mut self, filter: wgpu::FilterMode) -> Shown {
        self.filter = filter;
        self
    }

    /// The frame it shows.
    pub fn frame(&self) -> &Arc<Frame> {
        &self.frame
    }

    /// Which texture it is: the same key is the same texture (a host can
    /// keep what it registered for it), until the view's film changes size.
    pub fn key(&self) -> (u64, u32) {
        self.key
    }

    /// A view of the whole texture (RGBA, linear ACEScg, sampled only).
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// How a host samples it.
    pub fn filter(&self) -> wgpu::FilterMode {
        self.filter
    }
}

/// One image of a film: which output it is, a view of its texture, and
/// the texture's key (the same key is the same texture).
#[derive(Clone, Debug)]
pub struct FilmImage {
    /// The output.
    pub aov: Aov,
    /// A view of the whole texture: Rgba16Float for colors and normals
    /// (linear ACEScg colors), R32Float for depth and occlusion, R32Uint
    /// for ids.
    pub view: wgpu::TextureView,
    /// Which texture it is.
    pub key: (u64, u32),
}

/// A frame's film as a compositing host reads it, pinned until dropped.
pub struct ShownFilm {
    frame: Arc<Frame>,
    size: [u32; 2],
    images: Vec<FilmImage>,
    _retire: Box<dyn Send>,
}

impl ShownFilm {
    /// `frame`'s film of `size` pixels: `images`, the beauty first, held
    /// until `retire` drops.
    pub fn new(
        frame: Arc<Frame>,
        size: [u32; 2],
        images: Vec<FilmImage>,
        retire: impl Send + 'static,
    ) -> ShownFilm {
        debug_assert!(images.first().is_some_and(|i| i.aov.is_beauty()));
        ShownFilm {
            frame,
            size,
            images,
            _retire: Box::new(retire),
        }
    }

    /// The shown image as the film's beauty, alone: the default for an
    /// engine whose shown image is its film.
    pub fn beauty(shown: Shown) -> ShownFilm {
        let frame = shown.frame.clone();
        let image = FilmImage {
            aov: crate::aov::standard::beauty(),
            view: shown.view.clone(),
            key: shown.key,
        };
        ShownFilm {
            size: [frame.width, frame.height],
            frame,
            images: vec![image],
            _retire: Box::new(shown),
        }
    }

    /// The frame it holds.
    pub fn frame(&self) -> &Arc<Frame> {
        &self.frame
    }

    /// The film's size, in film pixels.
    pub fn size(&self) -> [u32; 2] {
        self.size
    }

    /// Display pixels a film pixel covers ([`Frame::block`]).
    pub fn block(&self) -> u32 {
        self.frame.block.max(1)
    }

    /// Its images, the beauty first.
    pub fn images(&self) -> &[FilmImage] {
        &self.images
    }

    /// Output `id`'s image, when it holds one.
    pub fn image(&self, id: &str) -> Option<&FilmImage> {
        self.images.iter().find(|i| &*i.aov.id == id)
    }
}

/// What a host gives an engine it makes.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// A directory the engine may keep caches in between runs (compiled
    /// kernels); none: no cache.
    pub cache: Option<PathBuf>,
    /// Frames also carry their pixels (`Frame::pixels`), for tests that
    /// check what the shown images hold.
    pub read_pixels: bool,
}

/// An engine and the display its frames show in.
pub type Made = (Box<dyn RenderEngine>, Box<dyn Display>);

/// An engine a host can make: what it is, and how to make one.
pub trait Engine: Send + Sync {
    /// What it is, without a device.
    fn description(&self) -> EngineDescription;

    /// One on the host's `device` and `queue`, waking `wake` once per
    /// result; an error says why there is none on this platform or device.
    fn make(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        wake: Wake,
        options: &Options,
    ) -> Result<Made, String>;
}
