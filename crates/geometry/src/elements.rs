//! [`Elements`]: a set of the elements of one domain, the one shape a
//! group and a selection both hold.
use crate::Error;
use crate::column::Column;

/// A set of the elements of one domain: which of its
/// [`count`](Self::count) elements are members.
///
/// It keeps whichever form is smallest (every element; a short ascending
/// list of ids; one bit an element), and the forms are invisible: two sets
/// of the same members over the same count are equal whatever form each
/// holds, and every method answers alike. Immutable and cheap to clone.
///
/// **Rules** (each constructor checks them): every member is below the
/// count; bits past the last element are clear.
///
/// ```
/// use kinogaki_geometry::Elements;
///
/// let hidden = Elements::from_indices(100, [70, 3, 70]).unwrap();
/// assert!(hidden.contains(70) && !hidden.contains(4));
/// assert_eq!((hidden.len(), hidden.count()), (2, 100));
/// assert_eq!(hidden.iter().collect::<Vec<_>>(), [3, 70]);
/// // The bit form of the same members is the same set.
/// assert_eq!(Elements::from_bits(100, hidden.words()).unwrap(), hidden);
/// assert!(Elements::from_indices(4, [4]).is_err());
/// ```
#[derive(Clone, Debug)]
pub struct Elements {
    count: u32,
    repr: Repr,
}

/// The stored form (private: [`Elements`] says why).
#[derive(Clone, Debug)]
enum Repr {
    /// Every element.
    All,
    /// The members' ids, strictly ascending, none of them all.
    Sparse(Column<u32>),
    /// One bit an element (bit `i % 64` of word `i / 64`), with its
    /// member count.
    Dense(Column<u64>, u32),
}

impl Elements {
    /// No member of `count` elements.
    pub fn none(count: usize) -> Self {
        Self::sparse(count, Column::default())
    }

    /// Every one of `count` elements.
    pub fn all(count: usize) -> Self {
        if count == 0 {
            return Self::none(0);
        }
        Self {
            count: narrow(count),
            repr: Repr::All,
        }
    }

    /// The members `indices` (in any order, repeats allowed) of `count`
    /// elements.
    ///
    /// # Errors
    /// The first index not below `count`.
    pub fn from_indices(
        count: usize,
        indices: impl IntoIterator<Item = u32>,
    ) -> Result<Self, Error> {
        let mut ids: Vec<u32> = indices.into_iter().collect();
        if let Some(&past) = ids.iter().find(|&&i| i as usize >= count) {
            return Err(
                Error::invalid(format!("element {past} is past the last of {count}"))
                    .element(past as usize),
            );
        }
        if ids.windows(2).any(|w| w[0] >= w[1]) {
            ids.sort_unstable();
            ids.dedup();
        }
        Ok(Self::ascending(count, Column::new(ids)))
    }

    /// The members whose bits are set in `words` (bit `i % 64` of word
    /// `i / 64` is element `i`), of `count` elements: what a file holds.
    ///
    /// # Errors
    /// When the words don't cover exactly `count` bits
    /// (`count.div_ceil(64)` words), or a bit past the last element is
    /// set.
    pub fn from_bits(count: usize, words: Column<u64>) -> Result<Self, Error> {
        if words.len() != count.div_ceil(64) {
            return Err(Error::invalid(format!(
                "a set of {count} elements has {} words, not {}",
                words.len(),
                count.div_ceil(64)
            )));
        }
        if let Some(&last) = words.last() {
            let used = count % 64;
            if used != 0 && last >> used != 0 {
                return Err(Error::invalid(format!(
                    "a set of {count} elements holds bits past its last element"
                )));
            }
        }
        let members: u32 = words.iter().map(|w| w.count_ones()).sum();
        Ok(Self::dense(count, words, members))
    }

