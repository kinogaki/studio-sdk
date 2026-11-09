//! A format's test support: a folder of a test's own ([`Scratch`]), a
//! write read back as Import reads it ([`round_trip`]), and the check that
//! File ▸ Open reads a file as Import does, in the file's own units
//! ([`document`]).
use crate::Unconverted;
use kinogaki_studio_api::codec::DataCodec;
use kinogaki_studio_api::data::Data;
use kinogaki_studio_api::element::MemoryElement;
use kinogaki_studio_api::geometry::{Units, UpAxis};
use kinogaki_studio_api::id::DataTypeId;
use kinogaki_studio_api::render::lights::LightCodec;
use kinogaki_studio_api::standard::{
    self, BrepCodec, CameraCodec, EmbeddedFileCodec, GeometryCodec, ImageCodec, InstancesCodec,
    MaterialsCodec,
};
use kinogaki_studio_api::{
    Diagnostic, Error, FileFormat, FileItem, FormatOptions, Imported, Input, Purpose, Reading,
    Writing,
};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

/// A new, empty folder of a test's own in the system's temporary folder,
/// by its canonical path (the temporary folder may be behind a link, which
/// a relative path a written file names can't climb back through);
/// removed, with what was written in it, when dropped. No two scratches of
/// a run share a folder, nor do two runs. It is the folder's [`Path`].
///
/// ```
/// use kinogaki_studio_api_testing::format::Scratch;
/// let scratch = Scratch::new("doc");
/// std::fs::write(scratch.join("a.txt"), "a").unwrap();
/// let folder = scratch.to_owned();
/// drop(scratch);
/// assert!(!folder.exists());
/// ```
#[derive(Debug)]
pub struct Scratch(PathBuf);

impl Scratch {
    /// A folder whose name holds `name`, the test's.
    pub fn new(name: &str) -> Scratch {
        static MADE: AtomicU32 = AtomicU32::new(0);
        let made = MADE.fetch_add(1, Ordering::Relaxed);
        let id = std::process::id();
        let dir = std::env::temp_dir().join(format!("kinogaki-{name}-{id}-{made}"));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a scratch folder");
        Scratch(dir.canonicalize().expect("a scratch folder"))
    }
}

impl std::ops::Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

/// `items` written to `path` by `format`'s writer with its default
/// options and `metadata` (what a read of the file the items came from
/// gave, [`Imported::metadata`]; `None` for items of no such file), and
/// the file read back as Import reads it with the format's default
/// options, in a project of `units` whose colors convert to nothing
/// ([`Unconverted`]): what was read (its warnings and metadata the read's),
/// and the writer's warnings.
///
/// # Errors
/// A format that writes nothing, a failed write, or a failed read.
pub fn round_trip(
    format: &dyn FileFormat,
    (items, metadata): (&[FileItem], Option<&FormatOptions>),
    path: &Path,
    units: Units,
) -> Result<(Imported, Vec<Diagnostic>), Error> {
    let label = format.label();
    let writer =
        (format.writer()).ok_or_else(|| Error::invalid(format!("{label} writes nothing")))?;
    let writing = Writing {
        metadata,
        ..Writing::new(units, &Unconverted)
    };
    let said = writer.write(path, items, &writing, &writer.options())?;
    let reading = Reading::new(Purpose::Objects, units, &Unconverted);
    let read = format.read(&Input::file(path), &format.options(), &reading)?;
    Ok((read, said))
}

/// Checks `format`'s [`Purpose::Document`] read of `input` with `options`
/// against its [`Purpose::Objects`] read: read in a project of units no
/// file states (feet, Z up), the document's lengths are in the units it
/// reports ([`Imported::units`], else the project's), and its items equal
/// what Import reads into those same units, each value as its codec saves
/// it. Returns those units.
///
/// # Errors
/// A read that fails, or what differs.
pub fn document(
    format: &dyn FileFormat,
    input: &Input<'_>,
    options: &FormatOptions,
) -> Result<Units, String> {
    let project = Units::new(0.3048, UpAxis::Z).expect("feet");
    let read = |purpose, units| {
        let reading = Reading::new(purpose, units, &Unconverted);
        (format.read(input, options, &reading)).map_err(|e| format!("{purpose:?}: {e}"))
    };
    let opened = read(Purpose::Document, project)?;
    let units = opened.units.unwrap_or(project);
    let imported = read(Purpose::Objects, units)?;
    if imported.units.is_some() {
        return Err("an Objects read reports units".into());
    }
    if opened.items.len() != imported.items.len() {
        let counts = (opened.items.len(), imported.items.len());
        return Err(format!("{} items opened, {} imported", counts.0, counts.1));
    }
    for (at, (a, b)) in opened.items.iter().zip(&imported.items).enumerate() {
        let data = match (&a.data, &b.data) {
            (Some(a), Some(b)) => same(a, b),
            (a, b) => a.is_none() && b.is_none(),
        };
        let held = (&a.name, a.parent, a.placement, a.visible);
        if held != (&b.name, b.parent, b.placement, b.visible) || !data {
            return Err(format!("item {at} ({}) differs from Import's", a.name));
        }
    }
    Ok(units)
}

