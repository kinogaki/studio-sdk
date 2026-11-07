//! [`Sampler`]: one layer of an [`Image`] read at points.
use super::{Image, Pixels, Pyramid, Sample, TILE, Tile, Wrap, decode, tiles_of};
use std::sync::Arc;

#[cfg(test)]
mod tests;

/// Which texels a reader reads about a point, and how it blends them: a
/// [`Sampler`]'s, a texture's ([`Sampling`](super::Sampling)).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum SampleFilter {
    /// The texel the point lies in, as it is: pixel art's.
    Nearest,
    /// The four texels whose centers surround the point, blended by its place
    /// between them (a GPU's mip levels besides, where it keeps them).
    #[default]
    Bilinear,
    /// The sixteen texels about the point, through Keys' cubic (a = −0.5,
    /// Catmull–Rom): it passes through every texel and reproduces a ramp.
    Bicubic,
    /// Bilinear on the layer's mean levels whose texels are as wide as the
    /// footprint: the two nearest it, blended by its base-2 logarithm, so
    /// points far apart read the mean about them rather than one texel.
    Footprint,
}

/// One level a sampler reads.
#[derive(Clone, Debug)]
struct Level {
    /// Its tiles, row-major over the data window.
    tiles: Arc<[Option<Tile>]>,
    /// Texels across and down.
    size: [u32; 2],
    /// Tiles across.
    across: usize,
    /// The layer's pixels a texel spans along each side: 2 to the level.
    scale: f64,
    /// Along each axis, the first and the last texel the display window
    /// covers.
    covered: [[f64; 2]; 2],
}

/// One layer of an [`Image`] read at points, between and past its texels:
/// what [`Image::sampler`] makes. Every reader of an image's values at points
/// (Attribute from Image, Wrangle's `sample`, an engine's CPU path, a probe)
/// reads through it, so they agree to the bit.
///
/// - **Coordinates** run 0–1 over the display window, `u` across and `v` up:
///   (0, 0) is its bottom left corner and (1, 1) its top right. Rows are stored
///   from the top, so `v` 0 reads the bottom row. A texel's center lies half a
///   texel in.
/// - **Values** are the layer's channels as stored, decoded as
///   [`Image::read_pixel`] decodes them, with no color conversion: a pixel
///   outside the data window, in a tile of nothing, or of a held layer reads 0.
/// - **Wrap** ([`Wrap`], across and up) says what a texel past the display
///   window's edge reads: the window tiled, tiled mirrored, its edge texel, or
///   nothing.
/// - **Filter** ([`SampleFilter`]) says which texels a point reads and how they
///   blend. Footprint reads the layer's mean levels, which [`Image::sampler`]
///   derives once ([`Pyramid`]), so one sampler serves every point of a run.
///
/// ```
/// use kinogaki_geometry::image::{Encoding, Format, Image, Rect, Sample, SampleFilter, Wrap};
///
/// // Two pixels side by side: 0 on the left, 1 on the right.
/// let format = Format::rgba(Sample::F32, Encoding::Raw);
/// let pixels = [0.0f32, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0, 1.0];
/// let image = Image::builder(Rect::new([0, 0], [2, 1]).unwrap())
///     .plane(format, &pixels)
///     .build()
///     .unwrap();
/// let read = image.sampler(0, SampleFilter::Bilinear, [Wrap::Clamp; 2]);
/// assert_eq!(read.sample([0.25, 0.5], 1.0), [0.0, 0.0, 0.0, 1.0]); // the left texel's center
/// assert_eq!(read.sample([0.5, 0.5], 1.0), [0.5, 0.5, 0.5, 1.0]); // halfway between
/// ```
///
/// Every tile it reads is shared, so it is cheap to make and to read from any
/// thread.
#[derive(Clone, Debug)]
pub struct Sampler {
    filter: SampleFilter,
    wrap: [Wrap; 2],
    sample: Sample,
    channels: usize,
    /// Bytes a pixel takes.
    pixel: usize,
    /// Level 0 first, then the mean levels Footprint reads; none when there
    /// is nothing to read.
    levels: Vec<Level>,
    /// The display window's width and height, in pixels.
    window: [f64; 2],
    /// Where the display window starts, in pixels from the data window's
    /// first.
    offset: [f64; 2],
}

