//! [`Image::converted`]: the one converter. A layer made anew tile by tile on
//! the process's workers, each source tile read once as RGBA floats, its
//! pixels taken through a conversion in runs of a tile, then reduced by
//! whole-pixel boxes and written in the format asked for. No whole-image
//! plane is ever made: the source tiles stay shared, and the result is tiled
//! as it is made.
use super::{Format, Image, Rect, TILE, Tile, decode, spread, tiles_of};
use crate::error::Error;
use crate::parallel;

/// Pixels a tile holds.
const PIXELS: usize = (TILE * TILE) as usize;

/// What a conversion makes of each run of pixels, in place: RGBA floats as
/// the layer stores them (one channel spread to grey, two grey and alpha, a
/// missing alpha 1).
pub type Convert<'a> = dyn Fn(&mut [[f32; 4]]) -> Result<(), Error> + Sync + 'a;

/// The box side a layer of `size` reduces by to fit `most` pixels a side
/// (none: 1, as it is).
///
/// ```
/// use kinogaki_geometry::image::reduction;
/// assert_eq!(reduction([8192, 4096], Some(2048)), 4);
/// assert_eq!(reduction([300, 200], Some(2048)), 1);
/// assert_eq!(reduction([300, 200], None), 1);
/// ```
pub fn reduction(size: [u32; 2], most: Option<u32>) -> u32 {
    let side = size[0].max(size[1]).max(1);
    most.map_or(1, |most| side.div_ceil(most.max(1)))
}

impl Tile {
    /// This tile of a layer of `format` made anew as `into` (its channels
    /// the first of RGBA): its pixels read as RGBA floats, run through
    /// `convert` and written. What [`Image::converted`] does to each tile,
    /// for a reader converting the tiles it shows as it shows them.
    ///
    /// # Errors
    /// What `convert` returns.
    pub fn converted(
        &self,
        format: &Format,
        into: &Format,
        convert: &Convert<'_>,
    ) -> Result<Tile, Error> {
        let mut pixels = vec![[0.0f32; 4]; PIXELS];
        read(format, Some(self), &mut pixels);
        convert(&mut pixels)?;
        Ok(write(&pixels, into))
    }
}

impl Image {
    /// Layer `layer` made anew as `format` (its channels the first of RGBA),
    /// tile by tile on the process's workers: each tile read once as RGBA
    /// floats and run through `convert`, then each `by` × `by` block of
    /// converted pixels averaged into one pixel of the result. The result
    /// holds that one layer: over this image's windows when `by` is 1, else
    /// over a window from (0, 0) of the reduced size.
    ///
    /// ```
    /// use kinogaki_geometry::image::{Encoding, Format, Image, Rect, Sample};
    ///
    /// let grey = Format { channels: vec!["Y".into()].into(), ..Format::rgba(Sample::U8, Encoding::Raw) };
    /// let image = Image::builder(Rect::new([0, 0], [2, 2]).unwrap())
    ///     .plane(grey, &[0u8, 255, 255, 255])
    ///     .build()
    ///     .unwrap();
    /// let halves = Format::rgba(Sample::F16, Encoding::Raw);
    /// let mean = image.converted(0, 2, halves, &|_| Ok(())).unwrap();
    /// assert_eq!(mean.read_pixel(0, 0, 0), [0.75, 0.75, 0.75, 1.0]);
    /// ```
    ///
    /// # Errors
    /// No such layer, a layer whose pixels the host holds, or what `convert`
    /// returns.
    pub fn converted(
        &self,
        layer: usize,
        by: u32,
        format: Format,
        convert: &Convert<'_>,
    ) -> Result<Image, Error> {
        let source = (self.layers.get(layer))
            .ok_or_else(|| Error::invalid(format_args!("no layer {layer}")))?;
        if source.held().is_some() {
            return Err(Error::invalid("the image's pixels aren't in memory"));
        }
        let by = by.max(1);
        let size = self.data.size();
        let out = size.map(|n| n.div_ceil(by).max(1));
        let across = tiles_of(out);
        let count = (across[0] * across[1]) as usize;
        let tiles = parallel::map_blocks(count, 1, |range| {
            (range.map(|t| {
                let at = [t as u32 % across[0], t as u32 / across[0]];
                match by {
                    1 => self.tile(layer, at).map_or_else(
                        || self.converted_tile(layer, at, (by, out), convert, &format),
                        |tile| tile.converted(source.format(), &format, convert).map(Some),
                    ),
                    _ => self.converted_tile(layer, at, (by, out), convert, &format),
                }
            }))
            .collect::<Result<Vec<_>, Error>>()
        })?;
        let tiles = tiles.into_iter().flatten().collect();
        let builder = match by {
            1 => Image::framed(self.display, self.data),
            _ => Image::builder(Rect::new([0, 0], out.map(|n| n as i32))?),
        };
        builder.aspect(self.aspect).tiles(format, tiles).build()
    }

