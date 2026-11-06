//! The optional facets of a [`DataType`](crate::DataType): what the
//! environment's surfaces ask of a value beyond identity and storage.
//!
//! | Facet | Read by |
//! |---|---|
//! | [`Displayable`] | the viewport: packets at a quality, levels, highlights |
//! | [`Pickable`] | the viewport's picking: hits and boxes to components |
//! | [`Tabular`] | the spreadsheet |
//! | [`Renderable`] | render sync: neutral prims for every engine |
//! | [`Snappable`] | gizmo moves: what they may land on |
//! | [`Glyphs`] | the viewport: an object kind's line glyphs (lights) |
//! | [`Canvas`] | the 2D canvas pane: pieces in canvas units, a pixel probe |
//!
//! A type offers a facet by returning it from the matching `DataType`
//! method; a facet it doesn't return is absent and the surface leaves its
//! values out.
mod canvas;
mod display;
mod glyph;
mod object;
mod pick;
mod render;
mod snap;
mod table;

pub use canvas::{
    Canvas, CanvasPiece, Live, LiveResult, Picture, PieceSource, Probe, ProbedColor, TextAlign,
    TextFace,
};
pub use display::{DisplayRequest, Displayable, DrawPurpose, Drawn, Piece, Role, Tally};
pub use glyph::{GlyphScale, Glyphs};
pub use object::{Look, Marquee, Materials, ObjectPicking};
pub use pick::{Combine, Pickable};
pub use render::{RenderCx, Renderable, Rendered};
pub use snap::{SnapHit, SnapKind, SnapQuery, SnapTargets, Snappable};
pub use table::{CellKind, Cells, Table, TableColumn, Tabular};