impl Image {
    /// Layer `layer` ready to be read through `filter`, past the display
    /// window's edges as `wrap` says (across, then up). Footprint derives the
    /// layer's mean levels here, once. No such layer, or a held one, reads 0
    /// everywhere.
    pub fn sampler(&self, layer: usize, filter: SampleFilter, wrap: [Wrap; 2]) -> Sampler {
        let window = self.display.size().map(f64::from);
        let offset =
            [0, 1].map(|k| (i64::from(self.display.min[k]) - i64::from(self.data.min[k])) as f64);
        let mut read = Sampler {
            filter,
            wrap,
            sample: Sample::F32,
            channels: 0,
            pixel: 0,
            levels: Vec::new(),
            window,
            offset,
        };
        let Some(at) = self.layers.get(layer) else {
            return read;
        };
        let Pixels::Tiles(tiles) = &at.pixels else {
            return read;
        };
        let format = &at.format;
        (read.sample, read.channels) = (format.sample, format.channels.len());
        read.pixel = format.pixel_bytes();
        let size = self.data.size();
        let level = |tiles, n: u8| {
            let scale = f64::from(1u32 << n);
            let size = Pyramid::level_size(size, n);
            let edges = |k: usize| {
                let first = (offset[k] / scale).floor();
                [first, ((window[k] + offset[k]) / scale).ceil() - 1.0]
            };
            Level {
                tiles,
                size,
                across: tiles_of(size)[0] as usize,
                scale,
                covered: [edges(0), edges(1)],
            }
        };
        read.levels.push(level(tiles.clone(), 0));
        if filter == SampleFilter::Footprint {
            // Every level down to one texel.
            let top = size[0].max(size[1]).next_power_of_two().ilog2() as u8;
            let mut pyramid = Pyramid::new(format.clone(), size);
            let mut leaf = |at| self.tile(layer, at).cloned();
            for n in 1..=top {
                let across = tiles_of(Pyramid::level_size(size, n));
                let places = (0..across[1]).flat_map(|y| (0..across[0]).map(move |x| [x, y]));
                let tiles: Vec<_> = places.map(|at| pyramid.tile(n, at, &mut leaf)).collect();
                read.levels.push(level(tiles.into(), n));
            }
        }
        read
    }
}

impl Sampler {
    /// The layer at `at` (`u` across and `v` up, 0–1 over the display window):
    /// its channels in order, a channel it doesn't hold 0 (alpha 1).
    /// `footprint` is how far apart the points read lie, in the layer's
    /// pixels: Footprint reads by it, the other filters read the point alone.
    /// A coordinate that isn't finite reads 0.
    pub fn sample(&self, at: [f64; 2], footprint: f64) -> [f32; 4] {
        if self.levels.is_empty() || !(at[0].is_finite() && at[1].is_finite()) {
            return [0.0; 4];
        }
        // In pixels from the display window's top left corner.
        let x = at[0] * self.window[0];
        let y = (1.0 - at[1]) * self.window[1];
        let first = &self.levels[0];
        match self.filter {
            SampleFilter::Nearest => {
                let tx = self.texel(first, 0, (x + self.offset[0]).floor());
                let ty = self.texel(first, 1, (y + self.offset[1]).floor());
                self.read(first, tx, ty)
            }
            SampleFilter::Bilinear => self.bilinear(first, x, y),
            SampleFilter::Bicubic => self.bicubic(x, y),
            SampleFilter::Footprint => {
                let at = footprint.max(1.0).log2();
                let last = self.levels.len() - 1;
                let n = (at.floor() as usize).min(last);
                let fine = self.bilinear(&self.levels[n], x, y);
                let t = (at - n as f64) as f32;
                if n == last || t == 0.0 {
                    return fine;
                }
                lerp(fine, self.bilinear(&self.levels[n + 1], x, y), t)
            }
        }
    }

