//! [`ImageCodec`]: how [`Image`] values save.
use super::{attributes, image};
use crate::Error;
use crate::codec::DataCodec;
use crate::data::Data;
use crate::element::{ElementReader, ElementWriter, Field};
use kinogaki_geometry::image::{
    Encoding, Format, Image, ImageBuilder, Rect, Sample, Tile, tiles_of,
};
use std::sync::Arc;

/// Saves [`Image`]: the fields `display` and `data` (each `I32s` min x, min
/// y, max x, max y) and `aspect`, the metadata's attributes as children, and
/// a child `layer<k>` (kind `layer`) for each layer: `name`, `channels`,
/// `sample`, `encoding` (`color` with `space` and `premultiplied`, or
/// `raw`), `tiles` (the places of the tiles it stores, ascending) and
/// `pixels` (their bytes, in order), so sparse layers stay sparse. A round
/// trip is byte-exact.
///
/// ```
/// use kinogaki_studio_api::DataCodec;
/// use kinogaki_studio_api::element::MemoryElement;
/// use kinogaki_studio_api::geometry::image::{Encoding, Format, Image, Rect, Sample};
/// use kinogaki_studio_api::standard::{self, ImageCodec};
///
/// let format = Format::rgba(Sample::U8, Encoding::color("sRGB", false));
/// let picture = Image::builder(Rect::new([0, 0], [2, 1]).unwrap())
///     .plane(format, &[255, 0, 0, 255, 0, 255, 0, 255])
///     .build()
///     .unwrap();
/// let data = standard::image().wrap(picture.clone());
/// let mut saved = MemoryElement::new("result");
/// saved.write(|w| ImageCodec.write(&data, None, w)).unwrap();
/// let read = saved.read(|r| ImageCodec.read(r, None)).unwrap();
/// assert_eq!(standard::image().get(&read), Some(&picture));
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct ImageCodec;

/// Why the element `from` can't be read.
fn invalid(from: &dyn ElementReader, why: impl std::fmt::Display) -> Error {
    Error::invalid(format_args!("{}: {why}", from.location()))
}

fn window(rect: Rect) -> Field {
    let ([x0, y0], [x1, y1]) = (rect.min(), rect.max());
    Field::I32s(vec![x0, y0, x1, y1])
}

fn read_window(from: &dyn ElementReader, name: &str) -> Result<Rect, Error> {
    let bad = || invalid(from, format_args!("{name} isn't a window"));
    let values = from.required(name)?;
    let crate::element::FieldRef::I32s(&[x0, y0, x1, y1]) = values else {
        return Err(bad());
    };
    Rect::new([x0, y0], [x1, y1]).map_err(|e| invalid(from, e))
}

impl DataCodec for ImageCodec {
    fn write(
        &self,
        data: &Data,
        _base: Option<&Data>,
        out: &mut dyn ElementWriter,
    ) -> Result<(), Error> {
        let value = image()
            .get(data)
            .ok_or_else(|| Error::invalid("not an image"))?;
        out.set("display", window(value.display()))?;
        out.set("data", window(value.data()))?;
        out.set("aspect", Field::Float(value.aspect()))?;
        attributes::write(value.attributes(), out)?;
        for (k, layer) in value.layers().iter().enumerate() {
            let mut child = out.child(&format!("layer{k}"), "layer")?;
            let format = layer.format();
            child.set("name", Field::Text(format.name.to_string()))?;
            let channels = format.channels.iter().map(|c| c.to_string()).collect();
            child.set("channels", Field::Texts(channels))?;
            child.set("sample", Field::Text(format.sample.name().into()))?;
            match &format.encoding {
                Encoding::Color {
                    space,
                    premultiplied,
                } => {
                    child.set("encoding", Field::Text("color".into()))?;
                    child.set("space", Field::Text(space.to_string()))?;
                    child.set("premultiplied", Field::Bool(*premultiplied))?;
                }
                _ => child.set("encoding", Field::Text("raw".into()))?,
            }
            let stored = layer.tiles().iter().enumerate();
            let (places, tiles): (Vec<u32>, Vec<&Tile>) = stored
                .filter_map(|(at, t)| Some((at as u32, t.as_ref()?)))
                .unzip();
            child.set("tiles", Field::U32s(places))?;
            let pixels = tiles.iter().flat_map(|t| t.bytes()).copied().collect();
            child.set("pixels", Field::U8s(pixels))?;
        }
        Ok(())
    }

    fn read(&self, from: &dyn ElementReader, _base: Option<&Data>) -> Result<Data, Error> {
        let data = read_window(from, "data")?;
        let mut builder: ImageBuilder = Image::builder(read_window(from, "display")?)
            .data(data)
            .aspect(from.float("aspect")?)
            .attributes(attributes::read(from)?);
        let across = tiles_of(data.size());
        let count = across[0] as usize * across[1] as usize;
        let layers = from.children();
        let layers = layers.iter().filter(|c| c.starts_with("layer"));
        for name in layers {
            let child = from.required_child(name)?;
            let child = &*child;
            let format = read_format(child)?;
            let places = child.u32s("tiles")?;
            let pixels = attributes::u8s(child, "pixels")?;
            let size = format.tile_bytes();
            if pixels.len() != places.len() * size {
                return Err(invalid(child, "its pixels aren't its tiles'"));
            }
            let mut tiles = vec![None; count];
            let mut last = None;
            for (place, bytes) in places.iter().zip(pixels.chunks_exact(size.max(1))) {
                let at = *place as usize;
                if at >= count || last.is_some_and(|l| l >= at) {
                    return Err(invalid(child, format_args!("a tile at {place}")));
                }
                last = Some(at);
                tiles[at] = Some(Tile::new(bytes.to_vec()));
            }
            builder = builder.tiles(format, tiles);
        }
        let value = builder.build().map_err(|e| invalid(from, e))?;
        Ok(image().wrap(value))
    }
}

fn read_format(child: &dyn ElementReader) -> Result<Format, Error> {
    let sample = child.text("sample")?;
    let sample = Sample::named(sample)
        .ok_or_else(|| invalid(child, format_args!("unknown sample {sample:?}")))?;
    let encoding = match child.text("encoding")? {
        "raw" => Encoding::Raw,
        "color" => Encoding::color(child.text("space")?, child.bool("premultiplied")?),
        other => return Err(invalid(child, format_args!("unknown encoding {other:?}"))),
    };
    let channels: Arc<[Arc<str>]> = (child.texts("channels")?.iter())
        .map(|c| Arc::from(c.as_str()))
        .collect();
    if !(1..=4).contains(&channels.len()) {
        return Err(invalid(child, "a layer holds 1 to 4 channels"));
    }
    Ok(Format {
        name: child.text("name")?.into(),
        channels,
        sample,
        encoding,
    })
}
