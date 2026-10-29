//! Line classes: a packet's line segments in named groups (edges, hulls,
//! curves, isoparm grids, sketch strokes…), each with its own look and its
//! own id space.
//!
//! A class owns a contiguous run of the packet's segments
//! ([`SurfaceSource::lines`](crate::SurfaceSource::lines), two
//! [`OverlayVertex`](crate::OverlayVertex) records a segment) and the ids
//! `first_id..first_id + ids`. Each segment's `component` is the class's
//! `first_id` plus the id of the line it belongs to within the class
//! (several segments share one id when a line is a polyline, a B-rep
//! edge's samples), so every class's ids sit in one packet-wide space:
//! one edge selection ([`ElementSet`](crate::ElementSet) of
//! [`ElementKind::Edge`](crate::ElementKind::Edge)) can hold ids of several
//! classes, and a pick turns a packet id back into (class, id) with
//! [`LineClass::local`].
use std::{ops::Range, sync::Arc};

/// A class's id, unique within its packet and below [`LineClassId::LIMIT`]
/// (display options hide classes by a bit mask).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct LineClassId(pub u8);
impl LineClassId {
    /// The most classes a packet has: ids are below this.
    pub const LIMIT: u8 = 32;
}

/// One state's look: a color (RGBA as the target stores it, 0–1) and a
/// width in points.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineLook {
    /// Linear RGBA.
    pub color: [f32; 4],
    /// Width in logical pixels.
    pub width: f32,
}
impl LineLook {
    /// A look of `color` and `width`.
    pub const fn new(color: [f32; 4], width: f32) -> Self {
        Self { color, width }
    }
}

/// How a class draws. A segment shows the first state that applies:
/// selected, hovered, previewed (what a second click would select, e.g. the
/// rest of an edge loop), then plain. Selected lines are wider than hovered
/// ones ([`LineStyle::valid`]).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LineStyle {
    /// Unselected, not hovered.
    pub plain: LineLook,
    /// Under the pointer.
    pub hover: LineLook,
    /// In a tool's preview (a loop or ring about to be picked).
    pub preview: LineLook,
    /// Selected; wider than `hover`.
    pub selected: LineLook,
    /// Dash and gap lengths in points for plain segments (selected,
    /// hovered and previewed ones draw solid); none: solid.
    pub dash: Option<[f32; 2]>,
    /// Clip depth (over w) the lines are drawn nearer than their surface.
    pub depth_bias: f32,
    /// How far behind a surface, in pixels of depth at the line, a point of
    /// the line still counts as visible for picking: chords of a curved
    /// face's isoparms dip under its tessellation, lines on a surface sit
    /// in it.
    pub occlusion: f32,
    /// A wire class (a surface's edges): its plain segments draw in the
    /// view's wire color (dark over shaded faces, light in wireframe)
    /// rather than `plain.color`, follow the wire display toggle and hide
    /// when there are more than one a two pixels of view (dense wires).
    /// Selected, hovered and previewed segments always show.
    pub wire: bool,
}
impl LineStyle {
    /// A surface's edges as the Mesh context draws them (owner round 4,
    /// after Maya's preselection): one point wide in the view's wire color;
    /// the hovered edge two points wide in the accent, the rest of its loop
    /// (what a double click selects) 1.5 points in blue; selected edges three
    /// points wide in the accent, so selected never looks like hovered.
    pub const EDGES: Self = Self {
        plain: LineLook::new([0.16, 0.16, 0.16, 0.85], 1.0),
        hover: LineLook::new(ACCENT, 2.0),
        preview: LineLook::new([0.30, 0.55, 0.95, 1.0], 1.5),
        selected: LineLook::new(ACCENT, 3.0),
        dash: None,
        depth_bias: 0.0,
        occlusion: 1.0,
        wire: true,
    };
}
/// The selection accent the renderer draws selected elements in.
pub const ACCENT: [f32; 4] = [0.906, 0.627, 0.247, 1.0];
impl LineStyle {
    /// Finite non-negative widths and bias, positive dash lengths, and
    /// selected wider than hovered.
    pub fn valid(&self) -> bool {
        let looks = [self.plain, self.hover, self.preview, self.selected];
        looks
            .iter()
            .all(|l| l.width.is_finite() && (0.0..=32.0).contains(&l.width))
            && self.selected.width > self.hover.width
            && self.depth_bias.is_finite()
            && self.depth_bias >= 0.0
            && self.occlusion.is_finite()
            && self.occlusion >= 0.0
            && self
                .dash
                .is_none_or(|d| d.iter().all(|v| v.is_finite() && *v > 0.0))
    }
}

