//! [`Column<T>`]: the shared, immutable array every standard value keeps its
//! values in, in shared fixed-size cells, so an edit copies the cells it
//! touches and a kept run shares them.
use crate::derived::Derived;
use std::fmt;
use std::ops::{Index, IndexMut, Range};
use std::sync::{Arc, OnceLock};

/// About how many bytes of values one cell holds: an edit's smallest copy.
const CELL_BYTES: usize = 64 << 10;

/// `log2` of how many `T` a cell holds.
const fn cell_shift<T>() -> u32 {
    let size = std::mem::size_of::<T>();
    let per_cell = CELL_BYTES / if size == 0 { 1 } else { size };
    per_cell.ilog2()
}

/// A shared, immutable array of `T`, with its own cache slots.
///
/// - **No copy in.** [`Column::new`] adopts a `Vec` as it is: its buffer is
///   the column's, never reallocated.
/// - **Clones share.** A clone is the same column: the same values, the
///   same [`key`](Self::key) and the same cache slots.
/// - **Edits copy cells.** A [`Writer`] copies only the fixed-size cells it
///   writes; every other cell stays shared with the column it came from, so
///   an edit's new memory follows the edit, not the column's size. A kept
///   run shifted by any amount shares its cells as long as its source is one
///   buffer.
/// - **Reads don't care.** `column[i]`, [`iter`](Self::iter) and
///   [`runs`](Self::runs) read the cells where they are.
///   [`as_slice`](Self::as_slice) is the one flat view: free for a column
///   made in one piece, one copy (kept) for one an edit left in pieces, so
///   code on an edit's path reads by cells and whole-value code may flatten.
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
/// assert_eq!(column.slice(1..3).as_slice(), &[2.0, 3.0]);
///
/// // A million values, one written: one cell is new, the rest are shared.
/// let long = Column::new(vec![0u32; 1 << 20]);
/// let mut edit = long.edit();
/// edit[1000] = 7;
/// let edited = edit.finish();
/// assert_eq!((long[1000], edited[1000], edited[999]), (0, 7, 0));
/// assert_eq!(edited.unshared(&long).count(), 1);
/// ```
pub struct Column<T>(Arc<Inner<T>>);

struct Inner<T> {
    /// The cells: cell `c` holds values `c << SHIFT` on.
    cells: Vec<Cell<T>>,
    len: usize,
    /// Whether the cells are one run of one buffer: a flat view is free.
    whole: bool,
    /// The flat view of a column in pieces, made when first asked for.
    flat: OnceLock<Vec<T>>,
    derived: Derived,
}

/// One cell's values: from `start` in `buffer` (which may hold more, as the
/// one buffer of a column made in one piece).
struct Cell<T> {
    buffer: Arc<Vec<T>>,
    start: usize,
}

impl<T> Clone for Cell<T> {
    fn clone(&self) -> Self {
        Self {
            buffer: self.buffer.clone(),
            start: self.start,
        }
    }
}

impl<T> Cell<T> {
    /// Whether `self` and `other` hold the same `len` values of one buffer.
    fn same(&self, other: &Self) -> bool {
        self.start == other.start && Arc::ptr_eq(&self.buffer, &other.buffer)
    }
}

impl<T> Column<T> {
    const SHIFT: u32 = cell_shift::<T>();
    const CELL: usize = 1 << Self::SHIFT;
    const MASK: usize = Self::CELL - 1;

    /// `values` as a column, without copying them.
    pub fn new(values: Vec<T>) -> Self {
        let len = values.len();
        let buffer = Arc::new(values);
        let cells = (0..len.div_ceil(Self::CELL))
            .map(|c| Cell {
                buffer: buffer.clone(),
                start: c << Self::SHIFT,
            })
            .collect();
        Self::of(cells, len)
    }

    fn of(cells: Vec<Cell<T>>, len: usize) -> Self {
        let whole = (cells.windows(2)).all(|w| {
            Arc::ptr_eq(&w[0].buffer, &w[1].buffer) && w[0].start + Self::CELL == w[1].start
        });
        Column(Arc::new(Inner {
            cells,
            len,
            whole,
            flat: OnceLock::new(),
            derived: Derived::default(),
        }))
    }

