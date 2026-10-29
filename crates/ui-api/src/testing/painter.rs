//! [`RecordingPainter`]: a [`Painter`] that records what it is asked to
//! draw.
use crate::color::Color;
use crate::geom::{Align2, Pos, Rect, Vec2};
use crate::paint::{
    Circle, Painter, RectShape, Segment, Shape, Slot, Stroke, StrokeKind, TextShape,
};
use crate::text::Font;
use std::cell::RefCell;
use std::rc::Rc;

/// Every glyph's advance in the recording painter's made-up face, as a
/// fraction of the font size; a line is one font size tall.
pub const GLYPH_ADVANCE: f32 = 0.5;

/// One recorded draw: the clip it was drawn under and its shapes (one
/// for a draw; what fills a [reserved](Painter::reserve) place, none
/// until it is filled).
type Entry = (Rect, Vec<Shape>);

/// A headless [`Painter`] for tests: every call is recorded as [`Shape`]s
/// in draw order, nothing is drawn. Text is measured in a made-up face
/// whose glyphs are all [`GLYPH_ADVANCE`] of the size wide, so layouts are
/// deterministic.
///
/// Clones and [`clipped`](Painter::clipped) painters share one record.
#[derive(Clone, Debug)]
pub struct RecordingPainter {
    log: Rc<RefCell<Vec<Entry>>>,
    clip: Rect,
}

impl RecordingPainter {
    /// A painter clipped to `clip`, with an empty record.
    pub fn new(clip: Rect) -> Self {
        RecordingPainter {
            log: Rc::default(),
            clip,
        }
    }

    /// The shapes drawn so far, in order (unfilled reserved places left
    /// out).
    pub fn shapes(&self) -> Vec<Shape> {
        self.log
            .borrow()
            .iter()
            .flat_map(|(_, s)| s.iter().cloned())
            .collect()
    }

    /// The shapes drawn so far with the clip each was drawn under.
    pub fn painted(&self) -> Vec<(Rect, Shape)> {
        self.log
            .borrow()
            .iter()
            .flat_map(|(clip, s)| s.iter().map(|s| (*clip, s.clone())))
            .collect()
    }

    /// Forget everything recorded.
    pub fn clear(&self) {
        self.log.borrow_mut().clear();
    }

    fn push(&self, shape: Shape) {
        self.log.borrow_mut().push((self.clip, vec![shape]));
    }

    /// `text`'s size on one line in `font`, in the made-up face.
    pub fn measure(text: &str, font: Font) -> Vec2 {
        Vec2::new(
            text.chars().count() as f32 * font.size * GLYPH_ADVANCE,
            font.size,
        )
    }

    /// `text`'s size in `font` wrapped to lines at most `width` wide, in
    /// the made-up face (a line breaks at any glyph; at least one a line).
    pub fn measure_wrapped(text: &str, font: Font, width: f32) -> Vec2 {
        let advance = font.size * GLYPH_ADVANCE;
        let glyphs = text.chars().count();
        let per_line = ((width / advance).floor() as usize).max(1);
        let lines = glyphs.div_ceil(per_line).max(1);
        Vec2::new(
            glyphs.min(per_line) as f32 * advance,
            lines as f32 * font.size,
        )
    }
}

impl Painter for RecordingPainter {
    fn clip(&self) -> Rect {
        self.clip
    }

