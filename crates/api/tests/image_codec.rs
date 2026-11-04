//! How images save: byte-exact, sparse layers staying sparse, every sample and
//! encoding, strict about what it reads.
use kinogaki_studio_api::DataCodec;
use kinogaki_studio_api::element::{Field, MemoryElement};
use kinogaki_studio_api::geometry::image::{Encoding, Format, Image, Rect, Sample, Tile};
use kinogaki_studio_api::geometry::{Attribute, Attributes, Domain, Meaning};
use kinogaki_studio_api::standard::{self, ImageCodec};
use std::sync::Arc;

fn round_trip(image: &Image) -> Image {
    let data = standard::image().wrap(image.clone());
    let mut saved = MemoryElement::new("result");
    saved.write(|w| ImageCodec.write(&data, None, w)).unwrap();
    let read = saved.read(|r| ImageCodec.read(r, None)).unwrap();
    standard::image().get(&read).unwrap().clone()
}

#[test]
fn an_image_round_trips_byte_exact_and_sparse_stays_sparse() {
    let window = Rect::new([-2, -3], [130, 70]).unwrap();
    let detail = Attributes::new()
        .with(
            Domain::Detail,
            "fps",
            Attribute::tuples(vec![24.0f32], Meaning::None).unwrap(),
        )
        .unwrap();
    let main = Format::rgba(Sample::F16, Encoding::color("ACEScg", true));
    let depth = Format {
        name: "Z".into(),
        channels: Arc::new([Arc::from("Z")]),
        sample: Sample::F32,
        encoding: Encoding::Raw,
    };
    let tile = Tile::new((0..main.tile_bytes()).map(|b| b as u8).collect());
    let z = Tile::new(vec![7; depth.tile_bytes()]);
    let image = Image::builder(window)
        .data(Rect::new([0, 0], [65, 65]).unwrap())
        .aspect(1.5)
        .attributes(detail)
        .tiles(main, vec![None, Some(tile), None, None])
        .tiles(depth, vec![Some(z), None, None, None])
        .build()
        .unwrap();
    let read = round_trip(&image);
    assert_eq!(read, image);
    assert!(read.layers()[0].tiles()[0].is_none(), "sparse stays sparse");
    assert_eq!(read.pixel_bytes(), image.pixel_bytes());
}

#[test]
fn reading_is_strict() {
    let write = |pixels: Vec<u8>, sample: &'static str| {
        let mut saved = MemoryElement::new("result");
        saved
            .write(|w| {
                w.set("display", Field::I32s(vec![0, 0, 1, 1]))?;
                w.set("data", Field::I32s(vec![0, 0, 1, 1]))?;
                w.set("aspect", Field::Float(1.0))?;
                let mut layer = w.child("layer0", "layer")?;
                layer.set("name", Field::Text(String::new()))?;
                layer.set("channels", Field::Texts(vec!["Y".into()]))?;
                layer.set("sample", Field::Text(sample.into()))?;
                layer.set("encoding", Field::Text("raw".into()))?;
                layer.set("tiles", Field::U32s(vec![0]))?;
                layer.set("pixels", Field::U8s(pixels))
            })
            .unwrap();
        saved.read(|r| ImageCodec.read(r, None))
    };
    assert!(write(vec![1; 4096], "u8").is_ok());
    assert!(write(vec![1; 3], "u8").is_err(), "not a tile's bytes");
    assert!(write(vec![1; 4096], "u9").is_err(), "no such sample");
}