    /// The members `i` of `count` elements for which `f(i)`.
    ///
    /// ```
    /// use kinogaki_geometry::Elements;
    ///
    /// let every_third = Elements::from_fn(10, |i| i % 3 == 0);
    /// assert_eq!(every_third.iter().collect::<Vec<_>>(), [0, 3, 6, 9]);
    /// ```
    pub fn from_fn(count: usize, mut f: impl FnMut(u32) -> bool) -> Self {
        let ids: Vec<u32> = (0..narrow(count)).filter(|&i| f(i)).collect();
        Self::ascending(count, Column::new(ids))
    }

    /// `ids`, strictly ascending and below `count`, in the smallest form.
    fn ascending(count: usize, ids: Column<u32>) -> Self {
        debug_assert!(ids.windows(2).all(|w| w[0] < w[1]));
        if ids.len() == count {
            return Self::all(count);
        }
        // A list costs 32 bits a member, the bits one an element.
        if ids.len().saturating_mul(32) <= count {
            return Self::sparse(count, ids);
        }
        let mut words = vec![0u64; count.div_ceil(64)];
        for &i in ids.iter() {
            words[i as usize / 64] |= 1 << (i % 64);
        }
        Self::dense(count, Column::new(words), ids.len() as u32)
    }

    fn sparse(count: usize, ids: Column<u32>) -> Self {
        Self {
            count: narrow(count),
            repr: Repr::Sparse(ids),
        }
    }

    fn dense(count: usize, words: Column<u64>, members: u32) -> Self {
        if members as usize == count {
            return Self::all(count);
        }
        Self {
            count: narrow(count),
            repr: Repr::Dense(words, members),
        }
    }

    /// How many elements it is a set of: its domain's count.
    pub fn count(&self) -> usize {
        self.count as usize
    }

    /// How many members it has.
    pub fn len(&self) -> usize {
        match &self.repr {
            Repr::All => self.count as usize,
            Repr::Sparse(ids) => ids.len(),
            Repr::Dense(_, members) => *members as usize,
        }
    }

    /// Whether it has no member.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Whether every element is a member (none of none is not all).
    pub fn is_all(&self) -> bool {
        matches!(self.repr, Repr::All)
    }

    /// Whether element `i` is a member (`false` past the count).
    #[inline]
    pub fn contains(&self, i: u32) -> bool {
        match &self.repr {
            Repr::All => i < self.count,
            Repr::Sparse(ids) => ids.binary_search(&i).is_ok(),
            Repr::Dense(words, _) => i < self.count && words[i as usize / 64] >> (i % 64) & 1 == 1,
        }
    }