    /// The result's tile `at`: the source tiles under it read, converted and
    /// reduced into its pixels; none when every value is 0.
    fn converted_tile(
        &self,
        layer: usize,
        at: [u32; 2],
        (by, out): (u32, [u32; 2]),
        convert: &Convert<'_>,
        format: &Format,
    ) -> Result<Option<Tile>, Error> {
        let size = self.data.size();
        // The result's pixels this tile holds, and the source's under them.
        let first = at.map(|n| n * TILE);
        let last = [0, 1].map(|k| (first[k] + TILE).min(out[k]));
        let from = first.map(|n| n * by);
        let to = [0, 1].map(|k| (last[k] * by).min(size[k]));
        let mut sums = vec![[0.0f32; 4]; PIXELS];
        let mut counts = vec![0u32; PIXELS];
        let mut pixels = vec![[0.0f32; 4]; PIXELS];
        for sy in from[1] / TILE..to[1].div_ceil(TILE) {
            for sx in from[0] / TILE..to[0].div_ceil(TILE) {
                let format = self.layers[layer].format();
                read(format, self.tile(layer, [sx, sy]), &mut pixels);
                convert(&mut pixels)?;
                let [x0, y0] = [sx, sy].map(|n| n * TILE);
                for y in y0.max(from[1])..(y0 + TILE).min(to[1]) {
                    for x in x0.max(from[0])..(x0 + TILE).min(to[0]) {
                        let p = pixels[((y - y0) * TILE + x - x0) as usize];
                        let o = ((y / by - first[1]) * TILE + x / by - first[0]) as usize;
                        (0..4).for_each(|k| sums[o][k] += p[k]);
                        counts[o] += 1;
                    }
                }
            }
        }
        for (sum, &n) in sums.iter_mut().zip(&counts) {
            *sum = sum.map(|v| v / n.max(1) as f32);
        }
        let made = write(&sums, format);
        Ok(made.bytes().iter().any(|&b| b != 0).then_some(made))
    }
}

/// `tile`'s pixels, of a layer of `format`, as RGBA floats as stored (one
/// channel spread to grey, two grey and alpha, a missing alpha 1); a tile of
/// nothing all zeros, alpha too when it has one.
fn read(format: &Format, tile: Option<&Tile>, pixels: &mut [[f32; 4]]) {
    let channels = format.channels.len();
    let alpha = matches!(channels, 2 | 4);
    let Some(tile) = tile else {
        pixels.fill([0.0, 0.0, 0.0, if alpha { 0.0 } else { 1.0 }]);
        return;
    };
    let (bytes, size) = (tile.bytes(), format.pixel_bytes());
    for (p, pixel) in pixels.iter_mut().enumerate() {
        *pixel = spread(
            channels,
            decode(format.sample, channels, &bytes[p * size..]),
        );
    }
}

/// `pixels`, RGBA floats, as a tile of `format`: its channels the first of
/// RGBA.
fn write(pixels: &[[f32; 4]], format: &Format) -> Tile {
    let channels = format.channels.len();
    let width = format.sample.bytes();
    let mut bytes = vec![0u8; PIXELS * channels * width];
    for (o, pixel) in pixels.iter().enumerate() {
        for (k, &v) in pixel[..channels].iter().enumerate() {
            (format.sample).write(v, &mut bytes[(o * channels + k) * width..]);
        }
    }
    Tile::new(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::image::{Encoding, Sample};

    fn floats(w: i32, h: i32, at: impl Fn(i32, i32) -> [f32; 4]) -> Image {
        let at = &at;
        let px: Vec<f32> = (0..h)
            .flat_map(|y| (0..w).flat_map(move |x| at(x, y)))
            .collect();
        let format = Format::rgba(Sample::F32, Encoding::color("ACEScg", false));
        Image::builder(Rect::new([0, 0], [w, h]).unwrap())
            .plane(format, &px)
            .build()
            .unwrap()
    }

    /// Past a tile's side both ways: every pixel lands where it was, and
    /// a conversion runs on each.
    #[test]
    fn a_layer_converts_a_tile_at_a_time_where_its_pixels_are() {
        let image = floats(130, 70, |x, y| [x as f32, y as f32, 0.5, 1.0]);
        let double = |run: &mut [[f32; 4]]| {
            run.iter_mut().for_each(|p| p[2] *= 2.0);
            Ok(())
        };
        let halves = Format::rgba(Sample::F16, Encoding::Raw);
        let made = image.converted(0, 1, halves, &double).unwrap();
        assert_eq!(
            (made.data(), made.display()),
            (image.data(), image.display())
        );
        for (x, y) in [(0, 0), (63, 0), (64, 1), (129, 69), (100, 65)] {
            assert_eq!(made.read_pixel(0, x, y), [x as f32, y as f32, 1.0, 1.0]);
        }
    }

    /// Each box's mean, the boxes at the edges counting what they cover.
    #[test]
    fn a_reduced_layer_holds_each_boxs_mean() {
        let image = floats(5, 3, |x, _| [x as f32, 0.0, 0.0, 1.0]);
        let format = Format::rgba(Sample::F32, Encoding::Raw);
        let made = image.converted(0, 2, format, &|_| Ok(())).unwrap();
        assert_eq!(made.data().size(), [3, 2]);
        assert_eq!(made.read_pixel(0, 0, 0)[0], 0.5);
        assert_eq!(made.read_pixel(0, 2, 1)[0], 4.0, "the last column alone");
    }

    #[test]
    fn a_conversion_that_fails_fails_the_layer() {
        let image = floats(2, 2, |_, _| [1.0; 4]);
        let format = Format::rgba(Sample::F16, Encoding::Raw);
        let failed = image.converted(0, 1, format.clone(), &|_| Err(Error::invalid("no")));
        assert!(failed.is_err());
        assert!(
            image.converted(3, 1, format, &|_| Ok(())).is_err(),
            "no layer 3"
        );
    }
}
