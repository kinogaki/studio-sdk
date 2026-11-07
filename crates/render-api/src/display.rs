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
//! the frame as an Image at the engine's own film size (one pixel a film
//! pixel, before any upscale to the view), the beauty its main layer and the
//! outputs it asks for beside it, each layer held as its texture
//! ([`Texture`]) and pinned while any holds it. By default an engine gives
//! its shown image as the beauty, with no other outputs.
//!
//! A host holds each engine it can make as an [`Engine`]: its description,
//! and how to make it on the host's device and queue, waking the host once
//! per result, with the [`Options`] it gives.
//!
//! This module is the crate's `wgpu` feature (wgpu 30.0.1).

use std::any::Any;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

/// The wgpu this contract's types are (30.0.1): what a host and its
/// engines and runners build against.
pub use wgpu;

use crate::aov::standard;
use crate::geometry::image::{Encoding, Format, HeldPixels, Image, Layer, Rect, Sample};
use crate::{Aov, AovClass, EngineDescription, RenderEngine, ViewId, ViewPass, Wake};

/// The images an engine shows its frames in.
pub trait Display: Send {
    /// The image holding `frame` (the engine's newest for its view),
    /// pinned; none when that image no longer holds `frame` (a newer pass
    /// is writing it, or the view's film changed size since): the host
    /// keeps showing what it showed.
    fn acquire(&mut self, frame: &Arc<ViewPass>) -> Option<Shown>;

    /// `frame`'s film as an Image ([`film`]), pinned as [`Display::acquire`]
    /// pins while any of its layers is held: the beauty and those of `aovs`
    /// the engine keeps for its views (it may start keeping one asked for
    /// from a later frame on). By default the shown image is the beauty and
    /// nothing else is kept.
    fn acquire_film(&mut self, frame: &Arc<ViewPass>, aovs: &[Aov]) -> Option<Image> {
        let _ = aovs;
        let shown = self.acquire(frame)?;
        let (key, view) = (shown.key(), shown.view().clone());
        let id = key.0 << 16 ^ u64::from(key.1);
        let size = frame.size();
        film(frame, size, vec![(standard::beauty(), view, id)], shown)
    }

    /// View `view` went ([`RenderEngine::drop_view`], which the host calls
    /// first): whatever the display keeps for it (wrapped images, the
    /// outputs it was asked for) goes. A later view the host gives the same
    /// id starts with nothing of it.
    fn drop_view(&mut self, view: ViewId);
}

/// An image the host shows. The engine does not write it while this
/// lives; drop it when the GPU is done sampling it.
pub struct Shown {
    frame: Arc<ViewPass>,
    key: (u64, u32),
    view: wgpu::TextureView,
    _retire: Box<dyn Send>,
}

impl Shown {
    /// `frame`'s image `view`, known by `key`, held until `retire` drops
    /// (the engine's pin on it).
    pub fn new(
        frame: Arc<ViewPass>,
        key: (u64, u32),
        view: wgpu::TextureView,
        retire: impl Send + 'static,
    ) -> Shown {
        Shown {
            frame,
            key,
            view,
            _retire: Box::new(retire),
        }
    }

    /// The frame it shows.
    pub fn frame(&self) -> &Arc<ViewPass> {
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
}

/// A layer's pixels held as a texture on the host's device
/// ([`HeldPixels`]): an output of a frame a display pinned
/// ([`Display::acquire_film`]), or a run's result. Nothing writes the texture
/// while any of its holders lives, so an Image of them is frame-scoped: shown,
/// read back or let go, never cached or saved.
#[derive(Clone)]
pub struct Texture {
    view: wgpu::TextureView,
    id: u64,
    generation: u64,
    _pin: Arc<dyn Any + Send + Sync>,
}

impl Texture {
    /// `view` of a whole texture (`Rgba16Float` for colors and vectors,
    /// `R32Float` for other numbers, `R32Uint` for ids), `id` its texture's
    /// identity, `generation` moving whenever its contents do, held by `pin`
    /// until the last holder goes.
    pub fn new(
        view: wgpu::TextureView,
        id: u64,
        generation: u64,
        pin: Arc<dyn Any + Send + Sync>,
    ) -> Texture {
        Texture {
            view,
            id,
            generation,
            _pin: pin,
        }
    }

