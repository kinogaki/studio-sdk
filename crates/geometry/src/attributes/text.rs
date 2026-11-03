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
/// use kinogaki_geometry::Text;
///
/// let text = Text::from_strs(["/Materials/Red", "", "/Materials/Red", "/Materials/Blue"]);
/// assert_eq!(&**text.indices(), &[0, Text::NONE, 0, 1]);
/// assert_eq!(text.get(3), Some("/Materials/Blue"));
/// assert_eq!(text.get(1), None);
/// ```
#[derive(Clone, Debug)]
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

    /// Each element's text in order; `None` for no value.
    ///
    /// ```
    /// use kinogaki_geometry::Text;
    ///
    /// let parts = Text::from_strs(["wheel", "", "wheel", "axle"]);
    /// let all: Vec<_> = parts.iter().collect();
    /// assert_eq!(all, [Some("wheel"), None, Some("wheel"), Some("axle")]);
    /// assert_eq!(parts.iter().len(), 4);
    /// ```
    pub fn iter(&self) -> impl ExactSizeIterator<Item = Option<&str>> + '_ {
        self.indices
            .iter()
            .map(|&i| self.table.get(i as usize).map(|s| &**s))
    }

    /// The same values over a table of only the strings used, in the order
    /// of first use: what saving writes, so equal texts save alike. The
    /// text itself when its table already is so.
    ///
    /// ```
    /// use kinogaki_geometry::Text;
    ///
    /// let full = Text::from_strs(["a", "b", "c"]);
    /// // A gather keeping the table: "b" is unused and "c" comes first.
    /// let gathered = Text::new(vec![2, 0, Text::NONE].into(), full.table().clone()).unwrap();
    /// let compact = gathered.compacted();
    /// assert_eq!(&**compact.indices(), &[0, 1, Text::NONE]);
    /// assert_eq!(compact.table().len(), 2);
    /// assert_eq!(compact, gathered);
    /// ```
    pub fn compacted(&self) -> Text {
        let mut remap = vec![Self::NONE; self.table.len()];
        let mut used = Vec::new();
        let indices: Vec<u32> = self
            .indices
            .iter()
            .map(|&i| match remap.get_mut(i as usize) {
                Some(r) if *r == Self::NONE => {
                    *r = used.len() as u32;
                    used.push(self.table[i as usize].clone());
                    *r
                }
                Some(r) => *r,
                None => Self::NONE,
            })
            .collect();
        if used.len() == self.table.len() && indices.iter().eq(self.indices.iter()) {
            return self.clone();
        }
        Self {
            indices: Column::new(indices),
            table: used.into(),
        }
    }

    /// The elements in `range`, sharing the indices' buffer and the table.
    pub(crate) fn slice(&self, range: std::ops::Range<usize>) -> Self {
        Self {
            indices: self.indices.slice(range),
            table: self.table.clone(),
        }
    }
}

/// Texts are equal when every element's text is, whatever their tables.
impl PartialEq for Text {
    fn eq(&self, other: &Self) -> bool {
        if self.len() != other.len() {
            return false;
        }
        if Arc::ptr_eq(&self.table, &other.table) || self.table == other.table {
            return self.indices == other.indices;
        }
        self.iter().eq(other.iter())
    }
}

/// Builds a [`Text`] one element at a time, keeping each distinct string
/// once. `""` is pushed as no value.
#[derive(Debug, Default)]
pub(crate) struct TextBuilder {
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
    pub(crate) fn intern(&mut self, text: &str) -> u32 {
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
    pub(crate) fn push(&mut self, text: &str) -> u32 {
        let i = self.intern(text);
        self.indices.push(i);
        i
    }

    /// The text built.
    pub(crate) fn finish(self) -> Text {
        Text {
            indices: Column::new(self.indices),
            table: self.table.into(),
        }
    }
}
