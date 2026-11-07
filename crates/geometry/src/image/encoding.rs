//! [`Encoding`]: what a layer's values mean, and [`ColorTag`]: which colors.
use std::fmt;
use std::sync::Arc;

/// What a layer's values mean.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Encoding {
    /// Colors, as `tag` says.
    Color {
        /// Which colors they are.
        tag: ColorTag,
        /// Whether the colors are premultiplied by their alpha.
        premultiplied: bool,
    },
    /// Data (normals, depth, ids): never converted, shown as they are.
    Raw,
}

impl Encoding {
    /// Colors in `space`, a space of the project's color configuration.
    pub fn color(space: &str, premultiplied: bool) -> Self {
        Self::Color {
            tag: ColorTag::Space(space.into()),
            premultiplied,
        }
    }

    /// Whether its values are colors premultiplied by their alpha (data and
    /// straight colors are not).
    pub fn premultiplied(&self) -> bool {
        matches!(
            self,
            Self::Color {
                premultiplied: true,
                ..
            }
        )
    }

    /// The configuration's space its colors are in, when its tag names one.
    pub fn space(&self) -> Option<&str> {
        match self {
            Self::Color { tag, .. } => tag.space(),
            Self::Raw => None,
        }
    }
}

/// Which colors a layer holds: what a file says of them, until the host
/// resolves the tag to a space of its color configuration. Engines, Comp
/// and the canvas receive [`Space`](Self::Space) alone.
#[derive(Clone, Debug)]
#[non_exhaustive]
pub enum ColorTag {
    /// A space of the configuration, by name.
    Space(Arc<str>),
    /// An ICC profile's bytes.
    Icc(Arc<[u8]>),
    /// Linear colors over these primaries: red, green, blue and white, x
    /// then y each (OpenEXR's `chromaticities`).
    Primaries([f32; 8]),
    /// Nothing said: linear light (a float file's), or encoded (an
    /// integer file's).
    Unknown {
        /// Whether the values are linear light.
        linear: bool,
    },
}

impl ColorTag {
    /// The configuration's space, when it names one.
    pub fn space(&self) -> Option<&str> {
        match self {
            Self::Space(space) => Some(space),
            _ => None,
        }
    }
}

/// Primaries compare and hash by their bits.
impl PartialEq for ColorTag {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Space(a), Self::Space(b)) => a == b,
            (Self::Icc(a), Self::Icc(b)) => a == b,
            (Self::Primaries(a), Self::Primaries(b)) => a.map(f32::to_bits) == b.map(f32::to_bits),
            (Self::Unknown { linear: a }, Self::Unknown { linear: b }) => a == b,
            _ => false,
        }
    }
}

impl Eq for ColorTag {}

impl std::hash::Hash for ColorTag {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        match self {
            Self::Space(space) => space.hash(state),
            Self::Icc(profile) => profile.hash(state),
            Self::Primaries(xy) => xy.map(f32::to_bits).hash(state),
            Self::Unknown { linear } => linear.hash(state),
        }
    }
}

/// What a UI says of it: the space's name, else what kind of tag it is.
impl fmt::Display for ColorTag {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Space(space) => f.write_str(space),
            Self::Icc(_) => f.write_str("ICC profile"),
            Self::Primaries(_) => f.write_str("primaries"),
            Self::Unknown { linear: true } => f.write_str("untagged, linear"),
            Self::Unknown { linear: false } => f.write_str("untagged"),
        }
    }
}
