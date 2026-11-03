//! [`Tabular`]: what the spreadsheet shows of a value.
//!
//! A value's tables are its domains (Points, Primitives, Detail…), each
//! with its built-in columns and its attributes. [`TableColumn::attribute`]
//! and [`Cells::attribute`] read any standard [`Attribute`], so every
//! type holding [`Attributes`](kinogaki_geometry::Attributes) shows them
//! alike: by storage and width, with the headings their meaning gives.
use crate::components::Components;
use crate::data::Data;
use kinogaki_geometry::{Attribute, Meaning, Storage, Values};
use std::ops::Range;

/// One of a value's tables (the spreadsheet's tabs: Points, Faces).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Table {
    /// The tab's text.
    pub name: String,
    /// How many rows.
    pub rows: usize,
}

/// What a column's cells hold.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum CellKind {
    /// Real numbers.
    Float,
    /// Lengths: real numbers in the project's length unit, which the
    /// spreadsheet names in the heading and shows at the project's
    /// precision. Their cells are [`Cells::Float`].
    Length,
    /// Whole numbers (indices, counts, ids), shown exactly.
    Integer,
    /// Text (names).
    Text,
}

/// One column of a table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableColumn {
    /// Its heading (`P`, `N`, `Cd`, `name`).
    pub name: String,
    /// Values per item: 3 for a vector, 16 for a 4 × 4 matrix, 1 for a
    /// scalar or text. The spreadsheet shows each as its own sub-column.
    pub width: usize,
    /// What the cells hold.
    pub kind: CellKind,
    /// What the values mean, which names the sub-columns: `.x .y .z`
    /// (points, vectors, normals, and plain values of width 2 to 4),
    /// `.r .g .b .a` (colors), `.u .v .w` (texture coordinates), a
    /// quaternion's `.x .y .z .w`, a matrix's `[r][c]`, else `[k]`.
    pub meaning: Meaning,
    /// Each row holds a list of items: its cells are [`Cells::Lists`].
    pub array: bool,
}

impl TableColumn {
    /// A column of `width` plain values a row of `kind`.
    pub fn new(name: &str, width: usize, kind: CellKind) -> Self {
        Self {
            name: name.to_owned(),
            width,
            kind,
            meaning: Meaning::None,
            array: false,
        }
    }

    /// Attribute `attribute`'s column, headed `name`; `None` for a storage
    /// this version doesn't show.
    pub fn attribute(name: &str, attribute: &Attribute) -> Option<Self> {
        let ty = attribute.ty();
        let kind = match ty.storage {
            Storage::F32 | Storage::F64 => CellKind::Float,
            Storage::I32 | Storage::I64 => CellKind::Integer,
            Storage::Text => CellKind::Text,
            _ => return None,
        };
        Some(Self {
            meaning: ty.meaning,
            array: ty.array,
            ..Self::new(name, usize::from(ty.width), kind)
        })
    }
}

/// A block of one column's cells: rows in order, each row's `width`
/// values together.
#[derive(Clone, Debug, PartialEq)]
pub enum Cells {
    /// Real numbers (a [`CellKind::Float`] or [`CellKind::Length`]
    /// column's): f32 values widened exactly.
    Float(Vec<f64>),
    /// Whole numbers, exact (i32 widened).
    Integer(Vec<i64>),
    /// Text.
    Text(Vec<String>),
    /// An array column's rows: row `r`'s items are
    /// `offsets[r]..offsets[r + 1]` of `items` (`width` values each);
    /// `offsets` starts at 0 and has a row more than the block.
    Lists {
        /// Where each row's items start, and where the last ends.
        offsets: Vec<u32>,
        /// The rows' items, in row order.
        items: Box<Cells>,
    },
}

impl Cells {
    /// The cells of `attribute` for the elements `rows` (clamped to its
    /// elements): its values by storage, an array's as [`Cells::Lists`].
    pub fn attribute(attribute: &Attribute, rows: Range<usize>) -> Cells {
        let elements = attribute.elements();
        let rows = rows.start.min(elements)..rows.end.min(elements);
        let Some(offsets) = attribute.offsets() else {
            return items(attribute, rows);
        };
        let first = offsets.get(rows.start).copied().unwrap_or(0);
        let last = offsets.get(rows.end).copied().unwrap_or(first);
        Cells::Lists {
            offsets: (rows.start..=rows.end)
                .map(|at| offsets[at] - first)
                .collect(),
            items: Box::new(items(attribute, first as usize..last as usize)),
        }
    }

    /// How many values it holds (rows' items for lists).
    pub fn len(&self) -> usize {
        match self {
            Cells::Float(v) => v.len(),
            Cells::Integer(v) => v.len(),
            Cells::Text(v) => v.len(),
            Cells::Lists { offsets, .. } => offsets.len().saturating_sub(1),
        }
    }

    /// Whether it holds none.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

/// Items `range` of `attribute`, `width` values each.
fn items(attribute: &Attribute, range: Range<usize>) -> Cells {
    let w = usize::from(attribute.ty().width);
    let values = range.start * w..range.end * w;
    match attribute.values() {
        Values::F32(v) => Cells::Float(values.map(|at| f64::from(v[at])).collect()),
        Values::F64(v) => Cells::Float(values.map(|at| v[at]).collect()),
        Values::I32(v) => Cells::Integer(values.map(|at| i64::from(v[at])).collect()),
        Values::I64(v) => Cells::Integer(values.map(|at| v[at]).collect()),
        Values::Text(t) => {
            Cells::Text(range.map(|i| t.get(i).unwrap_or_default().into()).collect())
        }
        _ => Cells::Text(Vec::new()),
    }
}

/// The spreadsheet's view of a data type. The spreadsheet shows only the
/// rows on screen, so it reads a column a range of rows at a time, and
/// only when its rows, or the value, change.
pub trait Tabular: Send + Sync {
    /// `data`'s tables, in tab order: one a domain it has.
    fn tables(&self, data: &Data) -> Vec<Table>;

    /// The columns of table `table` (an index into [`tables`](Self::tables)).
    fn columns(&self, data: &Data, table: usize) -> Vec<TableColumn>;

    /// Column `column`'s cells for `rows` of table `table`, of the column's
    /// [`CellKind`], `width` values a row ([`Cells::Lists`] for an array
    /// column).
    fn cells(&self, data: &Data, table: usize, column: usize, rows: Range<usize>) -> Cells;

    /// The rows of table `table` that `components` (a selection of `data`)
    /// selects, as the table's elements (faces selected as their points on
    /// a Points table), in ascending order: what the spreadsheet's
    /// Selected toggle keeps. `None` when the table's rows aren't
    /// components (a whole-value summary), so the toggle doesn't apply.
    ///
    /// Called off the UI thread; it may take time proportional to the
    /// value.
    fn selected(&self, data: &Data, table: usize, components: &Components) -> Option<Vec<u32>> {
        let _ = (data, table, components);
        None
    }
}