    /// `len` values made cell by cell (on the process's workers, in order of
    /// nothing): `fill(range, cell)` pushes the values of `range`. Cells are
    /// written once, with no flattening copy.
    #[track_caller]
    pub fn from_ranges(len: usize, fill: impl Fn(Range<usize>, &mut Vec<T>) + Sync) -> Self
    where
        T: Send + Sync,
    {
        let cell = Self::CELL;
        let blocks = crate::parallel::map(len, cell, |range| {
            let mut cells = Vec::with_capacity(range.len() / cell + 1);
            for start in range.clone().step_by(cell) {
                let end = (start + cell).min(range.end);
                let mut values = Vec::with_capacity(end - start);
                fill(start..end, &mut values);
                assert_eq!(values.len(), end - start);
                cells.push(Cell {
                    buffer: Arc::new(values),
                    start: 0,
                });
            }
            cells
        });
        Self::of(blocks.into_iter().flatten().collect(), len)
    }

    /// How many values it holds.
    #[inline]
    pub fn len(&self) -> usize {
        self.0.len
    }

    /// Whether it holds no values.
    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0.len == 0
    }

    /// Value `i`, if there is one.
    #[inline]
    pub fn get(&self, i: usize) -> Option<&T> {
        (i < self.0.len).then(|| &self[i])
    }

    /// The first value, if any.
    pub fn first(&self) -> Option<&T> {
        self.get(0)
    }

    /// The last value, if any.
    pub fn last(&self) -> Option<&T> {
        self.0.len.checked_sub(1).map(|i| &self[i])
    }

    /// The values in order, cell by cell.
    pub fn iter(&self) -> Iter<'_, T> {
        let mut cells = self.cells();
        Iter {
            current: cells.next().unwrap_or(&[]).iter(),
            back: [].iter(),
            cells,
        }
    }

    /// How many cells it holds.
    pub(crate) fn cell_count(&self) -> usize {
        self.0.cells.len()
    }

    /// Whether cell `c` of `self` and of `other` are one cell.
    pub(crate) fn shares_cell(&self, other: &Self, c: usize) -> bool {
        match (self.0.cells.get(c), other.0.cells.get(c)) {
            (Some(a), Some(b)) => a.same(b) && self.cell(c).len() == other.cell(c).len(),
            _ => false,
        }
    }

    /// Cell `c`'s values.
    #[inline]
    pub(crate) fn cell(&self, c: usize) -> &[T] {
        let cell = &self.0.cells[c];
        let len = (self.0.len - (c << Self::SHIFT)).min(Self::CELL);
        &cell.buffer[cell.start..cell.start + len]
    }

    /// Each cell's values, in order.
    fn cells(&self) -> Cells<'_, T> {
        Cells {
            column: self,
            cells: 0..self.0.cells.len(),
        }
    }

    /// The values as the fewest contiguous slices, in order: the reader of
    /// a whole column that doesn't want a flat view made for it.
    pub fn runs(&self) -> impl Iterator<Item = &[T]> {
        let mut c = 0;
        std::iter::from_fn(move || {
            let first = self.0.cells.get(c)?;
            let mut end = c + 1;
            while let Some(next) = self.0.cells.get(end)
                && Arc::ptr_eq(&next.buffer, &first.buffer)
                && next.start == first.start + ((end - c) << Self::SHIFT)
            {
                end += 1;
            }
            let values = (self.0.len.min(end << Self::SHIFT)) - (c << Self::SHIFT);
            c = end;
            Some(&first.buffer[first.start..first.start + values])
        })
    }

    /// The number of values before the first for which `before` is false,
    /// over values ordered so that it is true of a prefix: a binary search
    /// that reads the cells it lands on.
    pub fn partition_point(&self, before: impl Fn(&T) -> bool) -> usize {
        let (mut low, mut high) = (0, self.0.len);
        while low < high {
            let middle = low + (high - low) / 2;
            if before(&self[middle]) {
                low = middle + 1;
            } else {
                high = middle;
            }
        }
        low
    }

    /// Where a new run begins, past the first: the values before which
    /// [`run`](Self::run) can't look.
    pub(crate) fn breaks(&self) -> impl Iterator<Item = usize> {
        let cells = &self.0.cells;
        (1..cells.len())
            .filter(|&c| {
                let (a, b) = (&cells[c - 1], &cells[c]);
                !Arc::ptr_eq(&a.buffer, &b.buffer) || a.start + Self::CELL != b.start
            })
            .map(|c| c << Self::SHIFT)
    }

    /// The values in `range` when they lie in one buffer's run, without any
    /// copy: always for a column made in one piece, and for a range within a
    /// cell.
    #[inline]
    pub fn run(&self, range: Range<usize>) -> Option<&[T]> {
        assert!(
            range.start <= range.end && range.end <= self.0.len,
            "range {range:?} of a column of {}",
            self.0.len
        );
        if range.is_empty() {
            return Some(&[]);
        }
        let first = &self.0.cells[range.start >> Self::SHIFT];
        let cells = (range.start >> Self::SHIFT)..=((range.end - 1) >> Self::SHIFT);
        let joined = self.0.whole
            || cells.clone().enumerate().skip(1).all(|(k, c)| {
                let cell = &self.0.cells[c];
                Arc::ptr_eq(&cell.buffer, &first.buffer)
                    && cell.start == first.start + (k << Self::SHIFT)
            });
        joined.then(|| {
            let start = first.start + (range.start & Self::MASK);
            &first.buffer[start..start + range.len()]
        })
    }

    /// The values as one slice: free for a column made in one piece; a column
    /// an edit left in pieces copies them once and keeps the copy as long as
    /// it lives, so an edit's path reads by cells ([`iter`](Self::iter),
    /// `column[i]`, [`runs`](Self::runs)) instead.
    pub fn as_slice(&self) -> &[T]
    where
        T: Clone,
    {
        if self.0.whole {
            return self.run(0..self.0.len).unwrap_or(&[]);
        }
        self.0.flat.get_or_init(|| {
            let mut flat = Vec::with_capacity(self.0.len);
            self.runs().for_each(|run| flat.extend_from_slice(run));
            flat
        })
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

    /// The `D` derived from this column, built from it by `build` the first
    /// time any clone asks; every clone shares it, and it is dropped with
    /// the column. Only the crate that defines `D` asks for it (the crate's
    /// cache rule).
    pub fn derived<D: Send + Sync + 'static>(&self, build: impl FnOnce(&Self) -> D) -> Arc<D> {
        self.0.derived.get_or_build(|| build(self))
    }

    /// The ranges of values where `self` and `other` may differ, in order:
    /// the cells they don't share, and all of the longer one's values past
    /// the shorter. What it skips is equal for certain, read at the cost of
    /// the cell count; what it lists is for the caller to compare.
    pub fn unshared<'a>(&'a self, other: &'a Self) -> impl Iterator<Item = Range<usize>> + 'a {
        let shorter = self.0.len.min(other.0.len);
        let cells = shorter.div_ceil(Self::CELL);
        let (a, b) = (&self.0.cells, &other.0.cells);
        let differing = (0..cells).filter_map(move |c| {
            let start = c << Self::SHIFT;
            (!a[c].same(&b[c])).then(|| start..(start + Self::CELL).min(shorter))
        });
        let longer = self.0.len.max(other.0.len);
        let past = (shorter < longer).then_some(shorter..longer);
        differing.chain(past)
    }

    /// This column's values to change: nothing is copied until a value is
    /// written, and then only its cell.
    pub fn edit(&self) -> Writer<T> {
        Writer {
            cells: self.0.cells.clone(),
            held: self.0.len,
            open: Vec::new(),
        }
    }
}

