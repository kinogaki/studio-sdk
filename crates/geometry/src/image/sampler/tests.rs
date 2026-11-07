use super::*;
use crate::image::{Encoding, Format, HeldPixels, Rect};

/// A float image of `w` × `h` texels over `display`'s data window `data`:
/// each texel's red is its column, its green its row counted from the bottom,
/// blue a quarter and alpha 1.
fn ramp_over(display: Rect, data: Rect) -> Image {
    let [w, h] = data.size();
    let mut px = Vec::new();
    for row in 0..h {
        for x in 0..w {
            px.extend([x as f32, (h - 1 - row) as f32, 0.25, 1.0]);
        }
    }
    let format = Format::rgba(Sample::F32, Encoding::Raw);
    Image::framed(display, data)
        .plane(format, &px)
        .build()
        .unwrap()
}

fn window(w: i32, h: i32) -> Rect {
    Rect::new([0, 0], [w, h]).unwrap()
}

fn ramp(w: i32, h: i32) -> Image {
    ramp_over(window(w, h), window(w, h))
}

/// Texel (`x`, `y`)'s center (`y` from the bottom) of a `w` × `h` image.
fn center(x: f64, y: f64, w: f64, h: f64) -> [f64; 2] {
    [(x + 0.5) / w, (y + 0.5) / h]
}

fn near(a: [f32; 4], b: [f32; 4]) -> bool {
    a.iter().zip(b).all(|(a, b)| (a - b).abs() < 1e-4)
}

const FILTERS: [SampleFilter; 4] = [
    SampleFilter::Nearest,
    SampleFilter::Bilinear,
    SampleFilter::Bicubic,
    SampleFilter::Footprint,
];

#[test]
fn every_filter_and_wrap_reads_a_texel_center_as_stored() {
    let image = ramp(7, 5);
    for filter in FILTERS {
        for wrap in Wrap::ALL {
            let read = image.sampler(0, filter, [wrap; 2]);
            for y in 0..5 {
                for x in 0..7 {
                    let at = center(f64::from(x), f64::from(y), 7.0, 5.0);
                    let want = [x as f32, y as f32, 0.25, 1.0];
                    assert_eq!(read.sample(at, 1.0), want, "{filter:?} {wrap:?} ({x}, {y})");
                }
            }
        }
    }
}

#[test]
fn v_runs_up_from_the_bottom_row() {
    let image = ramp(4, 4);
    let read = image.sampler(0, SampleFilter::Nearest, [Wrap::Clamp; 2]);
    assert_eq!(read.sample([0.1, 0.01], 1.0)[1], 0.0, "the bottom row");
    assert_eq!(read.sample([0.1, 0.99], 1.0)[1], 3.0, "the top row");
    // The bottom row is the last stored.
    assert_eq!(image.read_pixel(0, 0, 3)[1], 0.0);
}

#[test]
fn bilinear_and_bicubic_reproduce_a_ramp_between_texels() {
    let image = ramp(16, 8);
    for filter in [SampleFilter::Bilinear, SampleFilter::Bicubic] {
        let read = image.sampler(0, filter, [Wrap::Clamp; 2]);
        // Two texels in from each edge, where every tap lies inside.
        for k in 0..50 {
            let x = 2.0 + 11.0 * f64::from(k) / 49.0;
            let y = 2.0 + 3.0 * f64::from(k % 7) / 6.0;
            let got = read.sample([(x + 0.5) / 16.0, (y + 0.5) / 8.0], 1.0);
            let want = [x as f32, y as f32, 0.25, 1.0];
            assert!(near(got, want), "{filter:?} at ({x}, {y}): {got:?}");
        }
    }
}

#[test]
fn nearest_reads_the_texel_the_point_lies_in() {
    let image = ramp(4, 2);
    let read = image.sampler(0, SampleFilter::Nearest, [Wrap::Clamp; 2]);
    // Just inside texel 2's left edge and texel 1's right edge.
    assert_eq!(read.sample([0.501, 0.25], 1.0)[0], 2.0);
    assert_eq!(read.sample([0.499, 0.25], 1.0)[0], 1.0);
}

