//! [`Image`]: a 2D value of tiled layers, what files, render textures, Comp's
//! boundaries and the 2D canvas exchange.
//!
//! - **Two windows.** The data window bounds the stored pixels, the display
//!   window frames them (OpenEXR's windows, Nuke's bbox), so a blur or a
//!   transform grows without a crop.
//! - **Layers.** One or more [`Layer`]s: `""` the main one, others by name
//!   (`N`, `Z`, `crypto00`). A layer holds 1–4 interleaved channels of one
//!   [`Sample`], its colors named by [`Encoding`]: a space of the project's
//!   color configuration, or raw data that is never converted.
//! - **Tiles.** A layer's pixels live in [`TILE`]-sided [`Tile`]s, row-major
//!   over the data window, each its own shared [`Column`], a
//!   tile of nothing but zeros absent. An edit ([`Image::with_tiles`]) makes
//!   a new image sharing every tile it didn't touch, so an edit's memory
//!   follows what it touched, and a tile's [`key`](Tile::key) is what a GPU
//!   upload or a mip level derived from it is keyed by.
//! - **Levels.** A [`Pyramid`] derives smaller levels of tiles on demand,
//!   keeping each while the tiles under it are the same.
//! - **Attributes.** An image's metadata is its Detail [`Attributes`].
//!
//! ```
//! use kinogaki_geometry::image::{Encoding, Format, Image, Rect, Sample};
//!
//! let window = Rect::new([0, 0], [100, 80]).unwrap();
//! let format = Format::rgba(Sample::F32, Encoding::color("ACEScg", true));
//! let pixels = vec![0.25f32; 4 * 100 * 80];
//! let image = Image::builder(window)
//!     .plane(format, bytemuck::cast_slice(&pixels))
//!     .build()
//!     .unwrap();
//! assert_eq!(image.read_pixel(0, 99, 79), [0.25; 4]);
//! assert_eq!(image.read_pixel(0, 100, 0), [0.0; 4]); // outside the data window
//! ```
mod pyramid;
mod sample;
#[cfg(test)]
mod tests;

pub use pyramid::Pyramid;
pub use sample::{Sample, f32_to_half, half_to_f32};

use crate::attributes::Attributes;
use crate::column::Column;
use crate::error::Error;
use crate::limits::MAX_NAME;
use std::sync::{Arc, Weak};

/// Pixels a side of a [`Tile`] (Krita's size): an edit copies 64 × 64 pixels
/// at the least.
pub const TILE: u32 = 64;

/// The largest side of an image's window, in pixels.
pub(crate) const MAX_SIDE: u32 = 1 << 24;

/// A window of pixels: `min` inclusive, `max` exclusive; never empty.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Rect {
    min: [i32; 2],
    max: [i32; 2],
}

impl Rect {
    /// The window from `min` up to, not including, `max`.
    ///
    /// # Errors
    /// An empty window, or one with a side over 2²⁴ pixels.
    pub fn new(min: [i32; 2], max: [i32; 2]) -> Result<Self, Error> {
        let side = |k: usize| i64::from(max[k]) - i64::from(min[k]);
        if (0..2).any(|k| side(k) <= 0 || side(k) > i64::from(MAX_SIDE)) {
            return Err(Error::invalid(format_args!(
                "an image window can't run from {min:?} to {max:?}"
            )));
        }
        Ok(Self { min, max })
    }

    /// Its first pixel.
    pub fn min(&self) -> [i32; 2] {
        self.min
    }

    /// One past its last pixel.
    pub fn max(&self) -> [i32; 2] {
        self.max
    }

    /// Its width and height, in pixels.
    pub fn size(&self) -> [u32; 2] {
        [0, 1].map(|k| (i64::from(self.max[k]) - i64::from(self.min[k])) as u32)
    }