    /// The members, ascending.
    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        let (all, ids, words) = match &self.repr {
            Repr::All => (0..self.count, &[][..], &[][..]),
            Repr::Sparse(ids) => (0..0, &ids[..], &[][..]),
            Repr::Dense(words, _) => (0..0, &[][..], &words[..]),
        };
        let bits = words.iter().enumerate().flat_map(|(w, &bits)| {
            let mut rest = bits;
            std::iter::from_fn(move || {
                (rest != 0).then(|| {
                    let bit = rest.trailing_zeros();
                    rest &= rest - 1;
                    w as u32 * 64 + bit
                })
            })
        });
        all.chain(ids.iter().copied()).chain(bits)
    }

    /// The members as an ascending column (shared when it is already one).
    pub fn indices(&self) -> Column<u32> {
        match &self.repr {
            Repr::Sparse(ids) => ids.clone(),
            _ => self.iter().collect(),
        }
    }

    /// One bit an element, as a file saves it (shared when it is already
    /// held so).
    pub fn words(&self) -> Column<u64> {
        if let Repr::Dense(words, _) = &self.repr {
            return words.clone();
        }
        let mut words = vec![0u64; self.count().div_ceil(64)];
        for i in self.iter() {
            words[i as usize / 64] |= 1 << (i % 64);
        }
        Column::new(words)
    }

    /// The elements in either set.
    ///
    /// ```
    /// use kinogaki_geometry::Elements;
    ///
    /// let a = Elements::from_indices(5, [0, 1]).unwrap();
    /// let b = Elements::from_indices(5, [1, 4]).unwrap();
    /// let ids = |e: Elements| e.iter().collect::<Vec<_>>();
    /// assert_eq!(ids(a.union(&b).unwrap()), [0, 1, 4]);
    /// assert_eq!(ids(a.intersection(&b).unwrap()), [1]);
    /// assert_eq!(ids(a.difference(&b).unwrap()), [0]);
    /// assert_eq!(ids(a.symmetric_difference(&b).unwrap()), [0, 4]);
    /// assert!(a.union(&Elements::none(6)).is_err());
    /// ```
    ///
    /// # Errors
    /// When the sets are of different counts.
    pub fn union(&self, other: &Elements) -> Result<Elements, Error> {
        self.combine(other, |a, b| a | b)
    }

    /// The elements in both sets.
    ///
    /// # Errors
    /// When the sets are of different counts.
    pub fn intersection(&self, other: &Elements) -> Result<Elements, Error> {
        self.combine(other, |a, b| a & b)
    }

    /// The elements of this set not in `other`.
    ///
    /// # Errors
    /// When the sets are of different counts.
    pub fn difference(&self, other: &Elements) -> Result<Elements, Error> {
        self.combine(other, |a, b| a & !b)
    }

    /// The elements in exactly one of the sets.
    ///
    /// # Errors
    /// When the sets are of different counts.
    pub fn symmetric_difference(&self, other: &Elements) -> Result<Elements, Error> {
        self.combine(other, |a, b| a ^ b)
    }

    /// Member by member through `op` on membership; two short lists merge
    /// as lists, anything else combines word by word.
    fn combine(
        &self,
        other: &Elements,
        op: impl Fn(bool, bool) -> bool,
    ) -> Result<Elements, Error> {
        if self.count != other.count {
            return Err(Error::invalid(format!(
                "a set of {} elements combined with one of {}",
                self.count, other.count
            )));
        }
        let count = self.count();
        if let (Repr::Sparse(a), Repr::Sparse(b)) = (&self.repr, &other.repr) {
            let mut out = Vec::with_capacity(a.len() + b.len());
            let (mut i, mut j) = (0, 0);
            while i < a.len() || j < b.len() {
                let (x, y) = (a.get(i).copied(), b.get(j).copied());
                let id = match (x, y) {
                    (Some(x), Some(y)) => x.min(y),
                    (Some(x), None) => x,
                    (None, Some(y)) => y,
                    (None, None) => unreachable!("one list has more"),
                };
                let (in_a, in_b) = (x == Some(id), y == Some(id));
                if op(in_a, in_b) {
                    out.push(id);
                }
                i += usize::from(in_a);
                j += usize::from(in_b);
            }
            return Ok(Self::ascending(count, Column::new(out)));
        }
        let (a, b) = (self.words(), other.words());
        let words: Vec<u64> = a
            .iter()
            .zip(b.iter())
            .map(|(&a, &b)| {
                // Bit by bit through `op`, from the four membership cases.
                let both = if op(true, true) { a & b } else { 0 };
                let only_a = if op(true, false) { a & !b } else { 0 };
                let only_b = if op(false, true) { !a & b } else { 0 };
                both | only_a | only_b
            })
            .collect();
        let members = words.iter().map(|w| w.count_ones()).sum();
        Ok(Self::dense(count, Column::new(words), members))
    }
}

/// Sets are equal when they are of the same count and have the same
/// members, whatever form each keeps.
impl PartialEq for Elements {
    fn eq(&self, other: &Self) -> bool {
        if self.count != other.count || self.len() != other.len() {
            return false;
        }
        match (&self.repr, &other.repr) {
            (Repr::All, Repr::All) => true,
            (Repr::Sparse(a), Repr::Sparse(b)) => a == b,
            (Repr::Dense(a, _), Repr::Dense(b, _)) => a == b,
            _ => self.iter().eq(other.iter()),
        }
    }
}

/// A count as stored: the product limits keep every count below
/// `u32::MAX`.
fn narrow(count: usize) -> u32 {
    u32::try_from(count).expect("a count within the product limits")
}
