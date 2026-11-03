//! [`AttributeType`]: an attribute's storage, width, meaning and whether
//! each element holds an array.
use crate::error::{ErrorKind, GeometryError};

/// How an attribute's values are stored.
///
/// New storages may be added; a reader that doesn't know one leaves the
/// attribute as it is (it passes through edits unchanged) and shows it as
/// unreadable.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Storage {
    /// Single-precision reals: colors, normals, texture coordinates.
    F32,
    /// Double-precision reals: rest positions, measurements.
    F64,
    /// 32-bit whole numbers.
    I32,
    /// 64-bit whole numbers: ids, exact at any size.
    I64,
    /// Indexed text ([`Text`](crate::Text)): always width 1.
    Text,
}

impl Storage {
    /// Every storage, in order.
    pub const ALL: [Storage; 5] = [
        Storage::F32,
        Storage::F64,
        Storage::I32,
        Storage::I64,
        Storage::Text,
    ];

    /// Its name in messages and files: `f32`, `f64`, `i32`, `i64`,
    /// `text`. Never renamed.
    pub fn name(self) -> &'static str {
        match self {
            Storage::F32 => "f32",
            Storage::F64 => "f64",
            Storage::I32 => "i32",
            Storage::I64 => "i64",
            Storage::Text => "text",
        }
    }

    /// The storage named `name` ([`Storage::name`]).
    pub fn from_name(name: &str) -> Option<Storage> {
        Storage::ALL.into_iter().find(|s| s.name() == name)
    }

    /// Whether it holds reals.
    pub fn is_real(self) -> bool {
        matches!(self, Storage::F32 | Storage::F64)
    }

    /// Whether it holds whole numbers.
    pub fn is_integer(self) -> bool {
        matches!(self, Storage::I32 | Storage::I64)
    }
}

/// What an attribute's values mean, so tools transform them right (a
/// point moves, a normal turns, a color stays). New meanings may be added;
/// a reader that doesn't know one treats the values as
/// [`Meaning::None`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Meaning {
    /// Plain numbers (or text), of any width.
    #[default]
    None,
    /// Positions (x y z): transformed as points.
    Point,
    /// Directions with a length (x y z): transformed as vectors.
    Vector,
    /// Surface normals (x y z): transformed by the inverse transpose.
    Normal,
    /// Colors (r g b, or r g b a): scene-linear in the project's working
    /// space, never transformed.
    Color,
    /// Texture coordinates (u v, or u v w): never transformed.
    TextureCoord,
    /// Rotations as unit quaternions (x y z w): composed with a
    /// transform's rotation.
    Rotation,
    /// Transforms as row-major matrices, 3 × 3 (linear) or 4 × 4
    /// (affine, acting on column vectors): composed with a transform.
    Transform,
}

impl Meaning {
    /// Every meaning, in order.
    pub const ALL: [Meaning; 8] = [
        Meaning::None,
        Meaning::Point,
        Meaning::Vector,
        Meaning::Normal,
        Meaning::Color,
        Meaning::TextureCoord,
        Meaning::Rotation,
        Meaning::Transform,
    ];

    /// Its name in messages and files: `none`, `point`, `vector`,
    /// `normal`, `color`, `texture_coord`, `rotation`, `transform`. Never
    /// renamed.
    pub fn name(self) -> &'static str {
        match self {
            Meaning::None => "none",
            Meaning::Point => "point",
            Meaning::Vector => "vector",
            Meaning::Normal => "normal",
            Meaning::Color => "color",
            Meaning::TextureCoord => "texture_coord",
            Meaning::Rotation => "rotation",
            Meaning::Transform => "transform",
        }
    }

    /// The meaning named `name` ([`Meaning::name`]).
    pub fn from_name(name: &str) -> Option<Meaning> {
        Meaning::ALL.into_iter().find(|m| m.name() == name)
    }

    /// The widths values of this meaning have: any for
    /// [`None`](Meaning::None).
    pub fn widths(self) -> &'static [u8] {
        match self {
            Meaning::None => &[],
            Meaning::Point | Meaning::Vector | Meaning::Normal => &[3],
            Meaning::Color => &[3, 4],
            Meaning::TextureCoord => &[2, 3],
            Meaning::Rotation => &[4],
            Meaning::Transform => &[9, 16],
        }
    }
}