#[test]
fn each_wrap_reads_past_the_edges_as_it_says() {
    let image = ramp(4, 4);
    let nearest = |wrap| image.sampler(0, SampleFilter::Nearest, [wrap, Wrap::Clamp]);
    let red = |wrap, u: f64| nearest(wrap).sample([u, 0.5], 1.0)[0];
    // Texel 1's center is u 0.375.
    for shift in [-2.0, -1.0, 1.0, 3.0] {
        assert_eq!(red(Wrap::Repeat, 0.375 + shift), 1.0, "repeat by {shift}");
    }
    assert_eq!(red(Wrap::MirrorRepeat, -0.375), 1.0, "mirrored at 0");
    assert_eq!(red(Wrap::MirrorRepeat, 1.375), 2.0, "mirrored at 1");
    assert_eq!(
        red(Wrap::MirrorRepeat, 2.375),
        1.0,
        "the next copy is upright"
    );
    assert_eq!(red(Wrap::Clamp, -3.0), 0.0);
    assert_eq!(red(Wrap::Clamp, 5.0), 3.0);
    let black = nearest(Wrap::Black);
    assert_eq!(black.sample([1.5, 0.5], 1.0), [0.0; 4]);
    assert_eq!(black.sample([-0.01, 0.5], 1.0), [0.0; 4]);
    // At the right edge bilinear blends the last texel with what lies past it.
    let edge = |wrap| {
        let read = image.sampler(0, SampleFilter::Bilinear, [wrap, Wrap::Clamp]);
        read.sample([1.0, 0.375], 1.0)
    };
    assert_eq!(
        edge(Wrap::Repeat),
        [1.5, 1.0, 0.25, 1.0],
        "3 and the first, 0"
    );
    assert_eq!(edge(Wrap::Clamp), [3.0, 1.0, 0.25, 1.0]);
    assert_eq!(edge(Wrap::MirrorRepeat), [3.0, 1.0, 0.25, 1.0]);
    assert_eq!(edge(Wrap::Black), [1.5, 0.5, 0.125, 0.5], "half nothing");
}

#[test]
fn pixels_outside_the_data_window_read_nothing() {
    // A 2 × 2 data window in the middle of a 4 × 4 display window.
    let data = Rect::new([1, 1], [3, 3]).unwrap();
    let image = ramp_over(window(4, 4), data);
    let read = image.sampler(0, SampleFilter::Nearest, [Wrap::Repeat; 2]);
    // Display texel (1, 1) from the top left is data texel (0, 0), the top
    // row of the data: green 1.
    let top_left = center(1.0, 2.0, 4.0, 4.0);
    assert_eq!(read.sample(top_left, 1.0), [0.0, 1.0, 0.25, 1.0]);
    assert_eq!(read.sample(center(0.0, 0.0, 4.0, 4.0), 1.0), [0.0; 4]);
    assert_eq!(read.sample(center(3.0, 2.0, 4.0, 4.0), 1.0), [0.0; 4]);
    // Data past the display window is never wrapped in: Repeat tiles the
    // display window.
    let wide = ramp_over(window(2, 1), Rect::new([0, 0], [4, 1]).unwrap());
    let read = wide.sampler(0, SampleFilter::Nearest, [Wrap::Repeat; 2]);
    assert_eq!(read.sample([0.75, 0.5], 1.0)[0], 1.0);
    assert_eq!(
        read.sample([1.25, 0.5], 1.0)[0],
        0.0,
        "texel 0 again, not 2"
    );
}

/// An 8 × 8 checker of 2 × 2 blocks, 0 and 1 in every channel.
fn checker() -> Image {
    let mut px = Vec::new();
    for y in 0..8 {
        for x in 0..8 {
            px.push(if (x / 2 + y / 2) % 2 == 0 {
                1.0f32
            } else {
                0.0
            });
        }
    }
    let format = Format {
        channels: Arc::new([Arc::from("Y")]),
        ..Format::rgba(Sample::F32, Encoding::Raw)
    };
    Image::builder(window(8, 8))
        .plane(format, &px)
        .build()
        .unwrap()
}

