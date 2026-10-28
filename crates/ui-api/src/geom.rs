//! Screen geometry in points: [`Pos`], [`Vec2`], [`Rect`] and the
//! alignments [`Align`] and [`Align2`].
//!
//! Points are the host's logical pixels, `f32`, with y pointing down. The
//! types and their arithmetic follow egui's `emath` (`Pos2`, `Vec2`, `Rect`,
//! `Align2`) formula for formula, so a host built on egui converts them
//! bit for bit (see `THIRD_PARTY_NOTICES.md`).
use std::ops::{Add, AddAssign, Div, Mul, Neg, Sub, SubAssign};

/// A position on screen, in points from the window's top left.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Pos {
    /// Points to the right.
    pub x: f32,
    /// Points down.
    pub y: f32,
}

/// A displacement or a size, in points.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Vec2 {
    /// Points to the right (or a width).
    pub x: f32,
    /// Points down (or a height).
    pub y: f32,
}

/// The position (`x`, `y`).
pub const fn pos(x: f32, y: f32) -> Pos {
    Pos { x, y }
}

/// The vector (`x`, `y`).
pub const fn vec2(x: f32, y: f32) -> Vec2 {
    Vec2 { x, y }
}

impl Pos {
    /// The window's top left.
    pub const ZERO: Pos = pos(0.0, 0.0);

    /// The position (`x`, `y`).
    pub const fn new(x: f32, y: f32) -> Self {
        pos(x, y)
    }

    /// The vector from the origin to here.
    pub const fn to_vec2(self) -> Vec2 {
        vec2(self.x, self.y)
    }

    /// The distance to `other`.
    pub fn distance(self, other: Pos) -> f32 {
        (self - other).length()
    }

    /// The squared distance to `other`.
    pub fn distance_sq(self, other: Pos) -> f32 {
        (self - other).length_sq()
    }

    /// The smaller of each coordinate.
    pub fn min(self, other: Pos) -> Pos {
        pos(self.x.min(other.x), self.y.min(other.y))
    }

    /// The larger of each coordinate.
    pub fn max(self, other: Pos) -> Pos {
        pos(self.x.max(other.x), self.y.max(other.y))
    }

    /// Whether both coordinates are finite.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }

    /// `t` of the way from here to `other`.
    pub fn lerp(self, other: Pos, t: f32) -> Pos {
        pos(
            self.x * (1.0 - t) + other.x * t,
            self.y * (1.0 - t) + other.y * t,
        )
    }
}

impl Vec2 {
    /// No displacement.
    pub const ZERO: Vec2 = vec2(0.0, 0.0);

    /// The vector (`x`, `y`).
    pub const fn new(x: f32, y: f32) -> Self {
        vec2(x, y)
    }

    /// The vector (`v`, `v`).
    pub const fn splat(v: f32) -> Self {
        vec2(v, v)
    }

    /// The position this vector reaches from the origin.
    pub const fn to_pos(self) -> Pos {
        pos(self.x, self.y)
    }

    /// The length.
    pub fn length(self) -> f32 {
        self.x.hypot(self.y)
    }

    /// The squared length.
    pub fn length_sq(self) -> f32 {
        self.x * self.x + self.y * self.y
    }

    /// This vector at length 1; a zero vector stays zero.
    pub fn normalized(self) -> Vec2 {
        let len = self.length();
        if len <= 0.0 { self } else { self / len }
    }

    /// The dot product with `other`.
    pub fn dot(self, other: Vec2) -> f32 {
        self.x * other.x + self.y * other.y
    }

    /// Turned a quarter clockwise on screen: (y, −x).
    pub fn rot90(self) -> Vec2 {
        vec2(self.y, -self.x)
    }

    /// The angle from the x axis, clockwise on screen, in radians.
    pub fn angle(self) -> f32 {
        self.y.atan2(self.x)
    }

    /// The unit vector at `angle` radians (clockwise on screen from x).
    pub fn angled(angle: f32) -> Vec2 {
        let (sin, cos) = angle.sin_cos();
        vec2(cos, sin)
    }

    /// Each component's magnitude.
    pub fn abs(self) -> Vec2 {
        vec2(self.x.abs(), self.y.abs())
    }

    /// The smaller component.
    pub fn min_elem(self) -> f32 {
        self.x.min(self.y)
    }

    /// The larger component.
    pub fn max_elem(self) -> f32 {
        self.x.max(self.y)
    }