/// An attribute's type: how its values are stored, how many make one
/// item, what they mean, and whether each element holds an array of items
/// (any length, empty too) instead of exactly one.
///
/// **Rules** ([`check`](Self::check)):
/// - `width` is 1 to 255;
/// - text has width 1 and no meaning;
/// - a meaning other than [`Meaning::None`] is on reals, at one of its
///   [`widths`](Meaning::widths): Point, Vector and Normal 3, Color 3 or 4,
///   TextureCoord 2 or 3, Rotation 4, Transform 9 or 16.
///
/// ```
/// use kinogaki_geometry::{AttributeType, Meaning, Storage};
///
/// let color = AttributeType::new(Storage::F32, 3, Meaning::Color);
/// assert!(color.check().is_ok());
/// assert!(AttributeType::new(Storage::F32, 2, Meaning::Color).check().is_err());
/// assert!(AttributeType::new(Storage::I32, 3, Meaning::Normal).check().is_err());
/// let weights = AttributeType { array: true, ..AttributeType::new(Storage::F64, 1, Meaning::None) };
/// assert!(weights.check().is_ok());
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AttributeType {
    /// How the values are stored.
    pub storage: Storage,
    /// Values per item, 1 to 255: 3 for a vector, 16 for a 4 × 4 matrix.
    pub width: u8,
    /// What the values mean.
    pub meaning: Meaning,
    /// Each element holds an array of items rather than one item.
    pub array: bool,
}

impl AttributeType {
    /// One item an element of `width` values in `storage` meaning
    /// `meaning` (not checked: see [`check`](Self::check)).
    pub const fn new(storage: Storage, width: u8, meaning: Meaning) -> Self {
        Self {
            storage,
            width,
            meaning,
            array: false,
        }
    }

    /// Indexed text, one string an element.
    pub const TEXT: AttributeType = AttributeType::new(Storage::Text, 1, Meaning::None);

    /// Checks the rules.
    ///
    /// # Errors
    /// The rule it breaks.
    pub fn check(self) -> Result<(), GeometryError> {
        let refuse = |why: String| Err(GeometryError::whole(ErrorKind::Attribute, why));
        if self.width == 0 {
            return refuse("an attribute has at least one value an item".into());
        }
        if self.storage == Storage::Text && (self.width != 1 || self.meaning != Meaning::None) {
            return refuse("text attributes have width 1 and no meaning".into());
        }
        let widths = self.meaning.widths();
        if self.meaning != Meaning::None {
            if !self.storage.is_real() {
                return refuse(format!(
                    "{} attributes hold reals, not {}",
                    self.meaning.name(),
                    self.storage.name()
                ));
            }
            if !widths.contains(&self.width) {
                return refuse(format!(
                    "{} attributes have width {}, not {}",
                    self.meaning.name(),
                    widths
                        .iter()
                        .map(u8::to_string)
                        .collect::<Vec<_>>()
                        .join(" or "),
                    self.width
                ));
            }
        }
        Ok(())
    }

    /// How it reads in messages: `f32 × 3 color`, `i64`, `f64[]`.
    pub fn describe(self) -> String {
        let mut out = self.storage.name().to_owned();
        if self.width > 1 {
            out.push_str(&format!(" × {}", self.width));
        }
        if self.array {
            out.push_str("[]");
        }
        if self.meaning != Meaning::None {
            out.push(' ');
            out.push_str(self.meaning.name());
        }
        out
    }
}
