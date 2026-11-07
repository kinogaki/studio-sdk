//! [`EmbeddedFileCodec`]: how an [`EmbeddedFile`] saves.
use super::{attributes, embedded_file};
use crate::Error;
use crate::codec::DataCodec;
use crate::data::Data;
use crate::element::{ElementReader, ElementWriter, Field};
use kinogaki_geometry::image::EmbeddedFile;

/// Saves an [`EmbeddedFile`]: its `name`, and a child `bytes` holding its
/// bytes as a field `bytes`, written once per save for every file holding the
/// same bytes ([`ElementWriter::share`], keyed by their hash) whatever its
/// name. Reading hashes the bytes again, once per load. A round trip is
/// byte-exact.
///
/// ```
/// use kinogaki_studio_api::DataCodec;
/// use kinogaki_studio_api::element::MemoryElement;
/// use kinogaki_studio_api::geometry::image::EmbeddedFile;
/// use kinogaki_studio_api::standard::{self, EmbeddedFileCodec};
///
/// let file = EmbeddedFile::new("wood.png", vec![0x89, b'P', b'N', b'G']);
/// let data = standard::embedded_file().wrap(file.clone());
/// let mut saved = MemoryElement::new("embedded");
/// saved.write(|w| EmbeddedFileCodec.write(&data, None, w)).unwrap();
/// let read = saved.read(|r| EmbeddedFileCodec.read(r, None)).unwrap();
/// assert_eq!(standard::embedded_file().get(&read), Some(&file));
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct EmbeddedFileCodec;

impl DataCodec for EmbeddedFileCodec {
    fn write(
        &self,
        data: &Data,
        _base: Option<&Data>,
        out: &mut dyn ElementWriter,
    ) -> Result<(), Error> {
        let file =
            (embedded_file().get(data)).ok_or_else(|| Error::invalid("not an embedded file"))?;
        out.set("name", Field::Text(file.name().to_owned()))?;
        let mut key = [0; 8];
        key.copy_from_slice(&file.hash()[..8]);
        out.share("bytes", "bytes", u64::from_le_bytes(key), &mut |w| {
            w.set("bytes", Field::U8s(file.bytes().to_vec()))
        })
    }

    fn read(&self, from: &dyn ElementReader, _base: Option<&Data>) -> Result<Data, Error> {
        let name = from.text("name")?;
        let bytes = from.read_shared("bytes", |b| {
            Ok(EmbeddedFile::new("", attributes::u8s(b, "bytes")?.to_vec()))
        })?;
        Ok(embedded_file().wrap(bytes.named(name)))
    }
}
