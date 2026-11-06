use super::*;
use crate::{Attribute, Domain, Meaning};

fn window(w: i32, h: i32) -> Rect {
    Rect::new([0, 0], [w, h]).unwrap()
}

fn linear() -> Encoding {
    Encoding::color("ACEScg", true)
}

/// An RGBA float image whose red is `x + 1`, green `y + 1`.
fn ramp(w: i32, h: i32) -> Image {
    let mut px = Vec::new();
    for y in 0..h {
        for x in 0..w {
            px.extend([x as f32 + 1.0, y as f32 + 1.0, 0.5, 1.0]);
        }
    }
    Image::builder(window(w, h))
        .plane(
            Format::rgba(Sample::F32, linear()),
            bytemuck::cast_slice(&px),
        )
        .build()
        .unwrap()
}

#[test]
fn windows_and_refusals() {
    assert!(Rect::new([0, 0], [0, 4]).is_err(), "empty");
    assert!(Rect::new([5, 0], [4, 4]).is_err(), "inside out");
    assert!(Rect::new([0, 0], [1 << 25, 4]).is_err(), "too wide");
    let r = Rect::new([-3, 2], [7, 4]).unwrap();
    assert_eq!(r.size(), [10, 2]);
    assert!(r.contains(-3, 2) && !r.contains(7, 2) && !r.contains(0, 4));

    let build = |b: ImageBuilder| b.build().unwrap_err().to_string();
    let format = || Format::rgba(Sample::U8, linear());
    assert!(build(Image::builder(window(2, 2)).aspect(0.0)).contains("aspect"));
    assert!(build(Image::builder(window(2, 2)).plane(format(), &[0; 3])).contains("bytes"));
    assert!(build(Image::builder(window(2, 2)).tiles(format(), Vec::new())).contains("tiles"));
    let short = Tile::new(vec![0; 5]);
    let wrong = Image::builder(window(2, 2)).tiles(format(), vec![Some(short)]);
    assert!(build(wrong).contains("bytes"));
    let twice =
        (Image::builder(window(2, 2)).tiles(format(), vec![None])).tiles(format(), vec![None]);
    assert!(build(twice).contains("two layers"));
    let none = Format {
        channels: Arc::new([]),
        ..format()
    };
    assert!(build(Image::builder(window(2, 2)).tiles(none, vec![None])).contains("channels"));
    let unnamed = Format::rgba(Sample::U8, Encoding::color("", false));
    assert!(build(Image::builder(window(2, 2)).tiles(unnamed, vec![None])).contains("space"));
    let points = Attributes::new()
        .with(
            Domain::Point,
            "x",
            Attribute::tuples(vec![1.0f32], Meaning::None).unwrap(),
        )
        .unwrap();
    let outside = Image::builder(window(2, 2)).attributes(points);
    assert!(build(outside).contains("Detail"));
}

#[test]
fn a_plane_reads_back_as_it_was_built() {
    for (w, h) in [(1, 1), (64, 64), (130, 70)] {
        let image = ramp(w, h);
        let plane = image.plane(0).unwrap();
        let built = ramp(w, h);
        assert_eq!(
            Image::builder(window(w, h))
                .plane(built.layers()[0].format().clone(), &plane)
                .build()
                .unwrap(),
            built
        );
        let floats: &[f32] = bytemuck::cast_slice(&plane);
        let at = ((h as usize - 1) * w as usize + w as usize - 1) * 4;
        assert_eq!(floats[at..at + 2], [w as f32, h as f32]);
    }
    assert!(ramp(2, 2).plane(1).is_none());
}

#[test]
fn a_data_window_apart_from_the_display_window() {
    let image = Image::builder(window(4, 4))
        .data(Rect::new([-1, -1], [1, 1]).unwrap())
        .plane(
            Format::rgba(Sample::F32, linear()),
            bytemuck::cast_slice(&[1.0f32; 16]),
        )
        .build()
        .unwrap();
    assert_eq!(image.read_pixel(0, -1, -1), [1.0; 4]);
    assert_eq!(
        image.read_pixel(0, 1, 1),
        [0.0; 4],
        "outside the data window"
    );
    assert_eq!(image.display().size(), [4, 4]);
}

