//! [`HeldPixels`]: a layer's pixels the host holds instead of tiles.
use std::any::Any;
use std::fmt;

/// Pixels a host holds for a layer in place of its tiles: a texture on its
/// GPU while a frame or a run shows it (render-api's `display::Texture`), or
/// a live result resolved each frame (the api's `facets::Live`). What a kind
/// is and how its pixels are read is its own; a consumer that needs tiles
/// asks the host to read them back. A held layer has no tiles: it reads as
/// zeros here, and [`Image::plane`](super::Image::plane) has none.
pub trait HeldPixels: Any + Send + Sync + fmt::Debug {
    /// Which pixels, within its kind: the same key is the same pixels while
    /// they are held.
    fn key(&self) -> u64;
}