    /// Whether pixel (`x`, `y`) lies inside.
    pub fn contains(&self, x: i32, y: i32) -> bool {
        (self.min[0]..self.max[0]).contains(&x) && (self.min[1]..self.max[1]).contains(&y)
    }
}

/// What a layer's values mean.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Encoding {
    /// Colors in a space of the project's color configuration.
    Color {
        /// The space, by its name in the configuration.
        space: Arc<str>,
        /// Whether the colors are premultiplied by their alpha.
        premultiplied: bool,
    },
    /// Data (normals, depth, ids): never converted, shown as they are.
    Raw,
}

impl Encoding {
    /// Colors in `space`.
    pub fn color(space: &str, premultiplied: bool) -> Self {
        Self::Color {
            space: space.into(),
            premultiplied,
        }
    }
}

/// How a layer holds its pixels.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Format {
    /// Its name: `""` the main layer, else `N`, `Z`, `crypto00`, ….
    pub name: Arc<str>,
    /// Its channels' names, interleaved in this order: 1 to 4.
    pub channels: Arc<[Arc<str>]>,
    /// What a channel's value is stored as.
    pub sample: Sample,
    /// What the values mean.
    pub encoding: Encoding,
}

impl Format {
    /// The main layer's format: `R`, `G`, `B`, `A`.
    pub fn rgba(sample: Sample, encoding: Encoding) -> Self {
        let channels = ["R", "G", "B", "A"].map(Arc::from);
        Self {
            name: "".into(),
            channels: Arc::new(channels),
            sample,
            encoding,
        }
    }

    /// Bytes a pixel takes.
    pub fn pixel_bytes(&self) -> usize {
        self.channels.len() * self.sample.bytes()
    }

    /// Bytes a tile takes.
    pub fn tile_bytes(&self) -> usize {
        (TILE * TILE) as usize * self.pixel_bytes()
    }
}

/// One tile's pixels: [`TILE`]² pixels of its layer's [`Format`], row-major,
/// shared by every image holding it.
#[derive(Clone, Debug, PartialEq)]
pub struct Tile(Column<u8>);

impl Tile {
    /// `bytes` as a tile, without copying them.
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(Column::new(bytes))
    }

    /// Its bytes.
    pub fn bytes(&self) -> &[u8] {
        self.0.as_slice()
    }

    /// How many bytes it holds.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether it holds no bytes (never a layer's tile).
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Its identity: the same key is the same tile, while it lives. What a
    /// GPU upload or a derived level is keyed by.
    pub fn key(&self) -> u64 {
        self.0.key()
    }

    /// Whether `other` is this very tile.
    pub fn ptr_eq(&self, other: &Tile) -> bool {
        self.0.ptr_eq(&other.0)
    }
}

/// One layer of an image.
#[derive(Clone, Debug, PartialEq)]
pub struct Layer {
    format: Format,
    tiles: Arc<[Option<Tile>]>,
}

impl Layer {
    /// How it holds its pixels.
    pub fn format(&self) -> &Format {
        &self.format
    }

    /// Its tiles, row-major over the data window; `None` holds all zeros.
    pub fn tiles(&self) -> &[Option<Tile>] {
        &self.tiles
    }
}

/// A 2D value of tiled layers ([module docs](self)).
#[derive(Clone, Debug, PartialEq)]
pub struct Image {
    display: Rect,
    data: Rect,
    aspect: f64,
    layers: Arc<[Layer]>,
    attributes: Arc<Attributes>,
}

/// An [`Image`] held without keeping its pixels or metadata alive: what a
/// store of images keeps, so an image goes with its last holder and the
/// store answers with the same tiles while one holds it.
#[derive(Clone, Debug)]
pub struct WeakImage {
    display: Rect,
    data: Rect,
    aspect: f64,
    layers: Weak<[Layer]>,
    attributes: Weak<Attributes>,
}

impl WeakImage {
    /// The image, while something else holds it.
    pub fn upgrade(&self) -> Option<Image> {
        Some(Image {
            display: self.display,
            data: self.data,
            aspect: self.aspect,
            layers: self.layers.upgrade()?,
            attributes: self.attributes.upgrade()?,
        })
    }
}