    fn clipped(&self, rect: Rect) -> Box<dyn Painter + '_> {
        Box::new(RecordingPainter {
            log: self.log.clone(),
            clip: self.clip.intersect(rect),
        })
    }

    fn segment(&self, a: Pos, b: Pos, stroke: Stroke) {
        self.push(Shape::segment(a, b, stroke));
    }

    fn circle(&self, center: Pos, radius: f32, fill: Color, stroke: Stroke) {
        self.push(Shape::Circle(Circle {
            center,
            radius,
            fill,
            stroke,
        }));
    }

    fn rect(&self, rect: Rect, radius: f32, fill: Color, stroke: Stroke, kind: StrokeKind) {
        self.push(Shape::Rect(RectShape {
            rect,
            radius,
            fill,
            stroke,
            kind,
        }));
    }

    fn polyline(&self, points: &[Pos], closed: bool, stroke: Stroke) {
        self.push(Shape::Polyline {
            points: points.to_vec(),
            closed,
            stroke,
        });
    }

    fn text(&self, at: Pos, anchor: Align2, text: &str, font: Font, color: Color) -> Rect {
        self.push(Shape::text(at, anchor, text, font, color));
        anchor.anchor_size(at, Self::measure(text, font))
    }

    fn wrapped_text(
        &self,
        at: Pos,
        anchor: Align2,
        text: &str,
        font: Font,
        color: Color,
        width: f32,
    ) -> Rect {
        let rect = anchor.anchor_size(at, Self::measure_wrapped(text, font, width));
        self.push(Shape::text(rect.min, Align2::LEFT_TOP, text, font, color));
        rect
    }

    fn icon(&self, rect: Rect, name: &str, tint: Color) {
        self.push(Shape::icon(rect, name, tint));
    }

    fn segments(&self, segments: &[Segment]) {
        for s in segments {
            self.push(Shape::Segment(*s));
        }
    }

    fn circles(&self, circles: &[Circle]) {
        for c in circles {
            self.push(Shape::Circle(*c));
        }
    }

    fn rects(&self, rects: &[RectShape]) {
        for r in rects {
            self.push(Shape::Rect(*r));
        }
    }

    fn texts(&self, texts: &[TextShape]) {
        for t in texts {
            self.push(Shape::Text(t.clone()));
        }
    }

    fn shapes(&self, shapes: &[Shape]) {
        for s in shapes {
            self.push(s.clone());
        }
    }

    fn text_size(&self, text: &str, font: Font) -> Vec2 {
        Self::measure(text, font)
    }

    fn wrapped_text_size(&self, text: &str, font: Font, width: f32) -> Vec2 {
        Self::measure_wrapped(text, font, width)
    }

    fn glyph_width(&self, _glyph: char, font: Font) -> f32 {
        font.size * GLYPH_ADVANCE
    }

    fn reserve(&self) -> Slot {
        let mut log = self.log.borrow_mut();
        log.push((self.clip, Vec::new()));
        Slot((log.len() - 1) as u32)
    }

    fn fill(&self, slot: Slot, shape: Shape) {
        self.fill_many(slot, vec![shape]);
    }

    fn fill_many(&self, slot: Slot, shapes: Vec<Shape>) {
        if let Some(entry) = self.log.borrow_mut().get_mut(slot.0 as usize) {
            entry.1 = shapes;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::geom::{pos, vec2};

    #[test]
    fn draws_are_recorded_in_order_with_their_clip() {
        let window = Rect::from_min_size(Pos::ZERO, vec2(100.0, 100.0));
        let painter = RecordingPainter::new(window);
        let backdrop = painter.reserve();
        let text = painter.text(
            pos(10.0, 10.0),
            Align2::LEFT_TOP,
            "abcd",
            Font::ui(10.0),
            Color::WHITE,
        );
        assert_eq!(text, Rect::from_min_size(pos(10.0, 10.0), vec2(20.0, 10.0)));
        painter.fill(
            backdrop,
            Shape::rect_filled(text.expand(2.0), 3.0, Color::BLACK),
        );
        let inner = Rect::from_min_size(pos(50.0, 50.0), vec2(100.0, 100.0));
        painter
            .clipped(inner)
            .circle_filled(pos(60.0, 60.0), 4.0, Color::WHITE);
        let painted = painter.painted();
        assert_eq!(painted.len(), 3);
        assert!(
            matches!(painted[0].1, Shape::Rect(_)),
            "the reserved place comes first"
        );
        assert!(matches!(painted[1].1, Shape::Text(_)));
        assert_eq!(
            painted[2].0,
            Rect::from_min_size(pos(50.0, 50.0), vec2(50.0, 50.0))
        );
        painter.clear();
        assert!(painter.shapes().is_empty());
    }

    #[test]
    fn a_reserved_place_holds_many_shapes_and_text_wraps() {
        let window = Rect::from_min_size(Pos::ZERO, vec2(100.0, 100.0));
        let painter = RecordingPainter::new(window);
        let labels = painter.reserve();
        painter.segment(
            pos(0.0, 0.0),
            pos(10.0, 10.0),
            Stroke::new(1.0, Color::WHITE),
        );
        let font = Font::ui(10.0);
        let label = |i: u32| {
            Shape::haloed_text(
                pos(5.0, 5.0),
                Align2::LEFT_BOTTOM,
                i.to_string(),
                font,
                Color::WHITE,
                Color::BLACK,
            )
        };
        painter.fill_many(labels, vec![label(1), label(2)]);
        let shapes = painter.shapes();
        assert_eq!(shapes.len(), 3);
        assert!(matches!(&shapes[1], Shape::Haloed { text, .. } if text.text == "2"));
        assert!(
            matches!(shapes[2], Shape::Segment(_)),
            "drawn after, over them"
        );
        let size = painter.wrapped_text_size("abcdefghij", font, 22.0);
        assert_eq!(size, vec2(20.0, 30.0), "four glyphs a line, three lines");
    }
}
