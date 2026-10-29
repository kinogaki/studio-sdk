//! The [`Painter`]: a plugin's overlays (handles, guides, badges, labels)
//! as a few primitives the host draws, and [`Shape`]s, the same primitives
//! as data for batches and reserved slots.
//!
//! Meshes, surfaces and wireframes never go through the painter: they are
//! the data types' display surfaces, drawn by the host's viewport. The
//! painter is for the few hundred marks a tool puts over them; draw many
//! of one kind with one batch call ([`Painter::segments`] and friends).
use crate::color::Color;
use crate::geom::{Align2, Pos, Rect, Vec2};
use crate::text::Font;

/// A line's width (points) and color.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Stroke {
    /// The width, in points.
    pub width: f32,
    /// The color.
    pub color: Color,
}

impl Stroke {
    /// No line.
    pub const NONE: Stroke = Stroke {
        width: 0.0,
        color: Color::TRANSPARENT,
    };

    /// A `width`-point line in `color`.
    pub const fn new(width: f32, color: Color) -> Self {
        Stroke { width, color }
    }

    /// Whether it draws nothing.
    pub fn is_empty(&self) -> bool {
        self.width <= 0.0 || self.color == Color::TRANSPARENT
    }
}

impl From<(f32, Color)> for Stroke {
    fn from((width, color): (f32, Color)) -> Self {
        Stroke { width, color }
    }
}

/// Where a closed outline sits relative to its shape's edge.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum StrokeKind {
    /// Inside the edge: the outline stays within the rectangle.
    #[default]
    Inside,
    /// Centred on the edge.
    Middle,
    /// Outside the edge.
    Outside,
}

/// A place in the painter's draw order kept for a shape known later
/// ([`Painter::reserve`]): a backdrop sized to text drawn after it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Slot(pub u32);

/// A straight line from `points[0]` to `points[1]`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Segment {
    /// Its ends.
    pub points: [Pos; 2],
    /// Its line.
    pub stroke: Stroke,
}

/// A circle, filled, outlined or both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Circle {
    /// Its middle.
    pub center: Pos,
    /// Its radius, in points.
    pub radius: f32,
    /// Its fill ([`Color::TRANSPARENT`] for none).
    pub fill: Color,
    /// Its outline, centred on the edge ([`Stroke::NONE`] for none).
    pub stroke: Stroke,
}

/// A rectangle with rounded corners, filled, outlined or both.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RectShape {
    /// Where.
    pub rect: Rect,
    /// The corners' radius, in points.
    pub radius: f32,
    /// Its fill ([`Color::TRANSPARENT`] for none).
    pub fill: Color,
    /// Its outline ([`Stroke::NONE`] for none).
    pub stroke: Stroke,
    /// Where the outline sits.
    pub kind: StrokeKind,
}

/// Text on one line.
#[derive(Clone, Debug, PartialEq)]
pub struct TextShape {
    /// Where its `anchor` point goes.
    pub at: Pos,
    /// Which of its points sits at `at`.
    pub anchor: Align2,
    /// The text.
    pub text: String,
    /// Its font.
    pub font: Font,
    /// Its color.
    pub color: Color,
}

/// An icon from the host's icon set, tinted.
#[derive(Clone, Debug, PartialEq)]
pub struct IconShape {
    /// Where (the icon fits it).
    pub rect: Rect,
    /// The icon's name in the host's icon set (`"sketch-finish"`); an
    /// unknown name draws nothing.
    pub name: String,
    /// The color its ink takes.
    pub tint: Color,
}

