//! [`DataType`]: everything the environment needs to handle values it
//! can't look inside: identity, a wire color, a content stamp, memory,
//! bounds, info rows and how the value saves, plus optional facets for
//! display, picking, the spreadsheet and rendering.
//!
//! A kernel package implements one `DataType` per kind of value it defines
//! (`kinogaki.mesh.geometry` over lib-mesh's `Mesh`, `kinogaki.cad.body`
//! over lib-cad's bodies) and registers it in its [`Package`](crate::Package).
//! A facet a type doesn't offer is simply absent: a type without
//! [`Displayable`] isn't drawn, one without [`Renderable`] isn't rendered,
//! and nothing fails.
use crate::codec::DataCodec;
use crate::data::Data;
use crate::facets::{Displayable, Pickable, Renderable, Tabular};
use crate::id::DataTypeId;
use crate::math::Bounds;

/// A display color: 8-bit sRGB. Wires of a data type, and the tiles of
/// object kinds, are drawn in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Color {
    /// Red, sRGB-encoded.
    pub r: u8,
    /// Green, sRGB-encoded.
    pub g: u8,
    /// Blue, sRGB-encoded.
    pub b: u8,
}

impl Color {
    /// The color with sRGB components `r`, `g`, `b`.
    pub const fn rgb(r: u8, g: u8, b: u8) -> Self {
        Self { r, g, b }
    }
}

/// One row of the info panel: what is counted or measured, and its value
/// as the user reads it ("Points", "1,024").
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct InfoRow {
    /// What the row is ("Faces", "Shells", "Tolerance").
    pub label: String,
    /// Its value, formatted for display.
    pub value: String,
}

impl InfoRow {
    /// The row `label`: `value`.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

/// A content hash: equal stamps mean equal values, whichever allocations
/// hold them (a recook that produces the same geometry, a value read back
/// from a file). Caches that outlive one allocation key on it.
pub type Stamp = [u8; 32];

/// A kind of value that flows on wires and stands as an object's result.
///
/// Every method takes values of this type only: the environment calls a
/// `DataType` with the [`Data`] whose [`ty`](Data::ty) is its
/// [`id`](Self::id), and an implementation may treat any other value as a
/// bug of the caller (returning an empty answer, never panicking across
/// the boundary). Methods are called from any thread.
pub trait DataType: Send + Sync {
    /// Its id (`kinogaki.mesh.geometry`).
    fn id(&self) -> DataTypeId;

    /// What a UI calls it ("Geometry", "Body").
    fn label(&self) -> &str;

    /// The color its wires and ports are drawn in.
    fn wire_color(&self) -> Color;

    /// `data`'s content hash ([`Stamp`]).
    fn stamp(&self, data: &Data) -> Stamp;

    /// Bytes `data` holds, for the memory budget and the info panel.
    /// Storage shared with other values counts in full.
    fn memory(&self, data: &Data) -> usize;

    /// `data`'s bounds; `None` when it is empty.
    fn bounds(&self, data: &Data) -> Option<Bounds>;

    /// The info panel's rows for `data`: counts and measures, in order.
    fn info(&self, data: &Data) -> Vec<InfoRow>;

    /// How values of this type save into and load from a project.
    fn codec(&self) -> &dyn DataCodec;

    /// How values show in the viewport, when they do.
    fn displayable(&self) -> Option<&dyn Displayable> {
        None
    }

    /// How viewport picks become component selections, when values can
    /// be picked below the object level.
    fn pickable(&self) -> Option<&dyn Pickable> {
        None
    }

    /// The spreadsheet's tables, when values have any.
    fn tabular(&self) -> Option<&dyn Tabular> {
        None
    }

    /// How values reach render engines, when they render.
    fn renderable(&self) -> Option<&dyn Renderable> {
        None
    }

    /// The type's empty value (an empty mesh), when it has one: what a
    /// bypassed node gives on outputs it doesn't pass its input to.
    /// `None` by default: reading such an output then fails.
    fn empty(&self) -> Option<Data> {
        None
    }
}