#[test]
fn footprint_reads_the_level_its_spacing_matches() {
    let image = checker();
    let read = image.sampler(0, SampleFilter::Footprint, [Wrap::Repeat; 2]);
    let at = |x: f64, y: f64, footprint| read.sample([x / 8.0, 1.0 - y / 8.0], footprint)[0];
    // Level 0, a texel apart (or closer): the checker's texels.
    assert_eq!(at(0.5, 0.5, 1.0), 1.0);
    assert_eq!(at(2.5, 0.5, 0.25), 0.0);
    // Level 1, two texels apart: its texels are the blocks.
    assert_eq!(at(1.0, 1.0, 2.0), 1.0);
    assert_eq!(at(3.0, 1.0, 2.0), 0.0);
    // Level 2, four apart: each texel the mean of a block of each.
    assert_eq!(at(1.0, 1.0, 4.0), 0.5);
    assert_eq!(at(6.0, 2.0, 4.0), 0.5);
    // Between: blended by the logarithm.
    assert!((at(1.0, 1.0, 8f64.sqrt()) - 0.75).abs() < 1e-6);
    // Far apart: the last level, one texel.
    assert_eq!(at(3.0, 5.0, 1000.0), 0.5);
    // The other filters read the point alone.
    let bilinear = image.sampler(0, SampleFilter::Bilinear, [Wrap::Repeat; 2]);
    assert_eq!(bilinear.sample([0.5 / 8.0, 1.0 - 0.5 / 8.0], 4.0)[0], 1.0);
}

#[derive(Debug)]
struct Held;

impl HeldPixels for Held {
    fn key(&self) -> u64 {
        1
    }
}

#[test]
fn nothing_to_read_reads_nothing() {
    let image = ramp(4, 4);
    for filter in FILTERS {
        let missing = image.sampler(3, filter, [Wrap::Repeat; 2]);
        assert_eq!(missing.sample([0.5, 0.5], 1.0), [0.0; 4]);
        let read = image.sampler(0, filter, [Wrap::Repeat; 2]);
        assert_eq!(read.sample([f64::NAN, 0.5], 1.0), [0.0; 4]);
        assert_eq!(read.sample([0.5, f64::INFINITY], 1.0), [0.0; 4]);
    }
    let format = Format::rgba(Sample::F16, Encoding::Raw);
    let held = Image::builder(window(4, 4))
        .held(format, Arc::new(Held))
        .build()
        .unwrap();
    let read = held.sampler(0, SampleFilter::Bilinear, [Wrap::Clamp; 2]);
    assert_eq!(read.sample([0.5, 0.5], 1.0), [0.0; 4]);
}

#[test]
fn values_decode_as_read_pixel_decodes_them() {
    // Eight-bit RGB: 0–255 as 0–1, alpha 1 where the layer has none.
    let format = Format {
        channels: Arc::new(["R", "G", "B"].map(Arc::from)),
        ..Format::rgba(Sample::U8, Encoding::color("sRGB", false))
    };
    let pixels: Vec<u8> = (0..3 * 70 * 3).map(|k| (k % 251) as u8).collect();
    let image = Image::builder(window(70, 3))
        .plane(format, &pixels)
        .build()
        .unwrap();
    let read = image.sampler(0, SampleFilter::Nearest, [Wrap::Clamp; 2]);
    for (x, y) in [(0, 0), (63, 1), (64, 2), (69, 0)] {
        let at = center(f64::from(x), f64::from(2 - y), 70.0, 3.0);
        assert_eq!(
            read.sample(at, 1.0),
            image.read_pixel(0, x, y),
            "({x}, {y})"
        );
    }
}

#[test]
fn levels_reach_one_texel_across_tiles_and_odd_sizes() {
    // 130 × 3: three tiles across; eight levels down to one texel.
    let image = ramp(130, 3);
    let read = image.sampler(0, SampleFilter::Footprint, [Wrap::Clamp; 2]);
    assert_eq!(read.levels.len(), 9);
    assert_eq!(read.levels[8].size, [1, 1]);
    let one = read.sample([0.5, 0.5], 1e9);
    assert!(one[0] > 0.0 && one[0] < 129.0 && one[3] == 1.0, "{one:?}");
}
