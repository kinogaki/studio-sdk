//! [`Attribute`]: one typed column of values, and [`Tuple`], the Rust
//! types its values are read and adopted as.
use super::text::Text;
use super::types::{AttributeType, Meaning, Storage};
use crate::Error;
use crate::column::Column;

/// An attribute's values, flat: `width` values an item, items in element
/// order (interleaved, so a `[f32; 3]` normal column uploads and indexes
/// as it is).
#[derive(Clone, Debug, PartialEq)]
#[non_exhaustive]
pub enum Values {
    /// Single-precision reals.
    F32(Column<f32>),
    /// Double-precision reals.
    F64(Column<f64>),
    /// 32-bit whole numbers.
    I32(Column<i32>),
    /// 64-bit whole numbers.
    I64(Column<i64>),
    /// Indexed text, one string an item.
    Text(Text),
}

impl Values {
    /// Their storage.
    pub fn storage(&self) -> Storage {
        match self {
            Values::F32(_) => Storage::F32,
            Values::F64(_) => Storage::F64,
            Values::I32(_) => Storage::I32,
            Values::I64(_) => Storage::I64,
            Values::Text(_) => Storage::Text,
        }
    }

    /// How many values (strings for text).
    pub fn len(&self) -> usize {
        match self {
            Values::F32(c) => c.len(),
            Values::F64(c) => c.len(),
            Values::I32(c) => c.len(),
            Values::I64(c) => c.len(),
            Values::Text(t) => t.len(),
        }
    }

    /// Whether there are none.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// The buffer's identity, for keying a cache built from these values
    /// ([`Column::key`]; text's indices).
    pub fn key(&self) -> u64 {
        match self {
            Values::F32(c) => c.key(),
            Values::F64(c) => c.key(),
            Values::I32(c) => c.key(),
            Values::I64(c) => c.key(),
            Values::Text(t) => t.indices().key(),
        }
    }

    /// Value `i` widened to a real: reals and whole numbers (an `i64`
    /// past 2⁵³ rounds); `None` for text or past the end.
    pub fn real(&self, i: usize) -> Option<f64> {
        Some(match self {
            Values::F32(c) => f64::from(*c.get(i)?),
            Values::F64(c) => *c.get(i)?,
            Values::I32(c) => f64::from(*c.get(i)?),
            Values::I64(c) => *c.get(i)? as f64,
            Values::Text(_) => return None,
        })
    }

    /// The first value that is NaN or infinite (reals only) among the cells
    /// `self` doesn't share with `base`, of their storage.
    fn first_not_finite_after(&self, base: &Values) -> Option<usize> {
        fn first<T: Copy>(
            new: &Column<T>,
            old: &Column<T>,
            finite: impl Fn(T) -> bool,
        ) -> Option<usize> {
            new.written_since(old)
                .find_map(|r| r.into_iter().find(|&i| !finite(new[i])))
        }
        match (self, base) {
            (Values::F32(new), Values::F32(old)) => first(new, old, f32::is_finite),
            (Values::F64(new), Values::F64(old)) => first(new, old, f64::is_finite),
            _ => None,
        }
    }

    /// The first value that is NaN or infinite (reals only), checked in
    /// parallel.
    fn first_not_finite(&self) -> Option<usize> {
        const BLOCK: usize = 262_144;
        match self {
            Values::F32(c) => {
                crate::parallel::first_refused(c.as_slice(), BLOCK, |x| x.is_finite())
            }
            Values::F64(c) => {
                crate::parallel::first_refused(c.as_slice(), BLOCK, |x| x.is_finite())
            }
            Values::I32(_) | Values::I64(_) | Values::Text(_) => None,
        }
    }
}

