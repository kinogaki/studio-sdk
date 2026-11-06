//! [`Sampling`]: how an image is read where a texture names it, past its edges
//! ([`Wrap`]) and between its texels ([`Filter`]). A material's texture holds
//! one, and the shaded views read the image by it.

/// How a coordinate outside 0–1 reads an image, along one axis.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Wrap {
    /// The image tiles.
    #[default]
    Repeat,
    /// Its edge texels go on.
    Clamp,
    /// It tiles, every other copy mirrored.
    MirrorRepeat,
    /// Nothing: black.
    Black,
}

impl Wrap {
    /// Every mode, in their code order.
    pub const ALL: [Wrap; 4] = [Wrap::Repeat, Wrap::Clamp, Wrap::MirrorRepeat, Wrap::Black];
}

/// How an image is read between its texels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Filter {
    /// Blended from the texels about the point, smoothly.
    #[default]
    Smooth,
    /// The texel the point lies in, as it is: pixel art's.
    Nearest,
}

/// How an image is read: across (u) and up (v), and between texels.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Sampling {
    /// Past its edges, along u and along v.
    pub wrap: [Wrap; 2],
    /// Between its texels.
    pub filter: Filter,
}