/// Tiles across and down a data window of `size`.
pub fn tiles_of(size: [u32; 2]) -> [u32; 2] {
    size.map(|n| n.div_ceil(TILE))
}

impl Image {
    /// An image framed by `display`: its data window that too until
    /// [`data`](ImageBuilder::data) says otherwise.
    pub fn builder(display: Rect) -> ImageBuilder {
        ImageBuilder {
            display,
            data: display,
            aspect: 1.0,
            layers: Vec::new(),
            attributes: Attributes::new(),
            error: None,
        }
    }

    /// The window that frames it.
    pub fn display(&self) -> Rect {
        self.display
    }

    /// The window its pixels are stored over.
    pub fn data(&self) -> Rect {
        self.data
    }

    /// A pixel's width over its height.
    pub fn aspect(&self) -> f64 {
        self.aspect
    }

    /// Its layers, the main one first when there is one.
    pub fn layers(&self) -> &[Layer] {
        &self.layers
    }

    /// Its metadata: Detail attributes.
    pub fn attributes(&self) -> &Attributes {
        &self.attributes
    }

    /// It held weakly ([`WeakImage`]).
    pub fn downgrade(&self) -> WeakImage {
        WeakImage {
            display: self.display,
            data: self.data,
            aspect: self.aspect,
            layers: Arc::downgrade(&self.layers),
            attributes: Arc::downgrade(&self.attributes),
        }
    }

    /// This image with `changed` tiles of layer `layer` replaced, by their
    /// place across and down the data window (`None` above: all zeros). It
    /// shares every other tile.
    ///
    /// # Errors
    /// No such layer or place, or a tile of another size.
    pub fn with_tiles(
        &self,
        layer: usize,
        changed: impl IntoIterator<Item = ([u32; 2], Option<Tile>)>,
    ) -> Result<Self, Error> {
        let at = self.layers.get(layer);
        let at = at.ok_or_else(|| Error::invalid(format_args!("no layer {layer}")))?;
        let across = tiles_of(self.data.size());
        let mut tiles: Vec<Option<Tile>> = at.tiles.to_vec();
        for ([x, y], tile) in changed {
            if x >= across[0] || y >= across[1] {
                return Err(Error::invalid(format_args!("no tile at ({x}, {y})")));
            }
            if let Some(t) = &tile {
                check_tile(t, &at.format)?;
            }
            tiles[(y * across[0] + x) as usize] = tile;
        }
        let mut layers = self.layers.to_vec();
        layers[layer].tiles = tiles.into();
        Ok(Self {
            layers: layers.into(),
            ..self.clone()
        })
    }

    /// This image with `attributes` as its metadata.
    ///
    /// # Errors
    /// An attribute or a group outside the Detail domain.
    pub fn with_attributes(&self, attributes: Attributes) -> Result<Self, Error> {
        check_attributes(&attributes)?;
        Ok(Self {
            attributes: Arc::new(attributes),
            ..self.clone()
        })
    }

    /// The tile of layer `layer` at `at`, when it holds any pixel.
    pub fn tile(&self, layer: usize, at: [u32; 2]) -> Option<&Tile> {
        let across = tiles_of(self.data.size());
        if at[0] >= across[0] || at[1] >= across[1] {
            return None;
        }
        let tiles = &self.layers.get(layer)?.tiles;
        tiles[(at[1] * across[0] + at[0]) as usize].as_ref()
    }