impl<T: Clone> Column<T> {
    /// The values in `range` as a column: its own key and cache slots, its
    /// cells shared as far as they line up (always, for a column made in one
    /// piece).
    ///
    /// # Panics
    /// When `range` is not within `0..len()`.
    pub fn slice(&self, range: Range<usize>) -> Self {
        let mut part = Writer::new();
        part.keep(self, range);
        part.finish()
    }

    /// The values copied into a `Vec`.
    pub fn to_vec(&self) -> Vec<T> {
        let mut out = Vec::with_capacity(self.0.len);
        self.runs().for_each(|run| out.extend_from_slice(run));
        out
    }
}

/// A column being made: runs kept from other columns ([`keep`](Self::keep)),
/// values appended, and values written in place. It holds cells; a cell
/// shared with another column is copied the first time a value in it is
/// written, and never again.
pub struct Writer<T> {
    /// The cells before the open one; the last may be short only while
    /// `open` is empty.
    cells: Vec<Cell<T>>,
    /// The values the cells hold.
    held: usize,
    /// The cell being appended to, after `cells`: not yet shared, so a push
    /// is a `Vec`'s.
    open: Vec<T>,
}

impl<T> Default for Writer<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Writer<T> {
    /// A writer holding nothing.
    pub fn new() -> Self {
        Self {
            cells: Vec::new(),
            held: 0,
            open: Vec::new(),
        }
    }