/// A packet's segments `segments` (segment indices, not record indices)
/// drawn with `style`; their components lie in `first_id..first_id + ids`.
#[derive(Clone, Debug, PartialEq)]
pub struct LineClass {
    /// Its id, unique within the packet.
    pub id: LineClassId,
    /// A name for diagnostics.
    pub name: &'static str,
    /// How its segments draw.
    pub style: LineStyle,
    /// The packet segments it owns (segment indices, two line records each).
    pub segments: Range<u32>,
    /// Its first packet-wide line id; `first_id + local` names its line `local`.
    pub first_id: u32,
    /// How many line ids the class has.
    pub ids: u32,
}
impl LineClass {
    /// The packet-wide id of this class's `local` id.
    pub fn global(&self, local: u32) -> u32 {
        self.first_id + local
    }
    /// This class's own id of packet-wide `id`, when it is one of its ids.
    pub fn local(&self, id: u32) -> Option<u32> {
        id.checked_sub(self.first_id).filter(|&l| l < self.ids)
    }
    /// Classes of one packet must cover distinct segments and distinct ids,
    /// with distinct class ids below the limit.
    pub fn validate(classes: &[LineClass], segments: usize) -> Result<(), String> {
        let mut seen = 0u32;
        for (at, c) in classes.iter().enumerate() {
            if c.id.0 >= LineClassId::LIMIT || seen & (1 << c.id.0) != 0 {
                return Err(format!("line class {} has a repeated or large id", c.name));
            }
            seen |= 1 << c.id.0;
            if c.segments.start > c.segments.end || c.segments.end as usize > segments {
                return Err(format!("line class {} exceeds the packet's lines", c.name));
            }
            if !c.style.valid() {
                return Err(format!("line class {} has an invalid style", c.name));
            }
            let ids = c.first_id as u64..c.first_id as u64 + c.ids as u64;
            for other in &classes[at + 1..] {
                let o = other.first_id as u64..other.first_id as u64 + other.ids as u64;
                if c.segments.start < other.segments.end && other.segments.start < c.segments.end
                    || ids.start < o.end && o.start < ids.end
                {
                    return Err(format!(
                        "line classes {} and {} overlap",
                        c.name, other.name
                    ));
                }
            }
        }
        Ok(())
    }
}

/// Line ids (packet-wide) under the pointer and what a second click would
/// add, drawn in their classes' hover and preview looks.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct LineHighlight {
    /// The line id under the pointer.
    pub hovered: Option<u32>,
    /// Sorted, unique.
    pub preview: Arc<[u32]>,
}

#[cfg(test)]
mod tests {
    use super::*;
    fn class(id: u8, segments: Range<u32>, first_id: u32) -> LineClass {
        LineClass {
            id: LineClassId(id),
            name: "test",
            style: LineStyle::EDGES,
            ids: segments.len() as u32,
            segments,
            first_id,
        }
    }
    #[test]
    fn classes_map_ids_both_ways_and_refuse_overlaps_and_bad_styles() {
        let hull = class(3, 4..7, 10);
        assert_eq!(hull.global(2), 12);
        assert_eq!(hull.local(12), Some(2));
        assert_eq!(hull.local(13), None);
        assert_eq!(hull.local(9), None);
        let edges = class(0, 0..4, 0);
        assert!(LineClass::validate(&[edges.clone(), hull.clone()], 7).is_ok());
        assert!(LineClass::validate(&[edges.clone(), hull.clone()], 6).is_err());
        assert!(LineClass::validate(&[edges.clone(), class(1, 3..5, 20)], 7).is_err());
        assert!(LineClass::validate(&[edges.clone(), class(1, 4..6, 2)], 7).is_err());
        assert!(LineClass::validate(&[edges.clone(), class(0, 4..6, 20)], 7).is_err());
        let mut thin = hull;
        thin.style.selected.width = thin.style.hover.width;
        assert!(LineClass::validate(&[edges, thin], 7).is_err());
    }
}
