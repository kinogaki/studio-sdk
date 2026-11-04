//! [`SurfaceTextures`]: the images a surface's polygons show in Shaded, each
//! a [`SurfaceImage`] a polygon's color multiplies by, read through a
//! [`TextureCorner`] per polygon vertex.
use crate::packet::{Column, SurfaceSource};
use std::ops::Range;
use std::sync::Arc;

/// An image as a shaded view shows it: a square of RGBA8 texels with every
/// mip level down to one texel, its first row at `v` 0 and its alpha 1. Its
/// colors are linear in the working space, coded with sRGB's curve so a GPU
/// filters them linearly. It is an approximation, as a view's look is: Render
/// views read the file itself.
pub struct SurfaceImage {
    side: u32,
    levels: Vec<Vec<u8>>,
}

impl SurfaceImage {
    /// The sides an image can have, so a renderer keeps same-sized images
    /// together and a material's uvs mean the same on any of them.
    pub const SIDES: [u32; 4] = [256, 512, 1024, 2048];

    /// An image of `width` × `height` texels whose rows `row` fills, first to
    /// last, with linear colors in the working space. It takes the smallest
    /// side holding both, the largest when none does: each of its texels
    /// averages those it covers, or repeats the one it lies on, so small
    /// images stay as crisp as they were.
    ///
    /// # Errors
    /// What `row` returns.
    pub fn from_rows<E>(
        width: u32,
        height: u32,
        mut row: impl FnMut(u32, &mut [[f64; 3]]) -> Result<(), E>,
    ) -> Result<Self, E> {
        let largest = Self::SIDES[Self::SIDES.len() - 1];
        let side = (Self::SIDES.into_iter())
            .find(|&s| s >= width.max(height))
            .unwrap_or(largest);
        let across = spans(width as usize, side as usize);
        let down = spans(height as usize, side as usize);
        let mut level = vec![[0.0f32; 3]; (side * side) as usize];
        let mut texels = vec![[0.0f64; 3]; width as usize];
        let mut first = 0;
        for y in 0..height as usize {
            row(y as u32, &mut texels)?;
            while !down[first].contains(&y) {
                first += 1;
            }
            let rows = (first..down.len()).take_while(|&d| down[d].contains(&y));
            for at in rows {
                let line = &mut level[at * side as usize..][..side as usize];
                for (sum, span) in line.iter_mut().zip(&across) {
                    for texel in &texels[span.clone()] {
                        for k in 0..3 {
                            sum[k] += texel[k] as f32;
                        }
                    }
                }
            }
        }
        for (at, sum) in level.iter_mut().enumerate() {
            let (x, y) = (at % side as usize, at / side as usize);
            let count = (across[x].len() * down[y].len()) as f32;
            *sum = sum.map(|v| v / count);
        }
        let mut levels = vec![coded(&level)];
        let mut size = side as usize;
        while size > 1 {
            level = halved(&level, size);
            levels.push(coded(&level));
            size /= 2;
        }
        Ok(Self { side, levels })
    }

    /// Its side, in texels.
    pub fn side(&self) -> u32 {
        self.side
    }

    /// Its RGBA8 texels by mip level, the largest first.
    pub fn levels(&self) -> &[Vec<u8>] {
        &self.levels
    }

    /// The bytes of all its levels.
    pub fn bytes(&self) -> u64 {
        self.levels.iter().map(|l| l.len() as u64).sum()
    }
}

/// The texels of `from` each of `side` texels covers, or the one it lies on.
fn spans(from: usize, side: usize) -> Vec<Range<usize>> {
    (0..side)
        .map(|d| {
            let start = d * from / side;
            start..((d + 1) * from / side).max(start + 1)
        })
        .collect()
}

/// The level of half `side`: each texel the average of the four it covers.
fn halved(level: &[[f32; 3]], side: usize) -> Vec<[f32; 3]> {
    let half = side / 2;
    let mut out = Vec::with_capacity(half * half);
    for y in 0..half {
        for x in 0..half {
            let at = |dx: usize, dy: usize| level[(y * 2 + dy) * side + x * 2 + dx];
            let four = [at(0, 0), at(1, 0), at(0, 1), at(1, 1)];
            out.push(std::array::from_fn(|k| {
                four.iter().map(|t| t[k]).sum::<f32>() / 4.0
            }));
        }
    }
    out
}

/// `level`'s linear colors as sRGB-coded RGBA8, opaque.
fn coded(level: &[[f32; 3]]) -> Vec<u8> {
    let code = |v: f32| {
        let v = v.clamp(0.0, 1.0);
        let c = match v <= 0.003_130_8 {
            true => 12.92 * v,
            false => 1.055 * v.powf(1.0 / 2.4) - 0.055,
        };
        (c * 255.0 + 0.5) as u8
    };
    let opaque = |[r, g, b]: &[f32; 3]| [code(*r), code(*g), code(*b), 255];
    level.iter().flat_map(opaque).collect()
}