    /// The smaller of each component.
    pub fn min(self, other: Vec2) -> Vec2 {
        vec2(self.x.min(other.x), self.y.min(other.y))
    }

    /// The larger of each component.
    pub fn max(self, other: Vec2) -> Vec2 {
        vec2(self.x.max(other.x), self.y.max(other.y))
    }

    /// Whether both components are finite.
    pub fn is_finite(self) -> bool {
        self.x.is_finite() && self.y.is_finite()
    }
}

impl Sub for Pos {
    type Output = Vec2;
    fn sub(self, other: Pos) -> Vec2 {
        vec2(self.x - other.x, self.y - other.y)
    }
}

impl Add<Vec2> for Pos {
    type Output = Pos;
    fn add(self, v: Vec2) -> Pos {
        pos(self.x + v.x, self.y + v.y)
    }
}

impl Sub<Vec2> for Pos {
    type Output = Pos;
    fn sub(self, v: Vec2) -> Pos {
        pos(self.x - v.x, self.y - v.y)
    }
}

impl AddAssign<Vec2> for Pos {
    fn add_assign(&mut self, v: Vec2) {
        *self = *self + v;
    }
}

impl SubAssign<Vec2> for Pos {
    fn sub_assign(&mut self, v: Vec2) {
        *self = *self - v;
    }
}

impl Add for Vec2 {
    type Output = Vec2;
    fn add(self, v: Vec2) -> Vec2 {
        vec2(self.x + v.x, self.y + v.y)
    }
}

impl Sub for Vec2 {
    type Output = Vec2;
    fn sub(self, v: Vec2) -> Vec2 {
        vec2(self.x - v.x, self.y - v.y)
    }
}

impl AddAssign for Vec2 {
    fn add_assign(&mut self, v: Vec2) {
        *self = *self + v;
    }
}

impl SubAssign for Vec2 {
    fn sub_assign(&mut self, v: Vec2) {
        *self = *self - v;
    }
}

impl Neg for Vec2 {
    type Output = Vec2;
    fn neg(self) -> Vec2 {
        vec2(-self.x, -self.y)
    }
}

impl Mul<f32> for Vec2 {
    type Output = Vec2;
    fn mul(self, s: f32) -> Vec2 {
        vec2(self.x * s, self.y * s)
    }
}

impl Mul<Vec2> for f32 {
    type Output = Vec2;
    fn mul(self, v: Vec2) -> Vec2 {
        vec2(self * v.x, self * v.y)
    }
}

/// Component by component.
impl Mul for Vec2 {
    type Output = Vec2;
    fn mul(self, v: Vec2) -> Vec2 {
        vec2(self.x * v.x, self.y * v.y)
    }
}

impl Div<f32> for Vec2 {
    type Output = Vec2;
    fn div(self, s: f32) -> Vec2 {
        vec2(self.x / s, self.y / s)
    }
}

/// An axis-aligned rectangle on screen. `min` is its top left, `max` its
/// bottom right; a rectangle with `min` past `max` is empty
/// ([`Rect::NOTHING`] is the emptiest, the start of a union).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    /// The top left corner.
    pub min: Pos,
    /// The bottom right corner.
    pub max: Pos,
}

impl Rect {
    /// Every point: the identity of [`Rect::intersect`].
    pub const EVERYTHING: Rect = Rect {
        min: pos(f32::NEG_INFINITY, f32::NEG_INFINITY),
        max: pos(f32::INFINITY, f32::INFINITY),
    };

    /// No point, inside out: the identity of [`Rect::union`].
    pub const NOTHING: Rect = Rect {
        min: pos(f32::INFINITY, f32::INFINITY),
        max: pos(f32::NEG_INFINITY, f32::NEG_INFINITY),
    };

    /// The rectangle from `min` to `max`.
    pub const fn from_min_max(min: Pos, max: Pos) -> Self {
        Rect { min, max }
    }

    /// The rectangle of `size` with its top left at `min`.
    pub fn from_min_size(min: Pos, size: Vec2) -> Self {
        Rect {
            min,
            max: min + size,
        }
    }

    /// The rectangle of `size` centred on `center`.
    pub fn from_center_size(center: Pos, size: Vec2) -> Self {
        Rect {
            min: center - size * 0.5,
            max: center + size * 0.5,
        }
    }