    /// Pixel (`x`, `y`) of layer `layer` as stored: its channels in order,
    /// decoded to floats (U8 and U16 to 0–1), no conversion of its space. A
    /// channel it doesn't hold reads 0, alpha 1; outside the data window,
    /// or in a tile of nothing, every channel reads 0.
    pub fn read_pixel(&self, layer: usize, x: i32, y: i32) -> [f32; 4] {
        let Some(at) = self.layers.get(layer) else {
            return [0.0; 4];
        };
        if !self.data.contains(x, y) {
            return [0.0; 4];
        }
        let dx = (i64::from(x) - i64::from(self.data.min[0])) as u32;
        let dy = (i64::from(y) - i64::from(self.data.min[1])) as u32;
        let Some(tile) = self.tile(layer, [dx / TILE, dy / TILE]) else {
            return [0.0; 4];
        };
        let format = &at.format;
        let place = ((dy % TILE) * TILE + dx % TILE) as usize * format.pixel_bytes();
        decode(format.sample, format.channels.len(), &tile.bytes()[place..])
    }

    /// Layer `layer`'s pixels over the whole data window as stored (native
    /// order), row-major from the top: what [`ImageBuilder::plane`] takes.
    /// None when there is no such layer.
    pub fn plane(&self, layer: usize) -> Option<Vec<u8>> {
        let format = &self.layers.get(layer)?.format;
        let [width, height] = self.data.size().map(|n| n as usize);
        let (bytes, side) = (format.pixel_bytes(), TILE as usize);
        let mut out = vec![0; width * height * bytes];
        for y in 0..height {
            for tx in 0..width.div_ceil(side) {
                let Some(tile) = self.tile(layer, [tx as u32, (y / side) as u32]) else {
                    continue;
                };
                let run = side.min(width - tx * side) * bytes;
                let from = &tile.bytes()[(y % side) * side * bytes..][..run];
                out[(y * width + tx * side) * bytes..][..run].copy_from_slice(from);
            }
        }
        Some(out)
    }

    /// Bytes of pixels it holds.
    pub fn pixel_bytes(&self) -> usize {
        (self.layers.iter())
            .map(|l| l.tiles.iter().flatten().map(Tile::len).sum::<usize>())
            .sum()
    }
}

/// `bytes`' first pixel of `channels` channels of `sample`, as four floats.
pub(crate) fn decode(sample: Sample, channels: usize, bytes: &[u8]) -> [f32; 4] {
    let mut out = [0.0, 0.0, 0.0, 1.0];
    for (k, value) in out.iter_mut().enumerate().take(channels) {
        *value = sample.read(&bytes[k * sample.bytes()..]);
    }
    out
}

fn check_tile(tile: &Tile, format: &Format) -> Result<(), Error> {
    if tile.len() != format.tile_bytes() {
        return Err(Error::invalid(format_args!(
            "a tile of layer {:?} holds {} bytes, not {}",
            format.name,
            tile.len(),
            format.tile_bytes()
        )));
    }
    Ok(())
}

fn check_attributes(attributes: &Attributes) -> Result<(), Error> {
    use crate::Domain;
    let outside = (attributes.iter().map(|(d, name, _)| (d, name)))
        .chain(attributes.groups().map(|(d, name, _)| (d, name)))
        .find(|(d, _)| *d != Domain::Detail);
    match outside {
        Some((_, name)) => Err(Error::invalid(format_args!(
            "image metadata holds Detail attributes alone, not {name}"
        ))),
        None => Ok(()),
    }
}

fn check_format(format: &Format) -> Result<(), Error> {
    if !(1..=4).contains(&format.channels.len()) {
        return Err(Error::invalid(format_args!(
            "layer {:?} has {} channels, not 1 to 4",
            format.name,
            format.channels.len()
        )));
    }
    if format.name.len() > MAX_NAME || (format.channels.iter()).any(|c| c.is_empty()) {
        return Err(Error::invalid(format_args!(
            "layer {:?} is misnamed",
            format.name
        )));
    }
    if let Encoding::Color { space, .. } = &format.encoding
        && space.is_empty()
    {
        return Err(Error::invalid(format_args!(
            "layer {:?} names no color space",
            format.name
        )));
    }
    Ok(())
}