/// What a polygon vertex reads: its texture coordinate and its image.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct TextureCorner {
    /// Its coordinates in the image: `v` 0 its first row. They repeat.
    pub uv: [f32; 2],
    /// Its image in [`SurfaceTextures::images`], plus one (0: none, the
    /// polygon shows its color alone). All of a polygon's vertices name one.
    pub image: u32,
}

/// The images a surface's polygons show, each multiplying a polygon's color.
/// Allocations are their identities, as a packet's: two are equal when they
/// are the same allocations, and a renderer uploads what is new.
#[derive(Clone)]
pub struct SurfaceTextures {
    /// One a polygon vertex, as a packet numbers them (`Vertex::component`).
    pub corners: Arc<[TextureCorner]>,
    /// The images the corners name; `None`: not read yet, or unreadable, so
    /// its polygons show their color alone.
    pub images: Arc<[Option<Arc<SurfaceImage>>]>,
}

impl SurfaceTextures {
    /// Whether they were made for `surface` (not for another packet, a cook
    /// on its way).
    pub fn fits(&self, surface: &dyn SurfaceSource) -> bool {
        self.corners.len() == surface.column_len(Column::Vertices)
    }

    /// Their allocations, which name them.
    pub fn key(&self) -> [usize; 2] {
        [
            Arc::as_ptr(&self.corners).cast::<TextureCorner>() as usize,
            Arc::as_ptr(&self.images).cast::<Option<Arc<SurfaceImage>>>() as usize,
        ]
    }
}

impl PartialEq for SurfaceTextures {
    fn eq(&self, other: &Self) -> bool {
        self.key() == other.key()
    }
}

impl std::fmt::Debug for SurfaceTextures {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SurfaceTextures({} corners, {} images)",
            self.corners.len(),
            self.images.len()
        )
    }
}

#[cfg(test)]
mod tests {
    use super::SurfaceImage;

    fn image(width: u32, height: u32, at: impl Fn(u32, u32) -> [f64; 3]) -> SurfaceImage {
        let filled = SurfaceImage::from_rows(width, height, |y, row| {
            for (x, texel) in row.iter_mut().enumerate() {
                *texel = at(x as u32, y);
            }
            Ok::<(), ()>(())
        });
        filled.expect("a toy image")
    }

    fn texel(image: &SurfaceImage, level: usize, x: u32, y: u32) -> [u8; 4] {
        let side = image.side() >> level;
        let at = ((y * side + x) * 4) as usize;
        std::array::from_fn(|k| image.levels()[level][at + k])
    }

    #[test]
    fn it_takes_the_smallest_side_holding_it_and_every_level() {
        let sized = |width, height| image(width, height, |_, _| [1.0; 3]);
        assert_eq!(sized(16, 8).side(), 256);
        assert_eq!(sized(257, 3).side(), 512);
        assert_eq!(sized(8192, 4096).side(), 2048);
        let small = sized(16, 8);
        assert_eq!(small.levels().len(), 9, "256 down to 1");
        assert_eq!(small.levels()[8].len(), 4);
        let texels: u64 = (0..9).map(|l| 4 * (256u64 >> l) * (256u64 >> l)).sum();
        assert_eq!(small.bytes(), texels);
    }

    #[test]
    fn a_small_image_repeats_its_texels_and_a_large_one_averages_them() {
        // Two texels side by side: the left half of the square, the right.
        let two = image(2, 1, |x, _| [f64::from(x), 0.0, 0.0]);
        assert_eq!(texel(&two, 0, 127, 0), [0, 0, 0, 255]);
        assert_eq!(texel(&two, 0, 128, 255), [255, 0, 0, 255]);
        // 4096 stripes of black and white in 2048 texels: linear grey, coded.
        let stripes = image(4096, 1, |x, _| [f64::from(x % 2); 3]);
        assert_eq!(texel(&stripes, 0, 1000, 1000)[0], 188);
    }

    #[test]
    fn levels_average_in_linear_light() {
        let halves = image(2, 2, |x, _| [f64::from(x); 3]);
        let last = halves.levels().len() - 1;
        assert_eq!(texel(&halves, last, 0, 0)[0], 188, "half of 1, coded");
    }

    #[test]
    fn a_row_that_fails_says_why() {
        let failed = SurfaceImage::from_rows(2, 2, |_, _| Err("no row"));
        assert!(matches!(failed, Err("no row")));
    }
}
