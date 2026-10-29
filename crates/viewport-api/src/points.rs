//! Point classes: a packet's point markers in named groups (a mesh's
//! points; a CAD body's control vertices and B-rep vertices…), each with
//! its own look, as line classes group lines.
//!
//! A class owns the marker components `first_id..first_id + ids` (a
//! marker's `component` is the id point picks and point selections use),
//! so classes sit in one packet-wide id space and a pick turns a packet id
//! back into (class, id) with [`PointClass::local`]. A packet without
//! classes ([`SurfaceSource::point_classes`](crate::SurfaceSource::point_classes)
//! empty) draws every marker in [`PointStyle::POINTS`], the Mesh context's
//! look.
use crate::ACCENT;

/// A class's id, unique within its packet and below
/// [`PointClassId::LIMIT`] (display options hide classes by a bit mask).
/// [`PointClassId::POINTS`] is the plain points' class.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PointClassId(pub u8);
impl PointClassId {
    /// The most classes a packet has: ids are below this.
    pub const LIMIT: u8 = 8;
    /// Plain points: a packet without classes, or a merged part's.
    pub const POINTS: Self = Self(0);
}

/// A marker's outline.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointShape {
    /// A square.
    Square,
    /// A circle.
    Circle,
    /// A diamond.
    Diamond,
}
impl PointShape {
    /// The shaders' code.
    pub fn code(self) -> u32 {
        match self {
            Self::Square => 0,
            Self::Circle => 1,
            Self::Diamond => 2,
        }
    }
}

/// One state's look: a color (RGBA as the target stores it, 0–1) and a
/// size across in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointLook {
    /// Linear RGBA.
    pub color: [f32; 4],
    /// Size in logical pixels.
    pub size: f32,
}
impl PointLook {
    /// A look of `color` and `size`.
    pub const fn new(color: [f32; 4], size: f32) -> Self {
        Self { color, size }
    }
}

/// How a class draws. A marker shows the first state that applies:
/// selected, hovered, then plain.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointStyle {
    /// Its shape.
    pub shape: PointShape,
    /// Unselected, not hovered.
    pub plain: PointLook,
    /// Under the pointer.
    pub hover: PointLook,
    /// Selected.
    pub selected: PointLook,
    /// How far behind a surface, in pixels of depth at the point, a marker
    /// still counts as visible for picking (a CV sits on its surface).
    pub occlusion: f32,
}
impl PointStyle {
    /// The Mesh context's points: dark squares five points across, selected
    /// ones seven across in the accent.
    pub const POINTS: Self = Self {
        shape: PointShape::Square,
        plain: PointLook::new([0.15, 0.15, 0.15, 1.0], 5.0),
        hover: PointLook::new([0.15, 0.15, 0.15, 1.0], 5.0),
        selected: PointLook::new(ACCENT, 7.0),
        occlusion: 1.0,
    };

    /// Finite sizes from 0 to 32 points and a non-negative occlusion.
    pub fn valid(&self) -> bool {
        [self.plain, self.hover, self.selected]
            .iter()
            .all(|l| l.size.is_finite() && (0.0..=32.0).contains(&l.size))
            && self.occlusion.is_finite()
            && self.occlusion >= 0.0
    }
}

/// A packet's markers whose components lie in `first_id..first_id + ids`,
/// drawn with `style`.
#[derive(Clone, Debug, PartialEq)]
pub struct PointClass {
    /// Its id, unique within the packet.
    pub id: PointClassId,
    /// A name for diagnostics.
    pub name: &'static str,
    /// How its markers draw.
    pub style: PointStyle,
    /// Its first packet-wide point id; `first_id + local` names its point `local`.
    pub first_id: u32,
    /// How many point ids the class has.
    pub ids: u32,
}
impl PointClass {
    /// Every marker of a packet without classes.
    pub fn points(ids: u32) -> Self {
        Self {
            id: PointClassId::POINTS,
            name: "points",
            style: PointStyle::POINTS,
            first_id: 0,
            ids,
        }
    }
    /// The packet-wide id of this class's `local` id.
    pub fn global(&self, local: u32) -> u32 {
        self.first_id + local
    }
    /// This class's own id of packet-wide `id`, when it is one of its ids.
    pub fn local(&self, id: u32) -> Option<u32> {
        id.checked_sub(self.first_id).filter(|&l| l < self.ids)
    }
    /// The class of packet-wide id `id` among `classes`.
    pub fn of(classes: &[PointClass], id: u32) -> Option<&PointClass> {
        classes.iter().find(|c| c.local(id).is_some())
    }
    /// Classes of one packet must hold distinct ids, with distinct class
    /// ids below the limit and valid styles.
    pub fn validate(classes: &[PointClass]) -> Result<(), String> {
        let mut seen = 0u32;
        for (at, c) in classes.iter().enumerate() {
            if c.id.0 >= PointClassId::LIMIT || seen & (1 << c.id.0) != 0 {
                return Err(format!("point class {} has a repeated or large id", c.name));
            }
            seen |= 1 << c.id.0;
            if !c.style.valid() {
                return Err(format!("point class {} has an invalid style", c.name));
            }
            let ids = c.first_id as u64..c.first_id as u64 + c.ids as u64;
            for other in &classes[at + 1..] {
                let o = other.first_id as u64..other.first_id as u64 + other.ids as u64;
                if ids.start < o.end && o.start < ids.end {
                    return Err(format!(
                        "point classes {} and {} overlap",
                        c.name, other.name
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn class(id: u8, first_id: u32, ids: u32) -> PointClass {
        PointClass {
            id: PointClassId(id),
            name: "c",
            style: PointStyle::POINTS,
            first_id,
            ids,
        }
    }

    #[test]
    fn classes_hold_distinct_ids_and_map_both_ways() {
        let cvs = class(1, 0, 4);
        let vertices = class(2, 4, 3);
        assert!(PointClass::validate(&[cvs.clone(), vertices.clone()]).is_ok());
        assert!(PointClass::validate(&[cvs.clone(), class(2, 3, 3)]).is_err());
        assert!(PointClass::validate(&[cvs.clone(), class(1, 4, 3)]).is_err());
        assert!(PointClass::validate(&[class(8, 0, 1)]).is_err());
        assert_eq!(vertices.local(5), Some(1));
        assert_eq!(vertices.global(1), 5);
        assert_eq!(vertices.local(3), None);
        let classes = [cvs, vertices];
        assert_eq!(
            PointClass::of(&classes, 6).map(|c| c.id),
            Some(PointClassId(2))
        );
        assert!(PointClass::of(&classes, 7).is_none());
        let mut bad = PointStyle::POINTS;
        bad.plain.size = f32::NAN;
        assert!(!bad.valid());
    }
}
