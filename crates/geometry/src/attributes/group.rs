//! [`Group`]: a named set of elements, as a bitset.
use crate::column::Column;
use crate::error::{ErrorKind, GeometryError};

/// A set of the elements of one class, one bit an element: bit `i % 64` of
/// word `i / 64` is element `i`.
///
/// **Rules** ([`Group::new`] checks them): the words cover exactly
/// [`elements`](Self::elements) bits (`elements.div_ceil(64)` words), and
/// the bits past the last element are clear, so two equal sets have equal
/// words.
///
/// ```
/// use kinogaki_geometry::Group;
///
/// let hidden = Group::from_indices(100, [3, 70]).unwrap();
/// assert!(hidden.contains(70) && !hidden.contains(4));
/// assert_eq!(hidden.members(), 2);
/// assert_eq!(hidden.iter().collect::<Vec<_>>(), [3, 70]);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Group {
    elements: usize,
    words: Column<u64>,
}

impl Group {
    /// The set over `elements` elements whose bits are `words`.
    ///
    /// # Errors
    /// When the words don't cover exactly `elements` bits, or a bit past
    /// the last element is set.
    pub fn new(elements: usize, words: Column<u64>) -> Result<Self, GeometryError> {
        if words.len() != elements.div_ceil(64) {
            return Err(GeometryError::whole(
                ErrorKind::Attribute,
                format!(
                    "a group of {elements} elements has {} words, not {}",
                    words.len(),
                    elements.div_ceil(64)
                ),
            ));
        }
        if let Some(&last) = words.last() {
            let used = elements % 64;
            if used != 0 && last >> used != 0 {
                return Err(GeometryError::whole(
                    ErrorKind::Attribute,
                    format!("a group of {elements} elements holds bits past its last element"),
                ));
            }
        }
        Ok(Self { elements, words })
    }

    /// The empty set over `elements` elements.
    pub fn none(elements: usize) -> Self {
        Self {
            elements,
            words: Column::new(vec![0; elements.div_ceil(64)]),
        }
    }

    /// The set over `elements` elements holding `indices`.
    ///
    /// # Errors
    /// When an index is not below `elements`.
    pub fn from_indices(
        elements: usize,
        indices: impl IntoIterator<Item = u32>,
    ) -> Result<Self, GeometryError> {
        let mut words = vec![0u64; elements.div_ceil(64)];
        for i in indices {
            if i as usize >= elements {
                return Err(GeometryError::at(
                    ErrorKind::Attribute,
                    i as usize,
                    format!("group member {i} is past its {elements} elements"),
                ));
            }
            words[i as usize / 64] |= 1 << (i % 64);
        }
        Ok(Self {
            elements,
            words: Column::new(words),
        })
    }

    /// The set of all `elements` elements.
    pub fn all(elements: usize) -> Self {
        let mut words = vec![u64::MAX; elements.div_ceil(64)];
        clear_tail(&mut words, elements);
        Self {
            elements,
            words: Column::new(words),
        }
    }

    /// The set over `elements` elements holding each `i` for which `f(i)`.
    ///
    /// ```
    /// use kinogaki_geometry::Group;
    ///
    /// let every_third = Group::from_fn(10, |i| i % 3 == 0);
    /// assert_eq!(every_third.iter().collect::<Vec<_>>(), [0, 3, 6, 9]);
    /// ```
    pub fn from_fn(elements: usize, mut f: impl FnMut(usize) -> bool) -> Self {
        let mut words = vec![0u64; elements.div_ceil(64)];
        for i in (0..elements).filter(|&i| f(i)) {
            words[i / 64] |= 1 << (i % 64);
        }
        Self {
            elements,
            words: Column::new(words),
        }
    }