/// One attribute: values of one [`AttributeType`], an item per element,
/// or with [`offsets`](Self::offsets) an array of items per element.
///
/// **Rules** (the constructors check them, once, so a table holding it
/// only compares counts):
/// - the type passes [`AttributeType::check`], and the values are of its
///   storage;
/// - the values are whole items (a multiple of the width);
/// - every real is finite;
/// - arrays: offsets start at 0, never decrease, and end at the item
///   count; element `e`'s items are `offsets[e]..offsets[e + 1]`.
///
/// ```
/// use kinogaki_geometry::{Attribute, Meaning};
///
/// let normals = vec![[0.0f32, 0.0, 1.0]; 4];
/// let address = normals.as_ptr() as usize;
/// let n = Attribute::tuples(normals, Meaning::Normal).unwrap();
/// assert_eq!(n.elements(), 4);
/// let view: &[[f32; 3]] = n.as_tuples().unwrap();
/// assert_eq!(view.as_ptr() as usize, address); // adopted, not copied
/// assert!(n.as_tuples::<[f64; 3]>().is_none());
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Attribute {
    ty: AttributeType,
    values: Values,
    offsets: Option<Column<u32>>,
}

impl Attribute {
    /// `values` as one item of `ty` an element.
    ///
    /// # Errors
    /// A rule broken, or `ty` an array type.
    pub fn new(ty: AttributeType, values: Values) -> Result<Self, Error> {
        if ty.array {
            return Err(refused(
                "an array attribute is made by Attribute::arrays".into(),
            ));
        }
        Self::checked(ty, values, None)
    }

    /// `values` in place of this attribute's, of its type, each item for an
    /// element: what an edit makes of it. Only the cells they don't share
    /// with this attribute's values are checked.
    ///
    /// # Errors
    /// A rule the new cells break, or an array attribute (whose items may
    /// move: [`arrays`](Self::arrays) checks them).
    pub fn with_values(&self, values: Values) -> Result<Self, Error> {
        if self.offsets.is_some() || values.storage() != self.values.storage() {
            return Err(refused("no such values of this attribute".into()));
        }
        let width = usize::from(self.ty.width);
        if !values.len().is_multiple_of(width) {
            return Err(refused(format!(
                "{} values aren't whole items of width {width}",
                values.len()
            )));
        }
        if let Some(at) = values.first_not_finite_after(&self.values) {
            return Err(
                Error::invalid(format!("value {at} is not finite")).element(at / width.max(1))
            );
        }
        Ok(Self {
            ty: self.ty,
            values,
            offsets: None,
        })
    }

    /// `values` as arrays of items of `ty`: element `e`'s are the items
    /// `offsets[e]..offsets[e + 1]`.
    ///
    /// # Errors
    /// A rule broken, or `ty` not an array type.
    pub fn arrays(ty: AttributeType, offsets: Column<u32>, values: Values) -> Result<Self, Error> {
        if !ty.array {
            return Err(refused(
                "an attribute of one item an element is made by Attribute::new".into(),
            ));
        }
        Self::checked(ty, values, Some(offsets))
    }

    /// `values` adopted without copying, one tuple an element: `Vec<f32>`
    /// is width 1, `Vec<[f32; 3]>` width 3, `Vec<[i64; 2]>` width 2.
    ///
    /// # Errors
    /// A width `meaning` doesn't have, or a real that isn't finite.
    pub fn tuples<T: Tuple>(values: Vec<T>, meaning: Meaning) -> Result<Self, Error> {
        let ty = AttributeType::new(T::Scalar::STORAGE, T::WIDTH, meaning);
        let flat = bytemuck::allocation::try_cast_vec::<T, T::Scalar>(values)
            .unwrap_or_else(|(_, v)| bytemuck::cast_slice(&v).to_vec());
        Self::new(ty, T::Scalar::values(Column::new(flat)))
    }

    /// Indexed text, one string an element.
    pub fn text(text: Text) -> Self {
        Self {
            ty: AttributeType::TEXT,
            values: Values::Text(text),
            offsets: None,
        }
    }

    fn checked(
        ty: AttributeType,
        values: Values,
        offsets: Option<Column<u32>>,
    ) -> Result<Self, Error> {
        ty.check()?;
        if values.storage() != ty.storage {
            return Err(refused(format!(
                "{} values for a {} attribute",
                values.storage().name(),
                ty.describe()
            )));
        }
        let width = usize::from(ty.width);
        if !values.len().is_multiple_of(width) {
            return Err(refused(format!(
                "{} values are not whole items of {width}",
                values.len()
            )));
        }
        if let Some(i) = values.first_not_finite() {
            return Err(Error::invalid(format!("value {i} is not finite")).element(i / width));
        }
        if let Some(offsets) = &offsets {
            check_offsets(offsets, values.len() / width)?;
        }
        Ok(Self {
            ty,
            values,
            offsets,
        })
    }

