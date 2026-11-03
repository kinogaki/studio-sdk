//! A compiled context's runner (the `wgpu` feature): its package's plans
//! ([`NetworkCompiler::compile`](super::NetworkCompiler::compile)'s values)
//! run on the host's device by a [`Runner`] its package makes
//! ([`PlanRunner`]). The host gives it the film ([`RunFilm`]) and its
//! [`ColorService`], whose view transforms come as WGSL (the host keeps the
//! color maths), and shows the [`RunImage`] it returns.
use crate::color::ColorService;
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
        colors: &dyn ColorService,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<RunImage, String>;
    /// The host submitted the last run as `index`: what it reads back is
    /// read once that work is done, waking `wake` once.
    fn submitted(&mut self, index: wgpu::SubmissionIndex, wake: Wake);
    /// Drops what it keeps (the network was closed).
    fn clear(&mut self);
}

/// How a compiled context's runner is made on a host's device.
pub trait PlanRunner: Send + Sync {
    /// One on `device` and `queue`.
    fn make(&self, device: &wgpu::Device, queue: &wgpu::Queue) -> Box<dyn Runner>;
}
