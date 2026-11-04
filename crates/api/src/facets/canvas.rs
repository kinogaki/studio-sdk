//! [`Canvas`]: what the 2D canvas pane paints of a value, a list of pieces
//! in canvas units, back to front: images under a placement, and text.
//! Pieces share their tiles with the value, never copying them; core uploads
//! and derives the levels it shows, and applies the view transform.
use crate::color::ColorService;
use crate::data::Data;
use crate::data_type::{Color, InfoRow};
use kinogaki_geometry::Xform;
use kinogaki_geometry::image::{Image, Rect};
use std::sync::Arc;

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
    },
    /// A line of text, over every image of the picture.
    Text {
        /// The text.
        text: Arc<str>,
        /// A line's height, in canvas units.
        height: f64,
        /// Its color, display-referred.
        color: Color,
        /// Where its top left is: the origin to canvas units.
        placed: Xform,
    },
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
    /// The color there, linear in the working space, when it is one: the
    /// host adds what the view transform makes of it.
    pub color: Option<[f64; 3]>,
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