#[test]
fn with_tiles_shares_what_it_did_not_touch() {
    let image = ramp(200, 100);
    let tile = image.tile(0, [1, 0]).unwrap().clone();
    let new = Tile::new(vec![0; tile.len()]);
    let edited = image.with_tiles(0, [([1, 0], Some(new))]).unwrap();
    for at in [[0, 0], [2, 0], [0, 1], [3, 1]] {
        let (a, b) = (image.tile(0, at).unwrap(), edited.tile(0, at).unwrap());
        assert!(a.ptr_eq(b), "{at:?} is shared");
    }
    assert!(!edited.tile(0, [1, 0]).unwrap().ptr_eq(&tile));
    assert_eq!(edited.read_pixel(0, 70, 1), [0.0; 4]);
    assert_eq!(
        image.read_pixel(0, 70, 1),
        [71.0, 2.0, 0.5, 1.0],
        "the old stays"
    );
    assert!(image.with_tiles(0, [([9, 9], None)]).is_err());
    assert!(image.with_tiles(3, [([0, 0], None)]).is_err());
    let short = Tile::new(vec![0; 2]);
    assert!(image.with_tiles(0, [([0, 0], Some(short))]).is_err());
}

#[test]
fn pixel_decodes_every_sample_and_fills_missing_channels() {
    let at = |sample: Sample, bytes: &[u8], channels: &[&str]| {
        let format = Format {
            name: "n".into(),
            channels: channels.iter().map(|c| Arc::from(*c)).collect(),
            sample,
            encoding: Encoding::Raw,
        };
        let image = Image::builder(window(1, 1))
            .plane(format, bytes)
            .build()
            .unwrap();
        image.read_pixel(0, 0, 0)
    };
    assert_eq!(at(Sample::U8, &[255], &["Y"]), [1.0, 0.0, 0.0, 1.0]);
    let half = 32768u16.to_ne_bytes();
    let u16s = at(Sample::U16, &half, &["Y"]);
    assert!((u16s[0] - 0.5).abs() < 1e-4);
    let f16 = f32_to_half(0.25).to_ne_bytes();
    assert_eq!(at(Sample::F16, &[f16[0], f16[1]], &["Z"])[0], 0.25);
    let f = 2.5f32.to_ne_bytes();
    let two = [f[0], f[1], f[2], f[3], f[0], f[1], f[2], f[3]];
    assert_eq!(at(Sample::F32, &two, &["U", "V"]), [2.5, 2.5, 0.0, 1.0]);
    let id = 4_000_000u32.to_ne_bytes();
    assert_eq!(
        at(Sample::U32, &id, &["id"])[0],
        4_000_000.0,
        "an id's number"
    );
}

#[test]
fn a_whole_number_writes_rounded_and_saturated() {
    let mut bytes = [0; 4];
    for (value, written) in [(6.6f32, 7u32), (-3.0, 0), (5e9, u32::MAX)] {
        Sample::U32.write(value, &mut bytes);
        assert_eq!(u32::from_ne_bytes(bytes), written);
    }
    assert_eq!(Sample::named("u32"), Some(Sample::U32));
}

#[test]
fn color_tags_name_their_space_and_compare_by_bits() {
    let space = Encoding::color("sRGB", false);
    assert_eq!(space.space(), Some("sRGB"));
    assert_eq!(Encoding::Raw.space(), None);
    let xy = [0.64, 0.33, 0.3, 0.6, 0.15, 0.06, 0.3127, 0.329];
    let primaries = ColorTag::Primaries(xy);
    assert_eq!(primaries.space(), None);
    assert_eq!(primaries, ColorTag::Primaries(xy));
    assert_ne!(ColorTag::Primaries([f32::NAN; 8]), primaries);
    assert_eq!(
        ColorTag::Primaries([f32::NAN; 8]),
        ColorTag::Primaries([f32::NAN; 8])
    );
    assert_eq!(primaries.to_string(), "primaries");
    assert_eq!(ColorTag::Space("ACEScg".into()).to_string(), "ACEScg");
}

#[test]
fn half_floats_round_trip() {
    for v in [
        0.0f32,
        1.0,
        -1.0,
        0.5,
        65504.0,
        6.1035156e-5,
        5.9604645e-8,
        1e-3,
    ] {
        assert_eq!(
            half_to_f32(f32_to_half(v)),
            half_to_f32(f32_to_half(half_to_f32(f32_to_half(v))))
        );
        assert!(
            (half_to_f32(f32_to_half(v)) - v).abs() <= v.abs() * 1e-3 + 1e-7,
            "{v}"
        );
    }
    assert_eq!(f32_to_half(1e9), 0x7c00, "too big is infinite");
    assert!(half_to_f32(f32_to_half(f32::NAN)).is_nan());
}