/// One primitive as data: for [`Painter::shapes`] and [`Painter::fill`].
#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    /// A straight line.
    Segment(Segment),
    /// Connected straight lines.
    Polyline {
        /// The corners, in order.
        points: Vec<Pos>,
        /// Whether the last corner joins the first.
        closed: bool,
        /// The line.
        stroke: Stroke,
    },
    /// Connected lines drawn in dashes: `dash` points on, `gap` off,
    /// measured along the whole path.
    Dashed {
        /// The corners, in order.
        points: Vec<Pos>,
        /// The line.
        stroke: Stroke,
        /// Each dash's length.
        dash: f32,
        /// Each gap's length.
        gap: f32,
    },
    /// Connected lines drawn as dots every `spacing` points.
    Dotted {
        /// The corners, in order.
        points: Vec<Pos>,
        /// The dots' color.
        color: Color,
        /// The distance between dots.
        spacing: f32,
        /// Each dot's radius.
        radius: f32,
    },
    /// A circle.
    Circle(Circle),
    /// A rectangle.
    Rect(RectShape),
    /// A convex polygon (an arrow head, a plane handle), filled and
    /// outlined.
    ConvexPolygon {
        /// The corners, in order around it.
        points: Vec<Pos>,
        /// Its fill.
        fill: Color,
        /// Its outline.
        stroke: Stroke,
    },
    /// Text.
    Text(TextShape),
    /// Text over a one-point outline in `halo` (four copies of it in that
    /// color, a point off each diagonal), readable over light and dark
    /// alike: labels over a model.
    Haloed {
        /// The text.
        text: TextShape,
        /// The outline's color.
        halo: Color,
    },
    /// An icon.
    Icon(IconShape),
}

impl Shape {
    /// A straight line from `a` to `b`.
    pub fn segment(a: Pos, b: Pos, stroke: impl Into<Stroke>) -> Self {
        Shape::Segment(Segment {
            points: [a, b],
            stroke: stroke.into(),
        })
    }

    /// Open connected lines through `points`.
    pub fn line(points: Vec<Pos>, stroke: impl Into<Stroke>) -> Self {
        Shape::Polyline {
            points,
            closed: false,
            stroke: stroke.into(),
        }
    }

    /// A closed outline through `points`.
    pub fn closed_line(points: Vec<Pos>, stroke: impl Into<Stroke>) -> Self {
        Shape::Polyline {
            points,
            closed: true,
            stroke: stroke.into(),
        }
    }

    /// A filled circle.
    pub fn circle_filled(center: Pos, radius: f32, fill: Color) -> Self {
        Shape::Circle(Circle {
            center,
            radius,
            fill,
            stroke: Stroke::NONE,
        })
    }

    /// A circle's outline.
    pub fn circle_stroke(center: Pos, radius: f32, stroke: impl Into<Stroke>) -> Self {
        Shape::Circle(Circle {
            center,
            radius,
            fill: Color::TRANSPARENT,
            stroke: stroke.into(),
        })
    }

    /// A filled rectangle with corners of `radius`.
    pub fn rect_filled(rect: Rect, radius: f32, fill: Color) -> Self {
        Shape::Rect(RectShape {
            rect,
            radius,
            fill,
            stroke: Stroke::NONE,
            kind: StrokeKind::Inside,
        })
    }

    /// A rectangle's outline, sitting `kind` of its edge.
    pub fn rect_stroke(
        rect: Rect,
        radius: f32,
        stroke: impl Into<Stroke>,
        kind: StrokeKind,
    ) -> Self {
        Shape::Rect(RectShape {
            rect,
            radius,
            fill: Color::TRANSPARENT,
            stroke: stroke.into(),
            kind,
        })
    }

    /// A filled and outlined convex polygon.
    pub fn convex_polygon(points: Vec<Pos>, fill: Color, stroke: impl Into<Stroke>) -> Self {
        Shape::ConvexPolygon {
            points,
            fill,
            stroke: stroke.into(),
        }
    }

    /// `text` with its `anchor` point at `at`.
    pub fn text(
        at: Pos,
        anchor: Align2,
        text: impl Into<String>,
        font: Font,
        color: Color,
    ) -> Self {
        Shape::Text(TextShape {
            at,
            anchor,
            text: text.into(),
            font,
            color,
        })
    }

    /// `text` (as [`Shape::text`] places it) over a one-point outline in
    /// `halo`.
    pub fn haloed_text(
        at: Pos,
        anchor: Align2,
        text: impl Into<String>,
        font: Font,
        color: Color,
        halo: Color,
    ) -> Self {
        Shape::Haloed {
            text: TextShape {
                at,
                anchor,
                text: text.into(),
                font,
                color,
            },
            halo,
        }
    }

    /// The icon `name` fitted to `rect`, tinted `tint`.
    pub fn icon(rect: Rect, name: impl Into<String>, tint: Color) -> Self {
        Shape::Icon(IconShape {
            rect,
            name: name.into(),
            tint,
        })
    }
}

