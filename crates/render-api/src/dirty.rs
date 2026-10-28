//! What changed about a prim: a small flag set, a subset of Hydra's
//! HdChangeTracker bits.

use std::fmt;
use std::ops::{BitOr, BitOrAssign};

/// Changed parts of a prim. Bits OR together when updates coalesce.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Dirty(u32);

impl Dirty {
    /// Nothing changed.
    pub const NONE: Dirty = Dirty(0);
    /// Mesh point positions (the triangles stand).
    pub const POINTS: Dirty = Dirty(1 << 0);
    /// Mesh triangles, and with them every per-triangle and per-corner array.
    pub const TOPOLOGY: Dirty = Dirty(1 << 1);
    /// Mesh corner normals.
    pub const NORMALS: Dirty = Dirty(1 << 2);
    /// Mesh corner texture coordinates.
    pub const UVS: Dirty = Dirty(1 << 3);
    /// Mesh per-triangle material slots or the slots' materials.
    pub const MATERIAL_SLOTS: Dirty = Dirty(1 << 4);
    /// A Mesh, Instancer, Camera or Light placement.
    pub const TRANSFORM: Dirty = Dirty(1 << 5);
    /// Whether a Mesh or Light is rendered.
    pub const VISIBILITY: Dirty = Dirty(1 << 6);
    /// Values of a Material, Light or Camera (a slider).
    pub const PARAMS: Dirty = Dirty(1 << 7);
    /// A Material's nodes or links.
    pub const NETWORK: Dirty = Dirty(1 << 8);
    /// A Texture's pixels or size.
    pub const PIXELS: Dirty = Dirty(1 << 9);
    /// Mesh attributes the materials read (names, layout or values).
    pub const ATTRIBUTES: Dirty = Dirty(1 << 10);
    /// How many bits there are.
    pub const COUNT: usize = 11;
    /// Every bit.
    pub const ALL: Dirty = Dirty((1 << Dirty::COUNT) - 1);

    const NAMES: [&'static str; Dirty::COUNT] = [
        "POINTS",
        "TOPOLOGY",
        "NORMALS",
        "UVS",
        "MATERIAL_SLOTS",
        "TRANSFORM",
        "VISIBILITY",
        "PARAMS",
        "NETWORK",
        "PIXELS",
        "ATTRIBUTES",
    ];

    /// The raw bits.
    pub const fn bits(self) -> u32 {
        self.0
    }

    /// Whether every bit of `other` is set.
    pub const fn contains(self, other: Dirty) -> bool {
        self.0 & other.0 == other.0
    }

    /// Whether any bit of `other` is set.
    pub const fn intersects(self, other: Dirty) -> bool {
        self.0 & other.0 != 0
    }

    /// Whether no bit is set.
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// The bits of either (`|` in a const).
    pub const fn union(self, other: Dirty) -> Dirty {
        Dirty(self.0 | other.0)
    }

    /// The bits of `self` not in `other`.
    pub const fn without(self, other: Dirty) -> Dirty {
        Dirty(self.0 & !other.0)
    }
}

impl BitOr for Dirty {
    type Output = Dirty;
    fn bitor(self, other: Dirty) -> Dirty {
        Dirty(self.0 | other.0)
    }
}

impl BitOrAssign for Dirty {
    fn bitor_assign(&mut self, other: Dirty) {
        self.0 |= other.0;
    }
}

impl fmt::Debug for Dirty {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.is_empty() {
            return f.write_str("Dirty(NONE)");
        }
        f.write_str("Dirty(")?;
        let mut first = true;
        for (bit, name) in Dirty::NAMES.iter().enumerate() {
            if self.0 & (1 << bit) != 0 {
                if !first {
                    f.write_str(" | ")?;
                }
                f.write_str(name)?;
                first = false;
            }
        }
        f.write_str(")")
    }
}
