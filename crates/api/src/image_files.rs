//! Image files as the host reads them: a package asks by source
//! ([`ImageSource`]: a file on disk or one embedded in the project, and how
//! its colors read) and never waits. What a reader gets is the file as the
//! installed format decoded it, held once for everyone: every layer in its
//! own samples and channels, each color layer tagged with the space of the
//! host's configuration its colors are in (the file's own tag resolved, or
//! the space the source names), data as stored. A reader takes it into its
//! own form where it uses it, a tile at a time
//! ([`Image::converted`](kinogaki_geometry::image::Image::converted) through
//! [`color::converting`](crate::color::converting)). An engine receives the
//! same image as a Texture prim, never a source.
//!
//! **The Image node.** Core's Image node ([`IMAGE_NODE`]) is the one node
//! reading a picture, in every context that carries images. Its source is a
//! file, the file embedded in the project, or another result; where networks
//! cook it gives the Image, and where they compile a compiler reads what it
//! names ([`image_source`]) and asks the host's files as it runs. A network
//! a package starts (a file's material, [`NetworkSeed`]) holds Image nodes
//! made by [`image_node::reading`].
//!
//! **Images named where no wire reaches.** A row of an object's kind that
//! reads an image (a dome's) holds a file's path, or [`OP`] and the
//! absolute path of an Image node (`op:/Library/Sky`): the host reads what
//! that node reads, follows it through renames and moves, and refuses one
//! reading a render, which renders the light reading it. A format's light
//! names an image of its file's materials item by `op:` and the image's key
//! ([`Materials::images`]), which Import points at the Image node it makes.
//!
//! [`NetworkSeed`]: crate::network::NetworkSeed
//! [`Materials::images`]: crate::standard::Materials::images
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
    /// `source` (a path relative to the project's folder unless absolute):
    /// the file as it holds it (see the module docs).
    fn image(&self, source: &ImageSource) -> FileImage;

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

    fn revision(&self) -> u64 {
        0
    }
}

/// Core's Image node: its source, then its rows ([`image_node`]).
pub const IMAGE_NODE: NodeTypeId = NodeTypeId::declared("kinogaki.core.image");

/// What an image row holding an Image node's path starts with (`op:`,
/// Houdini's spelling): see the module docs.
pub const OP: &str = "op:";

/// The rows of core's Image node ([`IMAGE_NODE`]), by name, its output, and
/// a node reading a source ([`reading`](image_node::reading)).
pub mod image_node {
    use crate::network::NewNode;
    use kinogaki_geometry::image::{ImageFile, ImageSource};
    use kinogaki_value::Value;
    use std::sync::Arc;

    /// Its one output: the Image.
    pub const IMAGE: &str = "image";
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
    /// How its colors read (a [`ColorSpace`](kinogaki_value::ValueKind::ColorSpace)
    /// choice): an [`ImageColor`](kinogaki_geometry::image::ImageColor)'s
    /// [`key`](kinogaki_geometry::image::ImageColor::key), `auto`,
    /// `data` or the name of the space they are in.
    pub const COLOR: &str = "color";
    /// Its button holding the file it reads embedded (the host reads it).
    pub const EMBED: &str = "embed";
    /// Its button writing the file it holds out (the host writes it), every
    /// Image node holding the same bytes then reading it.
    pub const EXTRACT: &str = "extract";

    /// An Image node reading `source`, to start a network with (a file's
    /// material's texture): its Source File with the path, or Embedded with
    /// the file, and its Color; the rest at its defaults.
    pub fn reading(source: &ImageSource) -> NewNode {
        let choice = |key: &str| Value::Choice(Arc::from(key));
        let node = match source.file() {
            ImageFile::Path(path) => NewNode::new(super::IMAGE_NODE)
                .with(SOURCE, choice(FILE))
                .with(FILE, path.to_string()),
            ImageFile::Embedded(file) => NewNode::new(super::IMAGE_NODE)
                .with(SOURCE, choice(EMBEDDED))
                .with(
                    EMBEDDED,
                    Value::Data(super::embedded_file().wrap(file.clone())),
                ),
        };
        node.with(COLOR, choice(source.color().key()))
    }
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
    Ok(Some(ImageSource::new(
        file,
        ImageColor::of_key(choice(COLOR)),
    )))
}

#[cfg(test)]
mod tests {
    use super::*;
    use kinogaki_geometry::EmbeddedFile;

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
            (COLOR, choice("ACEScg")),
        ]);
        let read = image_source(&file).unwrap().unwrap();
        assert_eq!(read.name(), "t/wood.png");
        assert_eq!(read.color(), &ImageColor::space("ACEScg").unwrap());
        let bytes = EmbeddedFile::new("wood.png", vec![1, 2, 3]);
        let embedded = rows(&[
            (SOURCE, choice(EMBEDDED)),
            (EMBEDDED, Value::Data(embedded_file().wrap(bytes.clone()))),
            (COLOR, choice(ImageColor::Data.key())),
        ]);
        let read = image_source(&embedded).unwrap().unwrap();
        assert_eq!(read.file(), &ImageFile::Embedded(bytes));
        assert_eq!(read.color(), &ImageColor::Data);
        let result = rows(&[(SOURCE, choice(RESULT))]);
        assert!(image_source(&result).unwrap().is_none());
        // A node reading a source reads it back.
        let space = ImageColor::space("ACEScg").unwrap();
        for source in [
            read,
            ImageSource::new(ImageFile::Path(Arc::from("/a.exr")), space),
            ImageSource::path("b.png"),
        ] {
            let made = reading(&source);
            assert_eq!(made.node_type, IMAGE_NODE);
            assert_eq!(image_source(&made.parameters).unwrap(), Some(source));
        }
        let blank = rows(&[(SOURCE, choice(FILE)), (FILE, Value::Text(" ".into()))]);
        assert!(image_source(&blank).is_err());
        assert!(image_source(&rows(&[(SOURCE, choice(EMBEDDED))])).is_err());
    }
}