    /// The elements in either set.
    ///
    /// ```
    /// use kinogaki_geometry::Group;
    ///
    /// let a = Group::from_indices(5, [0, 1]).unwrap();
    /// let b = Group::from_indices(5, [1, 4]).unwrap();
    /// assert_eq!(a.union(&b).unwrap().iter().collect::<Vec<_>>(), [0, 1, 4]);
    /// assert_eq!(a.intersection(&b).unwrap().iter().collect::<Vec<_>>(), [1]);
    /// assert_eq!(a.difference(&b).unwrap().iter().collect::<Vec<_>>(), [0]);
    /// assert_eq!(a.symmetric_difference(&b).unwrap().iter().collect::<Vec<_>>(), [0, 4]);
    /// assert_eq!(a.complement().iter().collect::<Vec<_>>(), [2, 3, 4]);
    /// assert!(a.union(&Group::none(6)).is_err());
    /// ```
    ///
    /// # Errors
    /// When the sets are over different numbers of elements.
    pub fn union(&self, other: &Group) -> Result<Group, GeometryError> {
        self.combine(other, |a, b| a | b)
    }

    /// The elements in both sets.
    ///
    /// # Errors
    /// When the sets are over different numbers of elements.
    pub fn intersection(&self, other: &Group) -> Result<Group, GeometryError> {
        self.combine(other, |a, b| a & b)
    }

    /// The elements of this set not in `other`.
    ///
    /// # Errors
    /// When the sets are over different numbers of elements.
    pub fn difference(&self, other: &Group) -> Result<Group, GeometryError> {
        self.combine(other, |a, b| a & !b)
    }

    /// The elements in exactly one of the sets.
    ///
    /// # Errors
    /// When the sets are over different numbers of elements.
    pub fn symmetric_difference(&self, other: &Group) -> Result<Group, GeometryError> {
        self.combine(other, |a, b| a ^ b)
    }

    /// The elements not in this set.
    pub fn complement(&self) -> Group {
        let mut words: Vec<u64> = self.words.iter().map(|w| !w).collect();
        clear_tail(&mut words, self.elements);
        Self {
            elements: self.elements,
            words: Column::new(words),
        }
    }

    /// Word by word; every `op` keeps the bits past the last element clear.
    fn combine(&self, other: &Group, op: impl Fn(u64, u64) -> u64) -> Result<Group, GeometryError> {
        if self.elements != other.elements {
            return Err(GeometryError::whole(
                ErrorKind::Attribute,
                format!(
                    "a group of {} elements combined with one of {}",
                    self.elements, other.elements
                ),
            ));
        }
        let words = self.words.iter().zip(other.words.iter());
        Ok(Self {
            elements: self.elements,
            words: words.map(|(&a, &b)| op(a, b)).collect(),
        })
    }

    /// How many elements it is over (the class's count).
    pub fn elements(&self) -> usize {
        self.elements
    }

    /// The bits.
    pub fn words(&self) -> &Column<u64> {
        &self.words
    }

    /// Whether element `i` is in the set (`false` past the elements).
    #[inline]
    pub fn contains(&self, i: usize) -> bool {
        i < self.elements && self.words[i / 64] >> (i % 64) & 1 == 1
    }

    /// How many elements are in the set.
    pub fn members(&self) -> usize {
        self.words.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// The elements in the set, ascending.
    pub fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.words.iter().enumerate().flat_map(|(w, &bits)| {
            let mut rest = bits;
            std::iter::from_fn(move || {
                (rest != 0).then(|| {
                    let bit = rest.trailing_zeros();
                    rest &= rest - 1;
                    w as u32 * 64 + bit
                })
            })
        })
    }

    /// The set over the elements in `range`, renumbered from 0.
    pub(crate) fn slice(&self, range: std::ops::Range<usize>) -> Self {
        Self::from_indices(
            range.len(),
            self.iter()
                .filter(|&i| range.contains(&(i as usize)))
                .map(|i| i - range.start as u32),
        )
        .expect("members within the range")
    }
}

/// Clears the bits past the last of `elements` elements.
fn clear_tail(words: &mut [u64], elements: usize) {
    if let Some(last) = words.last_mut()
        && !elements.is_multiple_of(64)
    {
        *last &= (1 << (elements % 64)) - 1;
    }
}