#[test]
fn zero_tiles_stay_absent() {
    let image = Image::builder(window(130, 10))
        .plane(Format::rgba(Sample::U8, linear()), &vec![0; 4 * 130 * 10])
        .build()
        .unwrap();
    assert!(image.layers()[0].tiles().iter().all(Option::is_none));
    assert_eq!(image.pixel_bytes(), 0);
}

#[test]
fn a_pyramid_means_tiles_and_derives_only_what_an_edit_touched() {
    let image = ramp(128, 64);
    let format = image.layers()[0].format().clone();
    let mut pyramid = Pyramid::new(format, image.data().size());
    assert_eq!(Pyramid::levels([128, 64]), 2);
    assert_eq!(Pyramid::levels([64, 64]), 1);
    assert_eq!(Pyramid::levels([65, 1]), 2);
    let leaf = |image: &Image| {
        let image = image.clone();
        move |at| image.tile(0, at).cloned()
    };
    let top = pyramid.tile(1, [0, 0], &mut leaf(&image)).unwrap();
    let read =
        |t: &Tile, x: usize, y: usize| decode(Sample::F32, 4, &t.bytes()[(y * 64 + x) * 16..]);
    // Pixel (0, 0) means reds 1, 2, 1, 2 and greens 1, 1, 2, 2.
    assert_eq!(read(&top, 0, 0), [1.5, 1.5, 0.5, 1.0]);
    assert_eq!(read(&top, 63, 31), [127.5, 63.5, 0.5, 1.0]);
    assert_eq!(read(&top, 0, 32), [0.0; 4], "past the level");
    // The same tiles: the very tile kept.
    let again = pyramid.tile(1, [0, 0], &mut leaf(&image)).unwrap();
    assert!(again.ptr_eq(&top));
    // A tile under it edited: made again; a level over nothing has nothing.
    let blank = image.with_tiles(0, [([1, 0], None)]).unwrap();
    let edited = pyramid.tile(1, [0, 0], &mut leaf(&blank)).unwrap();
    assert!(!edited.ptr_eq(&top));
    assert_eq!(read(&edited, 40, 0), [0.0; 4]);
    assert_eq!(read(&edited, 0, 0), [1.5, 1.5, 0.5, 1.0]);
    let none = blank.with_tiles(0, [([0, 0], None)]).unwrap();
    assert!(pyramid.tile(1, [0, 0], &mut leaf(&none)).is_none());
}

#[test]
fn edges_of_a_level_are_not_darkened_by_padding() {
    let image = Image::builder(window(3, 1))
        .plane(
            Format::rgba(Sample::F32, linear()),
            bytemuck::cast_slice(&[1.0f32; 12]),
        )
        .build()
        .unwrap();
    let mut pyramid = Pyramid::new(image.layers()[0].format().clone(), [3, 1]);
    let source = image.clone();
    let tile = pyramid.tile(1, [0, 0], &mut |at| source.tile(0, at).cloned());
    let tile = tile.unwrap();
    assert_eq!(decode(Sample::F32, 4, &tile.bytes()[16..]), [1.0; 4]);
}

#[test]
fn metadata_lives_in_detail() {
    let mut image = ramp(2, 2);
    let attribute = Attribute::tuples(vec![24.0f32], Meaning::None).unwrap();
    let detail = Attributes::new()
        .with(Domain::Detail, "fps", attribute)
        .unwrap();
    image = image.with_attributes(detail).unwrap();
    assert!(image.attributes().get(Domain::Detail, "fps").is_some());
}

/// Pixels a test holds: known by a number.
#[derive(Debug)]
struct Held(u64);

impl HeldPixels for Held {
    fn key(&self) -> u64 {
        self.0
    }
}

#[test]
fn a_held_layer_has_no_tiles_and_compares_by_its_key() {
    let format = || Format::rgba(Sample::F16, linear());
    let held = |key| {
        Image::builder(window(70, 3))
            .held(format(), Arc::new(Held(key)))
            .build()
            .unwrap()
    };
    let image = held(7);
    assert!(ramp(2, 2).layers()[0].held().is_none());
    let layer = &image.layers()[0];
    assert!(layer.tiles().is_empty() && layer.held().is_some_and(|h| h.key() == 7));
    assert_eq!(image.plane(0), None, "no pixels to give");
    assert_eq!(image.read_pixel(0, 1, 1), [0.0; 4]);
    assert_eq!(image.pixel_bytes(), 0);
    assert!(image.with_tiles(0, [([0, 0], None)]).is_err());
    assert_eq!(image, held(7));
    assert_ne!(image, held(8));
}
