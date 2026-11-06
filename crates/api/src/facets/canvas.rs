//! [`Canvas`]: what the 2D canvas pane paints of a value, a list of pieces
//! in canvas units, back to front: images and text under a placement.
//! Pieces share their tiles with the value, never copying them; core uploads
//! and derives the levels it shows, sets text, composites in linear light and
//! applies the view transform. A piece whose image's layer is [`Live`] shows a
//! result live: core draws the result's current image in its place.
//!
//! **Arranging.** A piece that says which node placed it ([`PieceSource`]) can
//! be selected and arranged on the canvas with no package code: clicks and
//! boxes pick its node, and core's gizmo writes the parameters of the node
//! type's declared [`HandleKind::Translate`](crate::cook::HandleKind::Translate),
//! [`Rotate`](crate::cook::HandleKind::Rotate) (the Z of `angles`, degrees,
//! clockwise as Y runs down) and [`Scale`](crate::cook::HandleKind::Scale)
//! (the X and Y of `factors`) handles.
use crate::Error;
use crate::color::ColorService;
use crate::data::Data;
use crate::data_type::InfoRow;
use crate::refs::NodeRef;
use kinogaki_geometry::Xform;
use kinogaki_geometry::image::{Encoding, Format, HeldPixels, Image, Layer, Rect, Sample};
use std::sync::Arc;

/// What a surface chooses to show live: an object's result, or one node of the
/// network it holds. What renders for it, and what runs on that, is the host's.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LiveResult {
    /// The object.
    pub object: crate::ObjectRef,
    /// The node of its network; none: the object's own result.
    pub node: Option<crate::NodeRef>,
}

/// A layer's pixels shown live ([`HeldPixels`]): what a cooked value showing
/// a render holds (a Layout piece placing a Render Output), so a new pass
/// recooks nothing. The canvas draws the result's current image in its place
/// each frame; a picture written reads the file the result last wrote in its
/// place. It saves as the reference, without pixels.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct Live {
    /// The result it shows.
    pub result: LiveResult,
    /// The file the result last wrote, when there is one on disk.
    pub file: Option<Arc<str>>,
}

impl Live {
    /// An Image of `size` pixels showing it: its main layer, colors in
    /// [`RENDER_SPACE`](kinogaki_render_api::RENDER_SPACE), premultiplied.
    ///
    /// # Errors
    /// A size of no pixels, or past an image's largest.
    pub fn image(self, size: [u32; 2]) -> Result<Image, Error> {
        let wide = |v: u32| i32::try_from(v).map_err(|_| Error::invalid("a result too large"));
        let window = Rect::new([0, 0], [wide(size[0])?, wide(size[1])?])?;
        let space = kinogaki_render_api::RENDER_SPACE;
        let format = Format::rgba(Sample::F16, Encoding::color(space, true));
        Image::builder(window).held(format, Arc::new(self)).build()
    }

    /// What `layer` shows live, when its pixels are.
    pub fn of(layer: &Layer) -> Option<&Live> {
        let held: &dyn std::any::Any = &**layer.held()?;
        held.downcast_ref()
    }
}

impl HeldPixels for Live {
    fn key(&self) -> u64 {
        use std::hash::{Hash, Hasher};
        let mut hasher = std::hash::DefaultHasher::new();
        self.hash(&mut hasher);
        hasher.finish()
    }
}

/// One piece a [`Picture`] paints.
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum CanvasPiece {
    /// A layer of an image. Its pixel `(x, y)` covers `x..x + 1`, `y..y + 1`
    /// of its own pixel plane (Y down), which `placed` puts on the canvas.
    Image {
        /// The image.
        image: Image,
        /// Which of its layers.
        layer: usize,
        /// Its pixel plane to canvas units (the XY plane; Z is ignored).
        placed: Xform,
        /// How much it covers what lies under it, 0 to 1.
        opacity: f32,
        /// The node that placed it, when one did and it can be arranged.
        source: Option<PieceSource>,
    },
    /// A block of text, composited with the images in its place among the
    /// pieces. Its plane has X 0 at the alignment point of its lines and
    /// Y 0 at the block's top, in the units `size` is in; `placed` puts it on
    /// the canvas. Core sets it at the size `placed` shows it, one pixel a
    /// canvas unit: what a file written at the picture's size shows.
    Text {
        /// The text; a line feed starts a line.
        text: Arc<str>,
        /// The face it is set in.
        face: TextFace,
        /// The em, in its plane's units.
        size: f64,
        /// How its lines sit against the alignment point.
        align: TextAlign,
        /// Its color: the code values of the display the picture is for, since
        /// a picture composes in that display's encoding (1 is display white).
        color: [f64; 3],
        /// Its plane to canvas units.
        placed: Xform,
        /// How much it covers what lies under it, 0 to 1.
        opacity: f32,
        /// The node that placed it, when one did and it can be arranged.
        source: Option<PieceSource>,
    },
}

