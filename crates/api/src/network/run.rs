//! A compiled context's runner (the `wgpu` feature): its package's plans
//! ([`NetworkCompiler::compile`](super::NetworkCompiler::compile)'s values)
//! run on the host's device by a [`Runner`] its package makes
//! ([`PlanRunner`]). The host lends it what it reads ([`RunCx`]): the film, its
//! [`ColorService`], whose view transforms come as WGSL (the host keeps the
//! color maths), and its [`ImageFiles`]; and it shows the [`RunImage`] it
//! returns.
use crate::Error;
use crate::color::ColorService;
use crate::image_files::ImageFiles;
use kinogaki_render_api::Wake;
use kinogaki_render_api::display::{FilmImage, Referred, wgpu};

/// The film a plan runs on: a frame's images at the film's size.
#[derive(Clone, Copy, Debug)]
pub struct RunFilm<'a> {
    /// The film's size, in film pixels.
    pub size: [u32; 2],
    /// Changes whenever the film's contents do (a new frame).
    pub stamp: u64,
    /// Its images, the beauty first.
    pub images: &'a [FilmImage],
}

/// What a run reads.
#[derive(Clone, Copy)]
pub struct RunCx<'a> {
    /// The film it runs on; no images while no render is under way.
    pub film: RunFilm<'a>,
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

/// The image a run shows.
#[derive(Clone, Debug)]
pub struct RunImage {
    /// A view of it.
    pub view: wgpu::TextureView,
    /// Its size, in pixels.
    pub size: [u32; 2],
    /// Which texture it is (the same key is the same texture, unchanged).
    pub key: u64,
    /// What it holds.
    pub referred: Referred,
}

/// A compiled context's executor on one device: it runs its package's
/// plans (`NetworkCompiler::compile`'s values, opaque to the host) on a
/// film in the host's encoder, keeping what it can reuse.
pub trait Runner: Send {
    /// Encodes `plan` on what `cx` lends into `encoder` (the host submits
    /// it), only what changed since the last run; the image it shows, or why
    /// it can't run (an output the engine doesn't keep, an unreadable file).
    fn run(
        &mut self,
        plan: &(dyn std::any::Any + Send + Sync),
        cx: &RunCx<'_>,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<RunImage, Error>;
    /// The host submitted the last run as `index`: what it reads back is
    /// read once that work is done, waking `wake` once.
    fn submitted(&mut self, index: wgpu::SubmissionIndex, wake: Wake);
    /// What its last run did.
    fn stats(&self) -> RunStats;
    /// What the last run's plan writes in a final render: each file, as the
    /// plan names it, and its image.
    fn written(&self) -> &[(String, RunImage)];
}

/// How a compiled context's runner is made on a host's device.
pub trait PlanRunner: Send + Sync {
    /// One on `device` and `queue`.
    fn make(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Box<dyn Runner>;
}