/// Builds an [`Image`], checking its rules: a positive finite aspect; layers
/// of unique names and 1 to 4 channels; a tile a place, of its layer's size;
/// metadata in Detail alone.
#[derive(Debug)]
pub struct ImageBuilder {
    display: Rect,
    data: Rect,
    aspect: f64,
    layers: Vec<Layer>,
    attributes: Attributes,
    error: Option<Error>,
}

impl ImageBuilder {
    /// Stores its pixels over `data` (set this before the layers).
    pub fn data(mut self, data: Rect) -> Self {
        self.data = data;
        self
    }

    /// A pixel's width over its height.
    pub fn aspect(mut self, aspect: f64) -> Self {
        self.aspect = aspect;
        self
    }

    /// Its metadata.
    pub fn attributes(mut self, attributes: Attributes) -> Self {
        self.attributes = attributes;
        self
    }

    /// A layer of `tiles`, row-major over the data window.
    pub fn tiles(mut self, format: Format, tiles: Vec<Option<Tile>>) -> Self {
        self.layers.push(Layer {
            format,
            tiles: tiles.into(),
        });
        self
    }

    /// A layer of `pixels`, row-major over the whole data window, split into
    /// tiles; a tile of nothing but zero bytes is left absent.
    pub fn plane(mut self, format: Format, pixels: &[u8]) -> Self {
        let [width, height] = self.data.size().map(|n| n as usize);
        let bytes = format.pixel_bytes();
        if pixels.len() != width * height * bytes {
            self.error = Some(Error::invalid(format_args!(
                "layer {:?} holds {} bytes over {width} × {height} pixels of {bytes} bytes",
                format.name,
                pixels.len(),
            )));
            return self;
        }
        let across = tiles_of(self.data.size()).map(|n| n as usize);
        let side = TILE as usize;
        let mut tiles = Vec::with_capacity(across[0] * across[1]);
        for ty in 0..across[1] {
            for tx in 0..across[0] {
                let mut tile = vec![0; format.tile_bytes()];
                let mut any = false;
                for row in 0..side.min(height.saturating_sub(ty * side)) {
                    let y = ty * side + row;
                    let run = side.min(width - tx * side) * bytes;
                    let from = (y * width + tx * side) * bytes;
                    let to = &mut tile[row * side * bytes..][..run];
                    to.copy_from_slice(&pixels[from..from + run]);
                    any |= to.iter().any(|&b| b != 0);
                }
                tiles.push(any.then(|| Tile::new(tile)));
            }
        }
        self.tiles(format, tiles)
    }

    /// The image.
    ///
    /// # Errors
    /// A rule it breaks, naming the layer at fault.
    pub fn build(self) -> Result<Image, Error> {
        if let Some(error) = self.error {
            return Err(error);
        }
        if !(self.aspect.is_finite() && self.aspect > 0.0) {
            return Err(Error::invalid(format_args!(
                "an image's pixel aspect can't be {}",
                self.aspect
            )));
        }
        check_attributes(&self.attributes)?;
        let across = tiles_of(self.data.size());
        let count = across[0] as usize * across[1] as usize;
        for (at, layer) in self.layers.iter().enumerate() {
            check_format(&layer.format)?;
            if layer.tiles.len() != count {
                return Err(Error::invalid(format_args!(
                    "layer {:?} has {} tiles, not {count}",
                    layer.format.name,
                    layer.tiles.len()
                )));
            }
            for tile in layer.tiles.iter().flatten() {
                check_tile(tile, &layer.format)?;
            }
            if (self.layers[..at].iter()).any(|l| l.format.name == layer.format.name) {
                return Err(Error::invalid(format_args!(
                    "two layers are named {:?}",
                    layer.format.name
                )));
            }
        }
        Ok(Image {
            display: self.display,
            data: self.data,
            aspect: self.aspect,
            layers: self.layers.into(),
            attributes: Arc::new(self.attributes),
        })
    }
}