    /// Layer `layer`'s texture, when its pixels are one.
    pub fn of(layer: &Layer) -> Option<&Texture> {
        let held: &dyn Any = &**layer.held()?;
        held.downcast_ref()
    }

    /// A view of the whole texture.
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Which texture it is: the same id is the same texture (a host keeps
    /// what it bound for it).
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Moves whenever its contents do.
    pub fn generation(&self) -> u64 {
        self.generation
    }
}

impl HeldPixels for Texture {
    fn key(&self) -> u64 {
        self.id ^ self.generation.rotate_left(29)
    }
}

impl std::fmt::Debug for Texture {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        (f.debug_struct("Texture").field("id", &self.id))
            .field("generation", &self.generation)
            .finish_non_exhaustive()
    }
}

/// `frame`'s film as an Image of `size` film pixels: each of `outputs` (an
/// output, a view of its texture and the texture's id) a layer held as that
/// texture, the beauty the main one, its format the texture's texels (colors
/// in [`RENDER_SPACE`](crate::RENDER_SPACE), premultiplied); held by `pin`
/// until the image's last holder goes. None when `size` holds no pixels.
pub fn film(
    frame: &Arc<ViewPass>,
    size: [u32; 2],
    outputs: Vec<(Aov, wgpu::TextureView, u64)>,
    pin: impl Send + 'static,
) -> Option<Image> {
    let window = Rect::new([0, 0], size.map(|v| v as i32)).ok()?;
    // The frame is held with the pin, so no other frame takes its place.
    let pin: Arc<dyn Any + Send + Sync> = Arc::new(Mutex::new((frame.clone(), pin)));
    let pass = pass(frame);
    let mut builder = Image::builder(window);
    for (aov, view, id) in outputs {
        let format = format_of(&aov, view.texture().format());
        let texture = Texture::new(view, id, pass, pin.clone());
        builder = builder.held(format, Arc::new(texture));
    }
    builder.build().ok()
}

/// Which pass `frame` is, among those its view published and that live.
fn pass(frame: &Arc<ViewPass>) -> u64 {
    use std::hash::{Hash, Hasher};
    let mut h = std::hash::DefaultHasher::new();
    (Arc::as_ptr(frame) as usize, frame.view, frame.generation).hash(&mut h);
    (frame.engine_image(), frame.stats.samples).hash(&mut h);
    h.finish()
}

/// The format of output `aov`'s layer held as a texture of `texels`.
fn format_of(aov: &Aov, texels: wgpu::TextureFormat) -> Format {
    let (channels, sample): (&[&str], Sample) = match texels {
        wgpu::TextureFormat::R32Uint => (&["id"], Sample::U32),
        wgpu::TextureFormat::R32Float if &*aov.id == standard::DEPTH => (&["Z"], Sample::F32),
        wgpu::TextureFormat::R32Float => (&["Y"], Sample::F32),
        _ if aov.class == AovClass::Vector => (&["X", "Y", "Z", "W"], Sample::F16),
        _ => (&["R", "G", "B", "A"], Sample::F16),
    };
    Format {
        name: crate::aov::layer(&aov.id).into(),
        channels: channels.iter().map(|c| Arc::from(*c)).collect(),
        sample,
        encoding: match aov.is_color() {
            true => Encoding::color(crate::RENDER_SPACE, true),
            false => Encoding::Raw,
        },
    }
}

/// What a host gives an engine it makes.
#[derive(Clone, Debug, Default)]
pub struct Options {
    /// A directory the engine may keep caches in between runs (compiled
    /// kernels); none: no cache.
    pub cache: Option<PathBuf>,
    /// Frames hold their pixels as tiles ([`ViewPass::pixels`]), for tests
    /// that check what the shown images hold.
    pub read_pixels: bool,
}

/// An engine and the display its frames show in.
pub type Made = (Box<dyn RenderEngine>, Box<dyn Display>);

/// An engine a host can make: what it is, and how to make one.
pub trait Engine: Send + Sync {
    /// What it is, without a device.
    fn description(&self) -> EngineDescription;

    /// One on the host's `device` and `queue`, waking `wake` once per
    /// result; the error says why there is none on this platform or device.
    fn make(
        &self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        wake: Wake,
        options: &Options,
    ) -> Result<Made, kinogaki_geometry::Error>;
}
