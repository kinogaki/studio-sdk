//! [`Text`]: indexed text values, and [`TextBuilder`], which writes them.
use crate::column::Column;
use crate::error::{ErrorKind, GeometryError};
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

/// Text values, one per element, as an index per element into a table of
/// distinct strings. The index is what a renderer uses as a slot (one
/// material per index), and copying or comparing elements' values never
/// touches a string.
///
/// **No value.** [`Text::NONE`] (`u32::MAX`) is an element without a
/// value, and so is the empty string: builders store `""` as `NONE`, and
/// the table never holds `""`. [`get`](Self::get) gives `None` for it.
///
/// **Rules** ([`Text::new`] checks them): every index is below the table's
/// length or is `NONE`; the table's strings are distinct and none is
/// empty.
///
/// ```
/// use kinogaki_geometry::{Text, TextBuilder};
///
/// let mut materials = TextBuilder::new();
/// for m in ["/Materials/Red", "", "/Materials/Red", "/Materials/Blue"] {
///     materials.push(m);
/// }
/// let text = materials.finish();
/// assert_eq!(&**text.indices(), &[0, Text::NONE, 0, 1]);
/// assert_eq!(text.get(3), Some("/Materials/Blue"));
/// assert_eq!(text.get(1), None);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Text {
    indices: Column<u32>,
    table: Arc<[Arc<str>]>,
}

impl Text {
    /// The index of an element without a value.
    pub const NONE: u32 = u32::MAX;

    /// `indices` into `table`, checked.
    ///
    /// # Errors
    /// An index past the table (naming the element), a repeated or empty
    /// string in the table, or a table of [`Text::NONE`] or more strings.
    pub fn new(indices: Column<u32>, table: Arc<[Arc<str>]>) -> Result<Self, GeometryError> {
        if table.len() >= Self::NONE as usize {
            return Err(GeometryError::whole(
                ErrorKind::Attribute,
                "a text table holds fewer than 4294967295 strings",
            ));
        }
        let mut seen = HashSet::with_capacity(table.len());
        for (i, s) in table.iter().enumerate() {
            if s.is_empty() {
                return Err(GeometryError::at(
                    ErrorKind::Attribute,
                    i,
                    format!("text table entry {i} is empty: an empty string is no value"),
                ));
            }
            if !seen.insert(&**s) {
                return Err(GeometryError::at(
                    ErrorKind::Attribute,
                    i,
                    format!("text table entry {i} repeats {s:?}"),
                ));
            }
        }
        let size = table.len() as u32;
        if let Some(at) = indices.iter().position(|&i| i >= size && i != Self::NONE) {
            return Err(GeometryError::at(
                ErrorKind::Attribute,
                at,
                format!(
                    "element {at}'s text index {} is past the table of {size}",
                    indices[at]
                ),
            ));
        }
        Ok(Self { indices, table })
    }

    /// The texts of `values` in order, `""` as no value.
    pub fn from_strs<'a>(values: impl IntoIterator<Item = &'a str>) -> Self {
        let mut builder = TextBuilder::new();
        for v in values {
            builder.push(v);
        }
        builder.finish()
    }

    /// Each element's index into the table, or [`Text::NONE`].
    pub fn indices(&self) -> &Column<u32> {
        &self.indices
    }

    /// The distinct strings.
    pub fn table(&self) -> &Arc<[Arc<str>]> {
        &self.table
    }

    /// How many elements it has a value (or no value) for.
    pub fn len(&self) -> usize {
        self.indices.len()
    }

    /// Whether it is for no element.
    pub fn is_empty(&self) -> bool {
        self.indices.is_empty()
    }

    /// Element `i`'s text; `None` for no value.
    ///
    /// # Panics
    /// When `i` is past the elements.
    pub fn get(&self, i: usize) -> Option<&str> {
        self.table.get(self.indices[i] as usize).map(|s| &**s)
    }

    /// The elements in `range`, sharing the indices' buffer and the table.
    pub(crate) fn slice(&self, range: std::ops::Range<usize>) -> Self {
        Self {
            indices: self.indices.slice(range),
            table: self.table.clone(),
        }
    }
}

/// Builds a [`Text`] one element at a time, keeping each distinct string
/// once. `""` is pushed as no value.
#[derive(Debug, Default)]
pub struct TextBuilder {
    indices: Vec<u32>,
    table: Vec<Arc<str>>,
    known: HashMap<Arc<str>, u32>,
}

impl TextBuilder {
    /// An empty builder.
    pub fn new() -> Self {
        Self::default()
    }

    /// The index `text` has in the table being built (added if new);
    /// [`Text::NONE`] for `""`. Pushes nothing.
    pub fn intern(&mut self, text: &str) -> u32 {
        if text.is_empty() {
            return Text::NONE;
        }
        if let Some(&i) = self.known.get(text) {
            return i;
        }
        let i = self.table.len() as u32;
        let kept: Arc<str> = text.into();
        self.table.push(kept.clone());
        self.known.insert(kept, i);
        i
    }

    /// Adds an element with value `text` (`""`: no value), returning its
    /// index.
    pub fn push(&mut self, text: &str) -> u32 {
        let i = self.intern(text);
        self.indices.push(i);
        i
    }

    /// Adds an element with no value.
    pub fn push_none(&mut self) {
        self.indices.push(Text::NONE);
    }

    /// Adds an element by an index [`intern`](Self::intern) gave (or
    /// [`Text::NONE`]).
    ///
    /// # Panics
    /// When `index` is not one this builder gave.
    pub fn push_index(&mut self, index: u32) {
        assert!(
            index == Text::NONE || (index as usize) < self.table.len(),
            "text index {index} was not interned here"
        );
        self.indices.push(index);
    }

    /// The text built.
    pub fn finish(self) -> Text {
        Text {
            indices: Column::new(self.indices),
            table: self.table.into(),
        }
    }
}