    /// Its type.
    pub fn ty(&self) -> AttributeType {
        self.ty
    }

    /// How many elements it is for.
    pub fn elements(&self) -> usize {
        match &self.offsets {
            Some(o) => o.len().saturating_sub(1),
            None => self.items(),
        }
    }

    /// How many items it holds: its elements', or every array's together.
    pub fn items(&self) -> usize {
        self.values.len() / usize::from(self.ty.width)
    }

    /// The values.
    pub fn values(&self) -> &Values {
        &self.values
    }

    /// An array attribute's offsets: `elements() + 1` of them.
    pub fn offsets(&self) -> Option<&Column<u32>> {
        self.offsets.as_ref()
    }

    /// The items as `T`, without copying: one an element, or every
    /// array's items in order. `None` when `T` isn't the storage and width.
    pub fn as_tuples<T: Tuple>(&self) -> Option<&[T]> {
        if self.ty.width != T::WIDTH {
            return None;
        }
        let flat = T::Scalar::column(&self.values)?;
        bytemuck::try_cast_slice(flat.as_slice()).ok()
    }

    /// The text, when it holds text.
    pub(crate) fn as_text(&self) -> Option<&Text> {
        match &self.values {
            Values::Text(t) => Some(t),
            _ => None,
        }
    }
}

fn check_offsets(offsets: &Column<u32>, items: usize) -> Result<(), Error> {
    match offsets.first() {
        Some(0) => {}
        _ => return Err(refused("an array attribute's offsets start at 0".into())),
    }
    if let Some(e) = offsets.as_slice().windows(2).position(|w| w[1] < w[0]) {
        return Err(
            Error::invalid(format!("element {e}'s array ends before it starts")).element(e),
        );
    }
    let end = *offsets.last().unwrap_or(&0) as usize;
    if end != items {
        return Err(refused(format!(
            "the arrays end at item {end} of {items} items"
        )));
    }
    Ok(())
}

fn refused(why: String) -> Error {
    Error::invalid(why)
}

mod sealed {
    use super::{Column, Storage, Values};

    /// A storage's Rust type: `f32`, `f64`, `i32` or `i64`.
    pub trait Scalar: bytemuck::Pod {
        /// The storage it is.
        const STORAGE: Storage;
        /// The column, when `values` are of this storage.
        fn column(values: &Values) -> Option<&Column<Self>>;
        /// `column` as values.
        fn values(column: Column<Self>) -> Values;
    }

    /// Closes [`Tuple`](super::Tuple) to the types below.
    pub trait Sealed {}
}

use sealed::Scalar;

/// What an attribute's items are read and adopted as: a storage's type
/// `S` (`f32`, `f64`, `i32` or `i64`; width 1) or `[S; N]` for `N` in 1
/// to 16. Sealed.
pub trait Tuple: bytemuck::Pod + sealed::Sealed {
    /// The storage's type.
    #[doc(hidden)]
    type Scalar: Scalar;
    /// Values an item.
    const WIDTH: u8;
}

macro_rules! scalar {
    ($t:ty, $storage:ident) => {
        impl sealed::Sealed for $t {}
        impl Scalar for $t {
            const STORAGE: Storage = Storage::$storage;
            fn column(values: &Values) -> Option<&Column<Self>> {
                match values {
                    Values::$storage(c) => Some(c),
                    _ => None,
                }
            }
            fn values(column: Column<Self>) -> Values {
                Values::$storage(column)
            }
        }
        impl Tuple for $t {
            type Scalar = $t;
            const WIDTH: u8 = 1;
        }
        scalar!(@arrays $t; 1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16);
    };
    (@arrays $t:ty; $($n:literal)*) => {
        $(
            impl sealed::Sealed for [$t; $n] {}
            impl Tuple for [$t; $n] {
                type Scalar = $t;
                const WIDTH: u8 = $n;
            }
        )*
    };
}

scalar!(f32, F32);
scalar!(f64, F64);
scalar!(i32, I32);
scalar!(i64, I64);