    /// How many values it holds.
    #[inline]
    pub fn len(&self) -> usize {
        self.held + self.open.len()
    }

    /// Whether it holds no values.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The open cell closed: the cells hold everything.
    fn close(&mut self) {
        if !self.open.is_empty() {
            self.held += self.open.len();
            let values = std::mem::take(&mut self.open);
            self.cells.push(Cell {
                buffer: Arc::new(values),
                start: 0,
            });
        }
    }

    /// The column of it. No value is copied.
    pub fn finish(mut self) -> Column<T> {
        self.close();
        Column::of(self.cells, self.held)
    }
}

impl<T: Clone> Writer<T> {
    /// Cell `c` as a `Vec` of its own, copied if another column shares it;
    /// a cell of the cells.
    fn owned(&mut self, c: usize) -> &mut Vec<T> {
        let len = (self.held - (c << Column::<T>::SHIFT)).min(Column::<T>::CELL);
        let cell = &mut self.cells[c];
        if cell.start != 0 || cell.buffer.len() != len || Arc::get_mut(&mut cell.buffer).is_none() {
            let values = cell.buffer[cell.start..cell.start + len].to_vec();
            *cell = Cell {
                buffer: Arc::new(values),
                start: 0,
            };
        }
        Arc::get_mut(&mut cell.buffer).expect("a cell of its own")
    }

    /// Room for the next value in the open cell: a full one closes, and a
    /// short last cell opens again.
    #[inline]
    fn open_next(&mut self) {
        if self.open.len() == Column::<T>::CELL {
            self.close();
            // A long run: the next cell fills too.
            self.open.reserve_exact(Column::<T>::CELL);
        } else if self.open.is_empty() && !self.held.is_multiple_of(Column::<T>::CELL) {
            self.reopen();
        }
    }

    /// The short last cell's values become the open cell's.
    #[cold]
    fn reopen(&mut self) {
        let last = self.cells.len() - 1;
        let values = std::mem::take(self.owned(last));
        self.held -= values.len();
        self.open = values;
        self.cells.pop();
    }

    /// `value` next.
    #[inline]
    pub fn push(&mut self, value: T) {
        self.open_next();
        self.open.push(value);
    }

    /// `values` next.
    pub fn extend_from_slice(&mut self, mut values: &[T]) {
        while !values.is_empty() {
            self.open_next();
            let room = Column::<T>::CELL - self.open.len();
            let (now, rest) = values.split_at(room.min(values.len()));
            self.open.extend_from_slice(now);
            values = rest;
        }
    }

    /// `from`'s values in `range` next. A whole cell's worth shares its
    /// cells while the source's values are one buffer's run, wherever the
    /// range falls: a run kept after a removal shares what it can.
    ///
    /// # Panics
    /// When `range` is not within `from`'s values.
    pub fn keep(&mut self, from: &Column<T>, range: Range<usize>) {
        assert!(
            range.start <= range.end && range.end <= from.len(),
            "range {range:?} of a column of {}",
            from.len()
        );
        let (cell, mask, shift) = (Column::<T>::CELL, Column::<T>::MASK, Column::<T>::SHIFT);
        let mut at = range.start;
        while at < range.end {
            let left = range.end - at;
            if self.len().is_multiple_of(cell) {
                self.close();
                let taken = cell.min(left);
                if from.run(at..at + taken).is_some() {
                    let source = &from.0.cells[at >> shift];
                    self.cells.push(Cell {
                        buffer: source.buffer.clone(),
                        start: source.start + (at & mask),
                    });
                    self.held += taken;
                    at += taken;
                    continue;
                }
            }
            // Up to the next cell of ours or of the source.
            let values = from.cell(at >> shift);
            let start = at & mask;
            let room = cell - (self.len() & mask);
            let end = (start + room).min(values.len()).min(start + left);
            self.extend_from_slice(&values[start..end]);
            at += end - start;
        }
    }

    /// Keeps the first `len` values (no change when it holds fewer).
    pub fn truncate(&mut self, len: usize) {
        if len >= self.len() {
            return;
        }
        if len > self.held {
            self.open.truncate(len - self.held);
            return;
        }
        self.open.clear();
        self.cells.truncate(len.div_ceil(Column::<T>::CELL));
        self.held = len;
        // A cell cut short copies when it is next written.
    }
}