/// Draws into one layer of the host's window, clipped to a rectangle.
///
/// The host hands one out per call ([`Ui::painter`](crate::Ui::painter));
/// it draws in the order called, over what the `Ui` drew before. Every
/// method is a request: the host draws it this frame, and nothing persists
/// to the next. Coordinates are screen points.
///
/// Implementations: Studio's (over its UI toolkit) and
/// [`RecordingPainter`](crate::testing::RecordingPainter), which records
/// for tests.
pub trait Painter {
    /// The rectangle drawing is clipped to.
    fn clip(&self) -> Rect;

    /// A painter into the same layer clipped to `rect` within this one's
    /// clip.
    fn clipped(&self, rect: Rect) -> Box<dyn Painter + '_>;

    /// A straight line from `a` to `b`.
    fn segment(&self, a: Pos, b: Pos, stroke: Stroke);

    /// A circle, filled and outlined.
    fn circle(&self, center: Pos, radius: f32, fill: Color, stroke: Stroke);

    /// A rectangle with corners of `radius`, filled and outlined `kind` of
    /// its edge.
    fn rect(&self, rect: Rect, radius: f32, fill: Color, stroke: Stroke, kind: StrokeKind);

    /// Connected straight lines through `points`, joined last to first when
    /// `closed`.
    fn polyline(&self, points: &[Pos], closed: bool, stroke: Stroke);

    /// `text` on one line with its `anchor` point at `at`; returns where it
    /// went.
    fn text(&self, at: Pos, anchor: Align2, text: &str, font: Font, color: Color) -> Rect;

    /// `text` wrapped to lines at most `width` wide, with its `anchor`
    /// point at `at`; returns where it went.
    fn wrapped_text(
        &self,
        at: Pos,
        anchor: Align2,
        text: &str,
        font: Font,
        color: Color,
        width: f32,
    ) -> Rect;

    /// The icon `name` from the host's icon set fitted to `rect` and tinted
    /// `tint`; an unknown name draws nothing.
    fn icon(&self, rect: Rect, name: &str, tint: Color);

    /// Many straight lines, in order.
    fn segments(&self, segments: &[Segment]);

    /// Many circles, in order.
    fn circles(&self, circles: &[Circle]);

    /// Many rectangles, in order.
    fn rects(&self, rects: &[RectShape]);

    /// Many texts, in order.
    fn texts(&self, texts: &[TextShape]);

    /// Any shapes, in order.
    fn shapes(&self, shapes: &[Shape]);

    /// How much room `text` takes on one line in `font`.
    fn text_size(&self, text: &str, font: Font) -> Vec2;

    /// How much room `text` takes wrapped to lines at most `width` wide.
    fn wrapped_text_size(&self, text: &str, font: Font, width: f32) -> Vec2;

    /// How wide `glyph` is in `font` (columns of a code face).
    fn glyph_width(&self, glyph: char, font: Font) -> f32;

    /// Keep a place in the draw order here, for a shape given later with
    /// [`Painter::fill`] (an empty place draws nothing).
    fn reserve(&self) -> Slot;

    /// Draw `shape` in the place `slot` kept, this frame.
    fn fill(&self, slot: Slot, shape: Shape);

    /// Draw `shapes`, in order, in the place `slot` kept, this frame: many
    /// marks known only after what is drawn over them (a view's labels
    /// under its tools).
    fn fill_many(&self, slot: Slot, shapes: Vec<Shape>);

    /// A filled circle.
    fn circle_filled(&self, center: Pos, radius: f32, fill: Color) {
        self.circle(center, radius, fill, Stroke::NONE);
    }

    /// A circle's outline.
    fn circle_stroke(&self, center: Pos, radius: f32, stroke: Stroke) {
        self.circle(center, radius, Color::TRANSPARENT, stroke);
    }

    /// A filled rectangle with corners of `radius`.
    fn rect_filled(&self, rect: Rect, radius: f32, fill: Color) {
        self.rect(rect, radius, fill, Stroke::NONE, StrokeKind::Inside);
    }

    /// A rectangle's outline, `kind` of its edge.
    fn rect_stroke(&self, rect: Rect, radius: f32, stroke: Stroke, kind: StrokeKind) {
        self.rect(rect, radius, Color::TRANSPARENT, stroke, kind);
    }
}
