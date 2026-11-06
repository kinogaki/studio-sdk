//! A compiled context's runner (the `wgpu` feature): its package's plans
//! ([`NetworkCompiler::compile`](super::NetworkCompiler::compile)'s values)
//! run on the host's device by a [`Runner`] its package makes
//! ([`PlanRunner`]). The host lends it what it reads ([`RunCx`]): the film, an
//! Image of the newest pass; its [`ColorService`] (the host keeps the color
//! maths) and its [`ImageFiles`]. A run gives an Image back, which the host
//! shows through the view transform unless its colors are a display's.
use crate::Error;
use crate::color::ColorService;
use crate::image_files::ImageFiles;
use kinogaki_geometry::image::Image;
use kinogaki_render_api::Wake;
use kinogaki_render_api::display::wgpu;

/// What a run reads.
#[derive(Clone, Copy)]
pub struct RunCx<'a> {
    /// The film it runs on: the newest pass's outputs as one Image at the
    /// film's size, each layer held as its texture
    /// ([`display::Texture`](kinogaki_render_api::display::Texture)), the beauty
    /// the main one and the others by their ids; none while no render is
    /// under way.
    pub film: Option<&'a Image>,
    /// The host's colors.
    pub colors: &'a dyn ColorService,
    /// The host's image files.
    pub files: &'a dyn ImageFiles,
}

/// What a runner's last run did, as a host counts it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RunStats {
    /// Steps it encoded (0 when nothing changed).
    pub steps: usize,
    /// GPU resources it has made so far.
    pub made: u64,
}

/// A compiled context's executor on one device: it runs its package's
/// plans (`NetworkCompiler::compile`'s values, opaque to the host) on a
/// film in the host's encoder, keeping what it can reuse.
pub trait Runner: Send {
    /// Encodes `plan` on what `cx` lends into `encoder` (the host submits
    /// it), only what changed since the last run. The Image it shows: one
    /// layer held as a texture (`Rgba16Float`, with `COPY_SRC` usage so the
    /// host can read it back: a frozen node keeps it, a final render writes
    /// it), its colors' space saying what they are (scene-referred
    /// [`RENDER_SPACE`](kinogaki_render_api::RENDER_SPACE), or a display's).
    /// Or why it can't run (an output the engine doesn't keep, an unreadable
    /// file).
    fn run(
        &mut self,
        plan: &(dyn std::any::Any + Send + Sync),
        cx: &RunCx<'_>,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<Image, Error>;
    /// The host submitted the last run as `index`: what it reads back is
    /// read once that work is done, waking `wake` once.
    fn submitted(&mut self, index: wgpu::SubmissionIndex, wake: Wake);
    /// What its last run did.
    fn stats(&self) -> RunStats;
    /// What the last run's plan writes in a final render: each file, as the
    /// plan names it, and its Image (as [`run`](Self::run) gives one).
    fn written(&self) -> &[(String, Image)];
}

/// How a compiled context's runner is made on a host's device.
pub trait PlanRunner: Send + Sync {
    /// One on `device` and `queue`.
    fn make(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Box<dyn Runner>;
}