/// Whether `a` and `b` hold equal values: of one standard type, what its
/// codec saves of each is the same (the values it holds compared so too:
/// an embedded file by its bytes, an instance's prototypes); of any other
/// type, the same type.
fn same(a: &Data, b: &Data) -> bool {
    let codecs: [(DataTypeId, &dyn DataCodec); 8] = [
        (standard::geometry().id(), &GeometryCodec),
        (standard::brep().id(), &BrepCodec),
        (standard::instances().id(), &InstancesCodec),
        (standard::materials().id(), &MaterialsCodec),
        (standard::light().id(), &LightCodec),
        (standard::camera().id(), &CameraCodec),
        (standard::image().id(), &ImageCodec),
        (standard::embedded_file().id(), &EmbeddedFileCodec),
    ];
    let Some((_, codec)) = codecs.into_iter().find(|(ty, _)| *ty == a.ty()) else {
        return a.ty() == b.ty();
    };
    let saved = |data: &Data| {
        let mut element = MemoryElement::new("value");
        element
            .write(|w| codec.write(data, w))
            .ok()
            .map(|()| element)
    };
    match (saved(a), saved(b)) {
        (Some(x), Some(y)) => a.ty() == b.ty() && x.same_as(&y, &same),
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kinogaki_studio_api::format::Conversion;
    use kinogaki_studio_api::geometry::{Vec3, Xform};
    use kinogaki_studio_api::id::FormatId;
    use kinogaki_studio_api::{FormatWriter, Value};

    /// A file of one group 1 centimetre along X; `keeps` whether its
    /// Document read reports the units it keeps.
    struct Centimetre {
        keeps: bool,
    }

    impl FileFormat for Centimetre {
        fn id(&self) -> FormatId {
            FormatId::declared("acme.format.centimetre")
        }
        fn label(&self) -> &str {
            "Centimetre"
        }
        fn extensions(&self) -> &[&str] {
            &["cm"]
        }
        fn produces(&self) -> &[DataTypeId] {
            &[]
        }
        fn read(
            &self,
            _: &Input<'_>,
            _: &FormatOptions,
            reading: &Reading<'_>,
        ) -> Result<Imported, Error> {
            let file = Units::new(0.01, UpAxis::Y)?;
            let kept = reading.document_units(file);
            let space = Conversion::new(file, kept.unwrap_or(reading.units));
            let item = FileItem {
                name: "Group".into(),
                parent: None,
                placement: space.placement(&Xform::translation(Vec3::new(1.0, 0.0, 0.0))),
                visible: true,
                data: None,
            };
            Ok(Imported {
                items: vec![item],
                units: kept.filter(|_| self.keeps),
                ..Imported::default()
            })
        }
    }

    /// A file holding one note, the metadata `note`, which its writer
    /// writes back from the metadata it is handed.
    struct Note;

    impl FileFormat for Note {
        fn id(&self) -> FormatId {
            FormatId::declared("acme.format.note")
        }
        fn label(&self) -> &str {
            "Note"
        }
        fn extensions(&self) -> &[&str] {
            &["note"]
        }
        fn produces(&self) -> &[DataTypeId] {
            &[]
        }
        fn read(
            &self,
            input: &Input<'_>,
            _: &FormatOptions,
            _: &Reading<'_>,
        ) -> Result<Imported, Error> {
            let text = String::from_utf8_lossy(&input.bytes()?).into_owned();
            let metadata = FormatOptions::from([("note".to_owned(), Value::Text(text))]);
            Ok(Imported {
                metadata,
                ..Imported::default()
            })
        }
        fn writer(&self) -> Option<&dyn FormatWriter> {
            Some(self)
        }
    }

    impl FormatWriter for Note {
        fn accepts(&self) -> &[DataTypeId] {
            &[]
        }
        fn write(
            &self,
            path: &Path,
            _: &[FileItem],
            writing: &Writing<'_>,
            _: &FormatOptions,
        ) -> Result<Vec<Diagnostic>, Error> {
            let note = writing
                .metadata
                .and_then(|m| m.get("note"))
                .and_then(Value::text);
            std::fs::write(path, note.unwrap_or_default())?;
            Ok(Vec::new())
        }
    }

    #[test]
    fn round_trip_carries_metadata() {
        let scratch = Scratch::new("note");
        let path = scratch.join("a.note");
        let kept = FormatOptions::from([("note".to_owned(), Value::Text("kept".into()))]);
        let (read, _) = round_trip(&Note, (&[], Some(&kept)), &path, Units::METERS).unwrap();
        assert_eq!(read.metadata, kept);
        let (read, _) = round_trip(&Note, (&[], None), &path, Units::METERS).unwrap();
        assert_eq!(read.metadata.get("note"), Some(&Value::Text(String::new())));
    }

    #[test]
    fn document_read_is_an_objects_read_in_its_own_units() {
        let input = Input::memory("a.cm", b"");
        let options = FormatOptions::new();
        let units = document(&Centimetre { keeps: true }, &input, &options);
        assert_eq!(units, Ok(Units::new(0.01, UpAxis::Y).unwrap()));
        // Kept in the file's units but not said: the lengths aren't the
        // project's the host takes them for.
        let unsaid = document(&Centimetre { keeps: false }, &input, &options);
        assert_eq!(unsaid, Err("item 0 (Group) differs from Import's".into()));
    }
}
