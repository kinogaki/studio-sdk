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
//!
//! A type offers a facet by returning it from the matching `DataType`
//! method; a facet it doesn't return is absent and the surface leaves its
//! values out.
mod display;
mod object;
mod pick;
mod render;
mod snap;
mod table;

pub use display::{
    DisplayQuality, DisplayRequest, Displayable, DrawPurpose, Drawn, Piece, Role, Tally, WhenOpen,
};
pub use object::{Look, Materials, Matrix, ObjectPicking, WorldRay};
pub use pick::{Combine, Pickable};
pub use render::Renderable;
pub use snap::{SnapHit, SnapKind, SnapQuery, SnapTargets, Snappable};
pub use table::{CellKind, Cells, Table, TableColumn, Tabular};
