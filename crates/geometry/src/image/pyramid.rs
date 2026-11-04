//! [`Pyramid`]: smaller levels of an image's tiles, derived on demand.
//!
//! Level 0 is the image's own tiles, as a caller's `leaf` gives them (a
//! caller may first convert them to linear light, which a mean needs). A
//! tile of level `n` is the mean of the four tiles of level `n − 1` under it,
//! kept while those four are the same tiles, so after an edit only what lies
//! over a changed tile is derived again. Only pixels inside the level count:
//! the padding past a window's edge doesn't darken it.
use super::{Format, TILE, Tile};
use std::collections::HashMap;

struct Kept {
    /// The keys of the four tiles it was made of (0: nothing there).
    under: [u64; 4],
    tile: Option<Tile>,
}

/// The tiles of one layer's smaller levels, derived on demand.
pub struct Pyramid {
    format: Format,
    size: [u32; 2],
    kept: HashMap<(u8, [u32; 2]), Kept>,
}

impl Pyramid {
    /// The levels of a layer of `format` over a data window of `size`.
    pub fn new(format: Format, size: [u32; 2]) -> Self {
        Self {
            format,
            size,
            kept: HashMap::new(),
        }
    }

    /// A level's width and height, in its own pixels.
    pub fn level_size(size: [u32; 2], level: u8) -> [u32; 2] {
        size.map(|n| n.div_ceil(1u32 << level).max(1))
    }

    /// How many levels until one tile holds it all (at least 1).
    pub fn levels(size: [u32; 2]) -> u8 {
        let side = size[0].max(size[1]).max(1);
        (side.div_ceil(TILE).next_power_of_two().ilog2() + 1) as u8
    }

    /// Tile `at` of `level`, `leaf` giving level 0's; `None` holds nothing.
    pub fn tile(
        &mut self,
        level: u8,
        at: [u32; 2],
        leaf: &mut dyn FnMut([u32; 2]) -> Option<Tile>,
    ) -> Option<Tile> {
        if level == 0 {
            return leaf(at);
        }
        let under: [Option<Tile>; 4] = std::array::from_fn(|k| {
            let k = k as u32;
            self.tile(level - 1, [at[0] * 2 + k % 2, at[1] * 2 + k / 2], leaf)
        });
        let keys = [0, 1, 2, 3].map(|k| under[k].as_ref().map_or(0, Tile::key));
        if let Some(kept) = self.kept.get(&(level, at))
            && kept.under == keys
        {
            return kept.tile.clone();
        }
        let tile = (keys != [0; 4]).then(|| self.mean(level, at, &under));
        let kept = Kept {
            under: keys,
            tile: tile.clone(),
        };
        self.kept.insert((level, at), kept);
        tile
    }

    /// Lets go of every tile kept.
    pub fn clear(&mut self) {
        self.kept.clear();
    }

    /// Tile `at` of `level` made from the tiles `under` it.
    fn mean(&self, level: u8, at: [u32; 2], under: &[Option<Tile>; 4]) -> Tile {
        let sample = self.format.sample;
        let channels = self.format.channels.len();
        let pixel = self.format.pixel_bytes();
        let side = TILE as usize;
        let [width, height] = Self::level_size(self.size, level - 1).map(|n| n as usize);
        let mut out = vec![0; self.format.tile_bytes()];
        for y in 0..side {
            for x in 0..side {
                let mut sum = [0.0f32; 4];
                let mut count = 0.0f32;
                for (dx, dy) in [(0, 0), (1, 0), (0, 1), (1, 1)] {
                    // The pixel of the level below, by its place in the level.
                    let px = (at[0] as usize * side + x) * 2 + dx;
                    let py = (at[1] as usize * side + y) * 2 + dy;
                    if px >= width || py >= height {
                        continue;
                    }
                    count += 1.0;
                    let tile = (py / side % 2) * 2 + px / side % 2;
                    let Some(t) = &under[tile] else { continue };
                    let place = (py % side * side + px % side) * pixel;
                    let bytes = &t.bytes()[place..];
                    for (k, s) in sum.iter_mut().enumerate().take(channels) {
                        *s += sample.read(&bytes[k * sample.bytes()..]);
                    }
                }
                if count > 0.0 {
                    let place = (y * side + x) * pixel;
                    for (k, s) in sum.iter().enumerate().take(channels) {
                        sample.write(s / count, &mut out[place + k * sample.bytes()..]);
                    }
                }
            }
        }
        Tile::new(out)
    }
}
