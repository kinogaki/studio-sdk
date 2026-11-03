//! [`Column<T>`]: the shared, immutable buffer every standard value keeps
//! its arrays in.
use crate::derived::Derived;
use std::fmt;
use std::ops::{Deref, Range};
use std::sync::{Arc, Weak};

/// A shared, immutable array of `T`, with its own cache slots.
///
/// - **No copy in.** [`Column::new`] adopts a `Vec` as it is: its buffer is
///   the column's, never reallocated.
/// - **Clones share.** A clone is the same column: the same buffer, the
///   same [`key`](Self::key) and the same cache slots.
/// - **Views share.** [`slice`](Self::slice) is a new column over part of
///   the same buffer, with a key and slots of its own.
/// - **Edits copy once.** An edit takes [`to_vec`](Self::to_vec), changes
///   it, and makes a new column of it.
///
/// ```
/// use kinogaki_geometry::Column;
///
/// let values = vec![1.0, 2.0, 3.0];
/// let address = values.as_ptr();
/// let column = Column::new(values);
/// assert_eq!(column.as_slice().as_ptr(), address); // adopted, not copied
/// let shared = column.clone();
/// assert!(shared.ptr_eq(&column) && shared.key() == column.key());
/// assert_eq!(&*column.slice(1..3), &[2.0, 3.0]);
/// ```
pub struct Column<T>(Arc<Inner<T>>);

struct Inner<T> {
    buffer: Arc<Vec<T>>,
    range: Range<usize>,
    derived: Derived,
}

impl<T> Column<T> {
    /// `values` as a column, without copying them.
    pub fn new(values: Vec<T>) -> Self {
        let range = 0..values.len();
        Self::over(Arc::new(values), range)
    }

    fn over(buffer: Arc<Vec<T>>, range: Range<usize>) -> Self {
        Column(Arc::new(Inner {
            buffer,
            range,
            derived: Derived::default(),
        }))
    }

    /// The values.
    #[inline]
    pub fn as_slice(&self) -> &[T] {
        &self.0.buffer[self.0.range.clone()]
    }

    /// How many values it holds.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.range.len()
    }

    /// Whether it holds no values.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.range.is_empty()
    }

    /// Whether `self` and `other` are one column (one is a clone of the
    /// other): the same values for certain, without comparing them.
    pub fn ptr_eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }

    /// The column's identity, for keying a cache within this session:
    /// equal keys of two live columns mean [`ptr_eq`](Self::ptr_eq). A key
    /// may be reused once the column is dropped, so it never outlives the
    /// column and is never saved.
    pub fn key(&self) -> u64 {
        Arc::as_ptr(&self.0) as *const () as usize as u64
    }

    /// The `D` derived from this column, built from its values by `build`
    /// the first time any clone asks; every clone shares it, and it is
    /// dropped with the column. Only the crate that defines `D` asks for
    /// it (the crate's cache rule).
    pub fn derived<D: Send + Sync + 'static>(&self, build: impl FnOnce(&[T]) -> D) -> Arc<D> {
        self.0.derived.get_or_build(|| build(self.as_slice()))
    }

    /// The values in `range` as a column over the same buffer: nothing is
    /// copied. It has its own key and cache slots.
    ///
    /// # Panics
    /// When `range` is not within `0..len()`.
    pub fn slice(&self, range: Range<usize>) -> Self {
        assert!(
            range.start <= range.end && range.end <= self.len(),
            "slice {range:?} of a column of {}",
            self.len()
        );
        let start = self.0.range.start;
        Self::over(
            self.0.buffer.clone(),
            start + range.start..start + range.end,
        )
    }

    /// The values copied into a `Vec`: the one copy an edit makes.
    pub fn to_vec(&self) -> Vec<T>
    where
        T: Clone,
    {
        self.as_slice().to_vec()
    }
}

/// A note of which column another came from, which keeps none of its
/// values alive and can't be mistaken for a later column: the allocation
/// stays reserved while the note lives.
pub(crate) struct Origin<T>(Weak<Inner<T>>);

impl<T> Origin<T> {
    /// A note of `column`.
    pub(crate) fn of(column: &Column<T>) -> Self {
        Origin(Arc::downgrade(&column.0))
    }

    /// Whether it notes `column`.
    pub(crate) fn is(&self, column: &Column<T>) -> bool {
        std::ptr::eq(self.0.as_ptr(), Arc::as_ptr(&column.0))
    }
}

impl<T> Clone for Column<T> {
    fn clone(&self) -> Self {
        Column(self.0.clone())
    }
}

impl<T> Default for Column<T> {
    fn default() -> Self {
        Self::new(Vec::new())
    }
}

impl<T> Deref for Column<T> {
    type Target = [T];
    #[inline]
    fn deref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> AsRef<[T]> for Column<T> {
    fn as_ref(&self) -> &[T] {
        self.as_slice()
    }
}

impl<T> From<Vec<T>> for Column<T> {
    fn from(values: Vec<T>) -> Self {
        Self::new(values)
    }
}

impl<T> FromIterator<T> for Column<T> {
    fn from_iter<I: IntoIterator<Item = T>>(values: I) -> Self {
        Self::new(values.into_iter().collect())
    }
}

/// Iterates the values by reference, as a slice does.
impl<'a, T> IntoIterator for &'a Column<T> {
    type Item = &'a T;
    type IntoIter = std::slice::Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.as_slice().iter()
    }
}

/// Columns are equal when their values are.
impl<T: PartialEq> PartialEq for Column<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr_eq(other) || self.as_slice() == other.as_slice()
    }
}

impl<T: fmt::Debug> fmt::Debug for Column<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const SHOWN: usize = 8;
        let values = self.as_slice();
        let mut list = f.debug_list();
        list.entries(values.iter().take(SHOWN));
        if values.len() > SHOWN {
            list.entry(&format_args!("… {} values", values.len()));
        }
        list.finish()
    }
}
