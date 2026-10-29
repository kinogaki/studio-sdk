//! [`Tabular`]: what the spreadsheet shows of a value.
use crate::components::Components;
use crate::data::Data;
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
    /// Whole numbers (indices, counts, ids).
    Integer,
    /// Text (names).
    Text,
}

/// One column of a table.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TableColumn {
    /// Its heading (`P`, `N`, `Cd`, `name`).
    pub name: String,
    /// Values per row: 3 for a vector, 1 for a scalar. The spreadsheet
    /// shows each as its own sub-column.
    pub width: usize,
    /// What the cells hold.
    pub kind: CellKind,
}

/// A block of one column's cells: rows in order, each row's `width`
/// values together.
#[derive(Clone, Debug, PartialEq)]
pub enum Cells {
    /// Real numbers (a [`CellKind::Float`] or [`CellKind::Length`]
    /// column's).
    Float(Vec<f64>),
    /// Whole numbers.
    Integer(Vec<i64>),
    /// Text.
    Text(Vec<String>),
}

/// The spreadsheet's view of a data type. The spreadsheet shows only the
/// rows on screen, so it reads a column a range of rows at a time.
pub trait Tabular: Send + Sync {
    /// `data`'s tables, in tab order.
    fn tables(&self, data: &Data) -> Vec<Table>;

    /// The columns of table `table` (an index into [`tables`](Self::tables)).
    fn columns(&self, data: &Data, table: usize) -> Vec<TableColumn>;

    /// Column `column`'s cells for `rows` of table `table`, of the column's
    /// [`CellKind`], `width` values a row.
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