impl<T> Index<usize> for Writer<T> {
    type Output = T;
    #[inline]
    fn index(&self, i: usize) -> &T {
        assert!(i < self.len(), "value {i} of {}", self.len());
        if i >= self.held {
            return &self.open[i - self.held];
        }
        let cell = &self.cells[i >> Column::<T>::SHIFT];
        &cell.buffer[cell.start + (i & Column::<T>::MASK)]
    }
}

impl<T: Clone> IndexMut<usize> for Writer<T> {
    fn index_mut(&mut self, i: usize) -> &mut T {
        assert!(i < self.len(), "value {i} of {}", self.len());
        if i >= self.held {
            let at = i - self.held;
            return &mut self.open[at];
        }
        let c = i >> Column::<T>::SHIFT;
        &mut self.owned(c)[i & Column::<T>::MASK]
    }
}

/// Each cell of a column, in order.
struct Cells<'a, T> {
    column: &'a Column<T>,
    cells: Range<usize>,
}

impl<'a, T> Iterator for Cells<'a, T> {
    type Item = &'a [T];
    fn next(&mut self) -> Option<&'a [T]> {
        self.cells.next().map(|c| self.column.cell(c))
    }
}

impl<'a, T> DoubleEndedIterator for Cells<'a, T> {
    fn next_back(&mut self) -> Option<&'a [T]> {
        self.cells.next_back().map(|c| self.column.cell(c))
    }
}

/// A column's values in order ([`Column::iter`]).
pub struct Iter<'a, T> {
    current: std::slice::Iter<'a, T>,
    back: std::slice::Iter<'a, T>,
    cells: Cells<'a, T>,
}

impl<'a, T> Iterator for Iter<'a, T> {
    type Item = &'a T;
    #[inline]
    fn next(&mut self) -> Option<&'a T> {
        loop {
            if let Some(value) = self.current.next() {
                return Some(value);
            }
            match self.cells.next() {
                Some(cell) => self.current = cell.iter(),
                None => return self.back.next(),
            }
        }
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        let cells: usize = self.cells.cells.len() << Column::<T>::SHIFT;
        let exact = self.current.len() + self.back.len();
        match self.cells.cells.clone().next_back() {
            None => (exact, Some(exact)),
            Some(last) => {
                let partial = Column::<T>::CELL - self.cells.column.cell(last).len();
                let total = exact + cells - partial;
                (total, Some(total))
            }
        }
    }
    fn fold<B, F: FnMut(B, &'a T) -> B>(self, init: B, mut f: F) -> B {
        let mut acc = self.current.fold(init, &mut f);
        for cell in self.cells {
            acc = cell.iter().fold(acc, &mut f);
        }
        self.back.fold(acc, f)
    }
}

impl<T> DoubleEndedIterator for Iter<'_, T> {
    fn next_back(&mut self) -> Option<Self::Item> {
        loop {
            if let Some(value) = self.back.next_back() {
                return Some(value);
            }
            match self.cells.next_back() {
                Some(cell) => self.back = cell.iter(),
                None => return self.current.next_back(),
            }
        }
    }
}

impl<T> ExactSizeIterator for Iter<'_, T> {}

impl<T> Index<usize> for Column<T> {
    type Output = T;
    #[inline]
    fn index(&self, i: usize) -> &T {
        assert!(i < self.0.len, "value {i} of a column of {}", self.0.len);
        let cell = &self.0.cells[i >> Self::SHIFT];
        &cell.buffer[cell.start + (i & Self::MASK)]
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
    type IntoIter = Iter<'a, T>;
    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

/// Columns are equal when their values are: the cells they share aren't
/// read.
impl<T: PartialEq> PartialEq for Column<T> {
    fn eq(&self, other: &Self) -> bool {
        self.ptr_eq(other)
            || self.0.len == other.0.len
                && (0..self.0.cells.len()).all(|c| {
                    self.0.cells[c].same(&other.0.cells[c]) || self.cell(c) == other.cell(c)
                })
    }
}

impl<T: fmt::Debug> fmt::Debug for Column<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const SHOWN: usize = 8;
        let mut list = f.debug_list();
        list.entries(self.iter().take(SHOWN));
        if self.0.len > SHOWN {
            list.entry(&format_args!("… {} values", self.0.len));
        }
        list.finish()
    }
}
