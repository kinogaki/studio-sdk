//! Image files as the host reads them: a package asks by source
//! ([`ImageSource`]: a file on disk or one embedded in the project, and how
//! its colors read) and never waits. What a reader gets are standard
//! [`Image`]s of half floats, premultiplied: the host holds the formats and
//! the color maths (a file's own tag, its file rules, its working space).
//! [`image`](ImageFiles::image) is linear ACEScg, for a scene-referred
//! network; [`picture`](ImageFiles::picture) is what the file holds, for a
//! page. An engine receives the same answers as Texture prims, never a
//! source: one decoded shape of a file for every package.
//!
//! **The Image node.** Core's Image node ([`IMAGE_NODE`]) is the one node
//! reading a picture, in every context that carries images. Its source is a
//! file, the file embedded in the project, or another result; where networks
//! cook it gives the Image, and where they compile a compiler reads what it
//! names ([`image_source`]) and asks the host's files as it runs.
use crate::standard::embedded_file;
use kinogaki_geometry::Error;
use kinogaki_geometry::image::{Image, ImageColor, ImageFile, ImageSource};
use kinogaki_value::{NodeTypeId, Parameters, Value};
use std::sync::Arc;

/// An image file as far as it is read.
#[derive(Clone, Debug, PartialEq)]
pub enum FileImage {
    /// It is on its way: the window wakes when it arrives.
    Reading,
    /// Here it is.
    Read(Image),
    /// Why it can't be read.
    Failed(Error),
}

/// The host's image files. A UI's or a runner's never wait; a cook's
/// ([`Project::files`](crate::cook::Project::files)) waits on its worker and
/// answers [`Read`](FileImage::Read) or [`Failed`](FileImage::Failed). Asked
/// again, a source that hasn't changed answers with the same tiles, so what
/// shows it uploads nothing.
pub trait ImageFiles: Send + Sync {
    /// `source` (a path relative to the project's folder unless absolute),
    /// its colors in linear ACEScg (data as stored).
    fn image(&self, source: &ImageSource) -> FileImage;

    /// `source` to place on a page: a display-referred file (a PNG, a JPEG)
    /// keeps its code values, in the space of the display its tag says; a
    /// scene-referred one (a linear OpenEXR) is linear ACEScg, as
    /// [`image`](Self::image) has it. Every layer the file holds, each
    /// color one so.
    fn picture(&self, source: &ImageSource) -> FileImage;

    /// Moves whenever a source's answer does.
    fn revision(&self) -> u64;
}

/// No image files: where nothing can read them (a project read outside the
/// editor).
#[derive(Clone, Copy, Debug, Default)]
pub struct NoFiles;

impl ImageFiles for NoFiles {
    fn image(&self, _: &ImageSource) -> FileImage {
        FileImage::Failed(Error::missing("nothing reads image files here"))
    }

    fn picture(&self, source: &ImageSource) -> FileImage {
        self.image(source)
    }

    fn revision(&self) -> u64 {
        0
    }
}

/// Core's Image node: its source, then its rows ([`image_node`]).
pub const IMAGE_NODE: NodeTypeId = NodeTypeId::declared("kinogaki.core.image");

/// The rows of core's Image node ([`IMAGE_NODE`]), by name.
pub mod image_node {
    /// What it reads (a choice): [`FILE`], [`EMBEDDED`] or [`RESULT`].
    pub const SOURCE: &str = "source";
    /// The file's path, as typed; what a File source reads.
    pub const FILE: &str = "file";
    /// The file's bytes ([`embedded_file`](crate::standard::embedded_file)
    /// data; a file of no name: none): what an Embedded source reads.
    pub const EMBEDDED: &str = "embedded";
    /// The result it shows (a path fitting
    /// [`Fits::Written`](crate::params::Fits::Written)): what a Result
    /// source reads.
    pub const RESULT: &str = "result";
    /// How its colors read (a choice): [`AUTO`], [`DATA`] or [`SPACE`].
    pub const COLOR: &str = "color";
    /// The space its colors are in while its Color is [`SPACE`].
    pub const SPACE: &str = "space";
    /// A Color: as the file says.
    pub const AUTO: &str = "auto";
    /// A Color: the values as stored.
    pub const DATA: &str = "data";
}

/// What an Image node holding `parameters` reads: its file or its embedded
/// file, its colors as its Color says; `None` for a Result source, which the
/// host resolves.
///
/// # Errors
/// No file chosen, or none embedded.
pub fn image_source(parameters: &Parameters) -> Result<Option<ImageSource>, Error> {
    use image_node::*;
    let choice = |name: &str| parameters.get(name).and_then(Value::choice).unwrap_or("");
    let text = |name: &str| parameters.get(name).and_then(Value::text).unwrap_or("");
    let file = match choice(SOURCE) {
        RESULT => return Ok(None),
        EMBEDDED => {
            let held = (parameters.get(EMBEDDED))
                .and_then(|v| match v {
                    Value::Data(d) => embedded_file().get(d),
                    _ => None,
                })
                .filter(|f| !f.name().is_empty());
            let held = held.ok_or_else(|| Error::invalid("no file is embedded"))?;
            ImageFile::Embedded(held.clone())
        }
        _ => match text(FILE).trim() {
            "" => return Err(Error::invalid("choose a file to read")),
            path => ImageFile::Path(Arc::from(path)),
        },
    };
    let space = text(SPACE).trim();
    let color = match choice(COLOR) {
        DATA => ImageColor::Data,
        SPACE if !space.is_empty() => ImageColor::Space(Arc::from(space)),
        _ => ImageColor::Auto,
    };
    Ok(Some(ImageSource::new(file, color)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kinogaki_geometry::image::EmbeddedFile;

    fn rows(rows: &[(&str, Value)]) -> Parameters {
        (rows.iter())
            .map(|(name, value)| ((*name).to_owned(), value.clone()))
            .collect()
    }

    #[test]
    fn an_image_node_reads_its_file_its_embedded_bytes_or_a_result() {
        use image_node::*;
        let choice = |key: &str| Value::Choice(Arc::from(key));
        let file = rows(&[
            (SOURCE, choice(FILE)),
            (FILE, Value::Text(" t/wood.png ".into())),
            (COLOR, choice(SPACE)),
            (SPACE, Value::Text("ACEScg".into())),
        ]);
        let read = image_source(&file).unwrap().unwrap();
        assert_eq!(read.name(), "t/wood.png");
        assert_eq!(read.color(), &ImageColor::Space(Arc::from("ACEScg")));
        let bytes = EmbeddedFile::new("wood.png", vec![1, 2, 3]);
        let embedded = rows(&[
            (SOURCE, choice(EMBEDDED)),
            (EMBEDDED, Value::Data(embedded_file().wrap(bytes.clone()))),
            (COLOR, choice(DATA)),
        ]);
        let read = image_source(&embedded).unwrap().unwrap();
        assert_eq!(read.file(), &ImageFile::Embedded(bytes));
        assert_eq!(read.color(), &ImageColor::Data);
        let result = rows(&[(SOURCE, choice(RESULT))]);
        assert!(image_source(&result).unwrap().is_none());
        let blank = rows(&[(SOURCE, choice(FILE)), (FILE, Value::Text(" ".into()))]);
        assert!(image_source(&blank).is_err());
        assert!(image_source(&rows(&[(SOURCE, choice(EMBEDDED))])).is_err());
    }
}