impl CanvasPiece {
    /// Its plane to canvas units.
    pub fn placed(&self) -> &Xform {
        match self {
            Self::Image { placed, .. } | Self::Text { placed, .. } => placed,
        }
    }

    /// The same piece with its plane put on the canvas by `new`.
    pub fn replaced(&self, new: Xform) -> Self {
        let mut piece = self.clone();
        match &mut piece {
            Self::Image { placed, .. } | Self::Text { placed, .. } => *placed = new,
        }
        piece
    }

    /// The node that placed it.
    pub fn source(&self) -> Option<PieceSource> {
        match self {
            Self::Image { source, .. } | Self::Text { source, .. } => *source,
        }
    }

    /// What it shows live, when its image's layer is [`Live`].
    pub fn live(&self) -> Option<&Live> {
        match self {
            Self::Image { image, layer, .. } => Live::of(image.layers().get(*layer)?),
            Self::Text { .. } => None,
        }
    }
}

/// The node that placed a piece, which arranging it edits.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PieceSource {
    /// The node ([`CookCx::node`](crate::CookCx::node)): one of the open
    /// network's top level, for the piece to be picked there.
    pub node: NodeRef,
    /// The point of its plane the node's Translate handle moves, and that
    /// its Rotate and Scale values turn and scale about.
    pub pivot: [f64; 2],
}

/// A face [`CanvasPiece::Text`] is set in: those every Studio carries, so a
/// picture reads the same on every machine.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum TextFace {
    /// A proportional face (Ubuntu Light).
    #[default]
    Sans,
    /// A monospaced face (Hack).
    Mono,
    /// The interface's own face (MEK Sans).
    Interface,
}

/// How a [`CanvasPiece::Text`]'s lines sit against its alignment point.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextAlign {
    /// Each line starts there.
    #[default]
    Left,
    /// Each line's middle is there.
    Center,
    /// Each line ends there.
    Right,
}

/// What a value paints.
#[derive(Clone, Debug, PartialEq)]
pub struct Picture {
    /// What Fit frames and the canvas outlines, in canvas units.
    pub frame: Rect,
    /// The pieces, back to front.
    pub pieces: Vec<CanvasPiece>,
}

/// What the probe reads at a point.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Probe {
    /// What it shows, in order (the channels by name, as stored).
    pub rows: Vec<InfoRow>,
    /// The color there, when it is one: the host adds what it shows as.
    pub color: Option<ProbedColor>,
}

/// A color the probe read.
#[derive(Clone, Debug, PartialEq)]
pub enum ProbedColor {
    /// Linear in the working space: it shows through the view.
    Scene([f64; 3]),
    /// A display's code values, in its space by name: they show as they are.
    Display {
        /// The space.
        space: Arc<str>,
        /// The code values.
        codes: [f64; 3],
    },
}

impl ProbedColor {
    /// The color of pixel `rgba`, held as `encoding` says; none for data or
    /// a tag naming no space.
    pub fn of(
        encoding: &Encoding,
        [r, g, b, a]: [f32; 4],
        colors: &dyn ColorService,
    ) -> Option<Self> {
        let (Some(space), Encoding::Color { premultiplied, .. }) = (encoding.space(), encoding)
        else {
            return None;
        };
        let under = if *premultiplied && a > 0.0 { a } else { 1.0 };
        let mut held = [[r, g, b].map(|v| f64::from(v / under))];
        if colors.display_referred(space) {
            return Some(Self::Display {
                space: space.into(),
                codes: held[0],
            });
        }
        colors.to_working(space, &mut held).ok()?;
        Some(Self::Scene(held[0]))
    }
}

/// How a data type's values paint on the 2D canvas.
pub trait Canvas: Send + Sync {
    /// What `data` paints. It is called when the value changes, so it may
    /// take time proportional to how many pieces it holds, never to their
    /// pixels.
    fn picture(&self, data: &Data) -> Picture;

    /// What lies under canvas point `at` of `data`, `colors` converting to
    /// the working space.
    fn probe(&self, data: &Data, at: [f64; 2], colors: &dyn ColorService) -> Probe;
}
