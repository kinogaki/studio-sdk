//! What changed about a prim: a small flag set, a subset of Hydra's
//! HdChangeTracker bits.

use std::fmt;
use std::ops::{BitOr, BitOrAssign};

/// Changed parts of a prim. Bits OR together when updates coalesce.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Dirty(u16);

impl Dirty {
    /// Nothing changed.
    pub const NONE: Dirty = Dirty(0);
    /// A shape's point positions (its connectivity stands), at the
    /// [`Change`](crate::Change)'s ranges.
    pub const POINTS: Dirty = Dirty(1 << 0);
    /// A shape's connectivity, and with it every array sized by it.
    pub const TOPOLOGY: Dirty = Dirty(1 << 1);
    /// A shape's or Instancer's primvars. Compare by name and column
    /// ([`Column::ptr_eq`](crate::geometry::Column::ptr_eq)): an unchanged
    /// primvar is the same column.
    pub const PRIMVARS: Dirty = Dirty(1 << 2);
    /// Which materials a shape's primitives, or an Instancer's instances,
    /// render with.
    pub const MATERIALS: Dirty = Dirty(1 << 3);
    /// A placement (a shape's, a Camera's or a Light's, an Instancer's or its
    /// instances'), or its samples across the shutter.
    pub const TRANSFORM: Dirty = Dirty(1 << 4);
    /// Whether a prim is rendered, and what sees it.
    pub const VISIBILITY: Dirty = Dirty(1 << 5);
    /// Values of a Material, Light or Camera (a slider), or a prim's name.
    pub const PARAMS: Dirty = Dirty(1 << 6);
    /// A Material's nodes or links.
    pub const NETWORK: Dirty = Dirty(1 << 7);
    /// How many bits there are.
    pub(crate) const COUNT: usize = 8;
    /// Every bit.
    pub const ALL: Dirty = Dirty((1 << Dirty::COUNT) - 1);

    const NAMES: [&'static str; Dirty::COUNT] = [
        "POINTS",
        "TOPOLOGY",
        "PRIMVARS",
        "MATERIALS",
        "TRANSFORM",
        "VISIBILITY",
        "PARAMS",
        "NETWORK",
    ];

    /// The raw bits.
    pub const fn bits(self) -> u16 {
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