    /// The four texels of `level` about (`x`, `y`), in pixels from the display
    /// window's top left corner, blended.
    fn bilinear(&self, level: &Level, x: f64, y: f64) -> [f32; 4] {
        let fx = (x + self.offset[0]) / level.scale - 0.5;
        let fy = (y + self.offset[1]) / level.scale - 0.5;
        let (ix, iy) = (fx.floor(), fy.floor());
        let xs = [self.texel(level, 0, ix), self.texel(level, 0, ix + 1.0)];
        let ys = [self.texel(level, 1, iy), self.texel(level, 1, iy + 1.0)];
        let (tx, ty) = ((fx - ix) as f32, (fy - iy) as f32);
        let row = |y| lerp(self.read(level, xs[0], y), self.read(level, xs[1], y), tx);
        lerp(row(ys[0]), row(ys[1]), ty)
    }

    /// The sixteen texels about (`x`, `y`), through Keys' cubic.
    fn bicubic(&self, x: f64, y: f64) -> [f32; 4] {
        let level = &self.levels[0];
        let fx = x + self.offset[0] - 0.5;
        let fy = y + self.offset[1] - 0.5;
        let (ix, iy) = (fx.floor(), fy.floor());
        let (wx, wy) = (keys((fx - ix) as f32), keys((fy - iy) as f32));
        let xs: [_; 4] = std::array::from_fn(|k| self.texel(level, 0, ix + k as f64 - 1.0));
        let mut out = [0.0f32; 4];
        for (j, weight) in wy.into_iter().enumerate() {
            let ty = self.texel(level, 1, iy + j as f64 - 1.0);
            let mut row = [0.0f32; 4];
            for (&tx, w) in xs.iter().zip(wx) {
                let texel = self.read(level, tx, ty);
                (0..4).for_each(|c| row[c] += texel[c] * w);
            }
            (0..4).for_each(|c| out[c] += row[c] * weight);
        }
        out
    }

    /// Along axis `k` of `level`, the texel that texel `i`'s tap reads (`i`
    /// whole, counted from the data window's first pixel), past the display
    /// window's edges as the wrap says: the texel holding its center once
    /// wrapped. `None` reads nothing.
    fn texel(&self, level: &Level, k: usize, i: f64) -> Option<u32> {
        let (window, scale) = (self.window[k], level.scale);
        // Its center, from the display window's start.
        let center = (i + 0.5) * scale - self.offset[k];
        let center = match self.wrap[k] {
            Wrap::Repeat => center.rem_euclid(window),
            Wrap::MirrorRepeat => {
                let m = center.rem_euclid(2.0 * window);
                if m > window { 2.0 * window - m } else { m }
            }
            Wrap::Clamp => center,
            Wrap::Black if (0.0..window).contains(&center) => center,
            Wrap::Black => return None,
        };
        let [first, last] = level.covered[k];
        let at = ((center + self.offset[k]) / scale)
            .floor()
            .clamp(first, last);
        (at >= 0.0 && at < f64::from(level.size[k])).then_some(at as u32)
    }

    /// Texel (`x`, `y`) of `level` decoded as stored; nothing reads 0.
    fn read(&self, level: &Level, x: Option<u32>, y: Option<u32>) -> [f32; 4] {
        let (Some(x), Some(y)) = (x, y) else {
            return [0.0; 4];
        };
        let tile = &level.tiles[(y / TILE) as usize * level.across + (x / TILE) as usize];
        let Some(tile) = tile else {
            return [0.0; 4];
        };
        let place = ((y % TILE) * TILE + x % TILE) as usize * self.pixel;
        decode(self.sample, self.channels, &tile.bytes()[place..])
    }
}

/// `a` to `b` by `t`.
fn lerp(a: [f32; 4], b: [f32; 4], t: f32) -> [f32; 4] {
    std::array::from_fn(|c| a[c] + (b[c] - a[c]) * t)
}

/// Keys' cubic weights (a = −0.5) of the taps one before, at, one past and two
/// past a point `t` past the first texel's center.
fn keys(t: f32) -> [f32; 4] {
    let (t2, t3) = (t * t, t * t * t);
    [
        (-t3 + 2.0 * t2 - t) * 0.5,
        (3.0 * t3 - 5.0 * t2 + 2.0) * 0.5,
        (-3.0 * t3 + 4.0 * t2 + t) * 0.5,
        (t3 - t2) * 0.5,
    ]
}