    /// The smallest rectangle holding `a` and `b`.
    pub fn from_two_pos(a: Pos, b: Pos) -> Self {
        Rect {
            min: a.min(b),
            max: a.max(b),
        }
    }

    /// The smallest rectangle holding `points` ([`Rect::NOTHING`] for none).
    pub fn from_points(points: &[Pos]) -> Self {
        points.iter().fold(Rect::NOTHING, |r, &p| r.extended(p))
    }

    /// The middle.
    pub fn center(&self) -> Pos {
        pos(
            (self.min.x + self.max.x) / 2.0,
            (self.min.y + self.max.y) / 2.0,
        )
    }

    /// The width and height.
    pub fn size(&self) -> Vec2 {
        self.max - self.min
    }

    /// The width.
    pub fn width(&self) -> f32 {
        self.max.x - self.min.x
    }

    /// The height.
    pub fn height(&self) -> f32 {
        self.max.y - self.min.y
    }

    /// Width over height.
    pub fn aspect_ratio(&self) -> f32 {
        self.width() / self.height()
    }

    /// Whether it holds `p`, edges included.
    pub fn contains(&self, p: Pos) -> bool {
        self.min.x <= p.x && p.x <= self.max.x && self.min.y <= p.y && p.y <= self.max.y
    }

    /// Whether it holds all of `other`.
    pub fn contains_rect(&self, other: Rect) -> bool {
        self.contains(other.min) && self.contains(other.max)
    }

    /// Whether it and `other` share a point, edges included.
    pub fn intersects(&self, other: Rect) -> bool {
        self.min.x <= other.max.x
            && other.min.x <= self.max.x
            && self.min.y <= other.max.y
            && other.min.y <= self.max.y
    }

    /// Whether it has a positive width and height.
    pub fn is_positive(&self) -> bool {
        self.min.x < self.max.x && self.min.y < self.max.y
    }

    /// Each edge moved `amount` inwards.
    pub fn shrink(self, amount: f32) -> Rect {
        self.shrink2(Vec2::splat(amount))
    }

    /// The left and right edges moved `amount.x` inwards, top and bottom
    /// `amount.y`.
    pub fn shrink2(self, amount: Vec2) -> Rect {
        Rect::from_min_max(self.min + amount, self.max - amount)
    }

    /// Each edge moved `amount` outwards.
    pub fn expand(self, amount: f32) -> Rect {
        self.expand2(Vec2::splat(amount))
    }

    /// The left and right edges moved `amount.x` outwards, top and bottom
    /// `amount.y`.
    pub fn expand2(self, amount: Vec2) -> Rect {
        Rect::from_min_max(self.min - amount, self.max + amount)
    }

    /// Moved by `amount`.
    pub fn translate(self, amount: Vec2) -> Rect {
        Rect::from_min_size(self.min + amount, self.size())
    }

    /// The part it shares with `other` (empty when none).
    pub fn intersect(self, other: Rect) -> Rect {
        Rect {
            min: self.min.max(other.min),
            max: self.max.min(other.max),
        }
    }

    /// The smallest rectangle holding both.
    pub fn union(self, other: Rect) -> Rect {
        Rect {
            min: self.min.min(other.min),
            max: self.max.max(other.max),
        }
    }

    /// The smallest rectangle holding it and `p`.
    pub fn extended(self, p: Pos) -> Rect {
        Rect {
            min: self.min.min(p),
            max: self.max.max(p),
        }
    }

    /// The top left corner.
    pub fn left_top(&self) -> Pos {
        self.min
    }

    /// The top right corner.
    pub fn right_top(&self) -> Pos {
        pos(self.max.x, self.min.y)
    }

    /// The bottom left corner.
    pub fn left_bottom(&self) -> Pos {
        pos(self.min.x, self.max.y)
    }

    /// The bottom right corner.
    pub fn right_bottom(&self) -> Pos {
        self.max
    }
}

/// Where along one axis: its start (left or top), middle or end.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Align {
    /// Left or top.
    #[default]
    Min,
    /// The middle.
    Center,
    /// Right or bottom.
    Max,
}

impl Align {
    /// 0, 0.5 or 1.
    pub fn to_factor(self) -> f32 {
        match self {
            Self::Min => 0.0,
            Self::Center => 0.5,
            Self::Max => 1.0,
        }
    }
}

/// Where in two dimensions: the horizontal then the vertical [`Align`].
/// A text's anchor says which of its points sits at the given position.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Align2(pub Align, pub Align);

