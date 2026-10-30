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
//! A compiled context (Comp) runs its plans on the film through a
//! [`Runner`] its package makes on the host's device ([`PlanRunner`]): the
//! host gives it the film ([`RunFilm`]) and its view transforms as WGSL
//! ([`ViewTransforms`]: the host keeps the color maths), and shows the
//! [`RunImage`] it returns; what the runner reads back (a palette) the host
//! asks for with [`Runner::kept`].
//!
//! This module is the crate's `wgpu` feature (wgpu 30.0.1).

use std::path::PathBuf;
use std::sync::Arc;

/// The wgpu this contract's types are (30.0.1): what a host and its
/// engines and runners build against.
pub use wgpu;

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

/// Which display and view a view transform is for.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum ViewChoice {
    /// The viewer's display and view.
    Viewer,
    /// A display and view of the host's color configuration, by name.
    Pinned {
        /// The display.
        display: Arc<str>,
        /// The view.
        view: Arc<str>,
    },
}

/// A view transform (scene-referred ACEScg to display code values, without
/// the viewer's exposure and gamma) as the host's color management writes
/// it for a compute shader: WGSL declaring its data buffer at the group and
/// binding asked for, with helpers and a function `fn <function>(c:
/// vec3<f32>) -> vec3<f32>`, and the buffer's contents. The host holds the
/// color maths; a package only embeds the shader.
#[derive(Clone, Debug, PartialEq)]
pub struct ViewTransform {
    /// The WGSL: the buffer's declaration, helpers and the function.
    pub source: String,
    /// The function's name.
    pub function: String,
    /// The buffer's contents (f32s).
    pub data: Vec<f32>,
}

/// The host's view transforms, for a runner that shows display code
/// values (Comp's To Display).
pub trait ViewTransforms {
    /// `view`'s transform as WGSL named `name` (`[A-Za-z0-9_]`) reading its
    /// data at `group`, `binding`; none when the configuration lacks it.
    fn transform(
        &self,
        view: &ViewChoice,
        name: &str,
        group: u32,
        binding: u32,
    ) -> Option<ViewTransform>;
    /// Changes whenever a transform would.
    fn revision(&self) -> u64;
}

/// What an image means.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Referred {
    /// Scene light, linear ACEScg: the host still applies its view.
    Scene,
    /// Display code values, 0 to 1: shown as they are.
    Display,
}

/// The film a plan runs on: a frame's images at the film's size.
#[derive(Clone, Copy, Debug)]
pub struct RunFilm<'a> {
    /// The film's size, in film pixels.
    pub size: [u32; 2],
    /// Display pixels a film pixel covers.
    pub block: u32,
    /// Changes whenever the film's contents do (a new frame).
    pub stamp: u64,
    /// Its images, the beauty first.
    pub images: &'a [FilmImage],
}

/// The image a run shows.
#[derive(Clone, Debug)]
pub struct RunImage {
    /// A view of it, at the film's size.
    pub view: wgpu::TextureView,
    /// Which texture it is (the same key is the same texture, unchanged).
    pub key: u64,
    /// What it holds.
    pub referred: Referred,
}

/// A compiled context's executor on one device: it runs its package's
/// plans (`NetworkCompiler::compile`'s values, opaque to the host) on a
/// film in the host's encoder, keeping what it can reuse.
pub trait Runner: Send {
    /// Encodes `plan` on `film` into `encoder` (the host submits it), only
    /// what changed since the last run; the image it shows, or why it
    /// can't run on this film (an output the engine doesn't keep).
    fn run(
        &mut self,
        plan: &(dyn std::any::Any + Send + Sync),
        film: &RunFilm<'_>,
        transforms: &dyn ViewTransforms,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<RunImage, String>;
    /// The host submitted the last run as `index`: what it reads back (a
    /// palette it made) is read once that work is done, waking `wake` once.
    fn submitted(&mut self, index: wgpu::SubmissionIndex, wake: Wake);
    /// What node `node` made that the runner keeps for the host to read
    /// (its last read-back palette, r, g, b a color); none when it keeps
    /// nothing for it.
    fn kept(&self, node: u64) -> Option<Arc<[f64]>>;
    /// Drops what it keeps (the network was closed).
    fn clear(&mut self);
}

/// How a compiled context's runner is made on a host's device.
pub trait PlanRunner: Send + Sync {
    /// One on `device` and `queue`.
    fn make(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Box<dyn Runner>;
}
