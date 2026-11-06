//! [`DataType`]: everything the environment needs to handle values it
//! can't look inside: identity, a wire color ([`WireColor`]), a content stamp, memory,
//! bounds, info rows and how the value saves, plus optional facets for
//! display, picking, the spreadsheet and rendering.
//!
//! A package implements one `DataType` per kind of value it defines beyond
//! the standard types (the Mesh plugin's `kinogaki.mesh.steps`, Edit Mesh's
//! held steps) and registers it in its [`Package`](crate::Package); a
//! kernel's operations work on the standard values and register no type of
//! their own for the same data.
//! A facet a type doesn't offer is simply absent: a type without
//! [`Displayable`] isn't drawn, one without [`Renderable`] isn't rendered,
//! and nothing fails.
use crate::codec::DataCodec;
use crate::data::Data;
use crate::facets::{
    Canvas, Displayable, Look, ObjectPicking, Pickable, Renderable, Snappable, Tabular,
};
use crate::id::DataTypeId;
use kinogaki_geometry::Bounds;

/// The color a data type's wires and ports are drawn in, 8-bit sRGB and
/// opaque ([`DataType::wire_color`]). What else shows on screen takes its
/// colors from the theme, and a part draws in the UI layer's own color.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WireColor {
    /// Red, sRGB-encoded.
    pub r: u8,
    /// Green, sRGB-encoded.
    pub g: u8,
    /// Blue, sRGB-encoded.
    pub b: u8,
}

impl WireColor {
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

/// `value`'s hash as a [`Stamp`]: for a data type whose content hashes
/// (a list of edits). A value hashing its floats by their bits gets one stamp
/// per bit pattern.
pub fn stamp_of(value: &impl std::hash::Hash) -> Stamp {
    use std::hash::Hasher;
    let word = |salt: u64| {
        let mut hasher = std::hash::DefaultHasher::new();
        hasher.write_u64(salt);
        value.hash(&mut hasher);
        hasher.finish().to_le_bytes()
    };
    let mut stamp = [0; 32];
    for (salt, bytes) in stamp.chunks_mut(8).enumerate() {
        bytes.copy_from_slice(&word(salt as u64));
    }
    stamp
}

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
    fn wire_color(&self) -> WireColor;

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

    /// How objects holding values of this type are picked and framed at
    /// the object levels, when they are.
    fn picking(&self) -> Option<&dyn ObjectPicking> {
        None
    }

    /// How values look against the scene's materials, when they take any.
    fn look(&self) -> Option<&dyn Look> {
        None
    }

    /// What gizmo moves may land on in values, when anything.
    fn snapping(&self) -> Option<&dyn Snappable> {
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

    /// How values paint on the 2D canvas, when they do.
    fn canvas(&self) -> Option<&dyn Canvas> {
        None
    }

    /// The type's empty value (an empty mesh), when it has one: what a
    /// bypassed node gives on outputs it doesn't pass its input to.
    /// `None` by default: reading such an output then fails.
    fn empty(&self) -> Option<Data> {
        None
    }

    /// Whether a value holds nothing (a mesh without points or faces),
    /// answered from what it holds, never by hashing it: hosts ask it of
    /// shown values every frame. `None` by default: the type can't tell.
    fn is_empty(&self, _: &Data) -> Option<bool> {
        None
    }
}