impl Align2 {
    /// Top left.
    pub const LEFT_TOP: Align2 = Align2(Align::Min, Align::Min);
    /// Middle of the left edge.
    pub const LEFT_CENTER: Align2 = Align2(Align::Min, Align::Center);
    /// Bottom left.
    pub const LEFT_BOTTOM: Align2 = Align2(Align::Min, Align::Max);
    /// Middle of the top edge.
    pub const CENTER_TOP: Align2 = Align2(Align::Center, Align::Min);
    /// The middle.
    pub const CENTER_CENTER: Align2 = Align2(Align::Center, Align::Center);
    /// Middle of the bottom edge.
    pub const CENTER_BOTTOM: Align2 = Align2(Align::Center, Align::Max);
    /// Top right.
    pub const RIGHT_TOP: Align2 = Align2(Align::Max, Align::Min);
    /// Middle of the right edge.
    pub const RIGHT_CENTER: Align2 = Align2(Align::Max, Align::Center);
    /// Bottom right.
    pub const RIGHT_BOTTOM: Align2 = Align2(Align::Max, Align::Max);

    /// The rectangle of `size` whose anchor point is at `at`.
    pub fn anchor_size(self, at: Pos, size: Vec2) -> Rect {
        let x = match self.0 {
            Align::Min => at.x,
            Align::Center => at.x - 0.5 * size.x,
            Align::Max => at.x - size.x,
        };
        let y = match self.1 {
            Align::Min => at.y,
            Align::Center => at.y - 0.5 * size.y,
            Align::Max => at.y - size.y,
        };
        Rect::from_min_size(pos(x, y), size)
    }

    /// The anchor point of `rect`.
    pub fn pos_in_rect(self, rect: &Rect) -> Pos {
        let x = match self.0 {
            Align::Min => rect.min.x,
            Align::Center => rect.center().x,
            Align::Max => rect.max.x,
        };
        let y = match self.1 {
            Align::Min => rect.min.y,
            Align::Center => rect.center().y,
            Align::Max => rect.max.y,
        };
        pos(x, y)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rectangles_measure_and_combine() {
        let r = Rect::from_min_size(pos(10.0, 20.0), vec2(30.0, 40.0));
        assert_eq!(r.center(), pos(25.0, 40.0));
        assert_eq!(r.size(), vec2(30.0, 40.0));
        assert!(r.contains(pos(10.0, 60.0)) && !r.contains(pos(9.9, 30.0)));
        assert_eq!(Rect::from_center_size(r.center(), r.size()), r);
        assert_eq!(r.shrink(5.0).size(), vec2(20.0, 30.0));
        let s = Rect::from_min_size(pos(30.0, 50.0), vec2(30.0, 30.0));
        assert_eq!(r.intersect(s), Rect::from_min_max(pos(30.0, 50.0), r.max));
        assert_eq!(Rect::NOTHING.union(r), r);
        assert_eq!(Rect::EVERYTHING.intersect(r), r);
        assert!(!r.intersect(r.translate(vec2(100.0, 0.0))).is_positive());
        assert_eq!(
            Rect::from_points(&[pos(3.0, 1.0), pos(1.0, 3.0)]).size(),
            vec2(2.0, 2.0)
        );
    }

    #[test]
    fn anchors_place_sizes() {
        let at = pos(100.0, 100.0);
        let size = vec2(20.0, 10.0);
        assert_eq!(Align2::LEFT_TOP.anchor_size(at, size).min, at);
        assert_eq!(Align2::CENTER_CENTER.anchor_size(at, size).center(), at);
        assert_eq!(Align2::RIGHT_BOTTOM.anchor_size(at, size).max, at);
        let r = Align2::LEFT_BOTTOM.anchor_size(at, size);
        assert_eq!(Align2::LEFT_BOTTOM.pos_in_rect(&r), at);
    }

    #[test]
    fn vectors_measure() {
        assert_eq!(vec2(3.0, 4.0).length(), 5.0);
        assert_eq!(vec2(3.0, 4.0).normalized(), vec2(0.6, 0.8));
        assert_eq!(Vec2::ZERO.normalized(), Vec2::ZERO);
        assert_eq!(pos(1.0, 1.0).distance(pos(4.0, 5.0)), 5.0);
        assert_eq!(vec2(-2.0, 1.0).abs().max_elem(), 2.0);
    }
}
