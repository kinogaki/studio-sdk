//! [`Number`]: how a number field shows, moves and limits its value.
use kinogaki_studio_api::params::Unit;
use std::ops::RangeInclusive;

/// How a number field looks and moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NumberStyle {
    /// A field dragged to scrub by [`Number::speed`] per point.
    #[default]
    Field,
    /// The value's place in the soft range fills the field; dragging sets
    /// it from the pointer.
    Slider,
    /// A whole number with step arrows at the right.
    Stepper,
}

/// A number's unit, limits and look. The host shows and parses the value
/// in the user's units (a [`Unit::Length`] in the project's millimetres or
/// inches), so a plugin states only what the number measures.
#[derive(Clone, Debug, PartialEq)]
pub struct Number {
    /// What it measures.
    pub unit: Unit,
    /// Hard limits: typed and dragged values clamp to them.
    pub range: RangeInclusive<f64>,
    /// The span dragging and the slider cover; typing may leave it.
    pub soft: Option<RangeInclusive<f64>>,
    /// Stored units per dragged point (a tenth with Shift).
    pub speed: f64,
    /// Most decimal places shown; 0 rounds to whole numbers.
    pub decimals: usize,
    /// How the field looks and moves.
    pub style: NumberStyle,
    /// Where a slider's fill starts (stored units): it fills between this
    /// and the value, so a value at its origin shows empty. `None` fills
    /// from the span's start.
    pub origin: Option<f64>,
}

impl Default for Number {
    fn default() -> Self {
        Self::new()
    }
}

impl Number {
    /// A plain number: no unit or limits, scrubbed by 0.01 per point, up to
    /// four places.
    pub const fn new() -> Self {
        Number {
            unit: Unit::Plain,
            range: f64::NEG_INFINITY..=f64::INFINITY,
            soft: None,
            speed: 0.01,
            decimals: 4,
            style: NumberStyle::Field,
            origin: None,
        }
    }

    /// A whole number in `range` with step arrows.
    pub fn integer(range: RangeInclusive<f64>) -> Self {
        Number {
            range,
            speed: 1.0,
            decimals: 0,
            style: NumberStyle::Stepper,
            ..Self::new()
        }
    }

    /// A slider across `range` (0–1 for weights).
    pub fn slider(range: RangeInclusive<f64>) -> Self {
        Number {
            range,
            style: NumberStyle::Slider,
            ..Self::new()
        }
    }

    /// A length, in scene units.
    pub fn length() -> Self {
        Number {
            unit: Unit::Length,
            ..Self::new()
        }
    }

    /// An angle, in degrees.
    pub fn angle() -> Self {
        Number {
            unit: Unit::Angle,
            speed: 0.5,
            ..Self::new()
        }
    }

    /// A percentage stored as shown (0–100).
    pub fn percent() -> Self {
        Number {
            unit: Unit::Percent,
            speed: 0.5,
            decimals: 1,
            ..Self::new()
        }
    }

    /// Measuring `unit`.
    pub fn unit(mut self, unit: Unit) -> Self {
        self.unit = unit;
        self
    }

    /// With hard limits `range`.
    pub fn range(mut self, range: RangeInclusive<f64>) -> Self {
        self.range = range;
        self
    }

    /// With the soft range `soft` (what dragging and the slider cover).
    pub fn soft(mut self, soft: RangeInclusive<f64>) -> Self {
        self.soft = Some(soft);
        self
    }

    /// Moving `speed` stored units per dragged point.
    pub fn speed(mut self, speed: f64) -> Self {
        self.speed = speed;
        self
    }

    /// A slider filled from `origin` to the value rather than from its
    /// start (a −100..100 slider about 0 shows empty at 0).
    pub fn origin(mut self, origin: f64) -> Self {
        self.origin = Some(origin);
        self
    }

    /// Showing at most `decimals` places.
    pub fn decimals(mut self, decimals: usize) -> Self {
        self.decimals = decimals;
        self
    }

    /// Drawn as `style`.
    pub fn style(mut self, style: NumberStyle) -> Self {
        self.style = style;
        self
    }

    /// `v` within the hard range (rounded when whole).
    pub fn clamp(&self, v: f64) -> f64 {
        let v = v.clamp(*self.range.start(), *self.range.end());
        if self.decimals == 0 { v.round() } else { v }
    }

    /// The span the slider fill and drags cover.
    pub fn span(&self) -> RangeInclusive<f64> {
        self.soft.clone().unwrap_or_else(|| self.range.clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_clamp_and_span() {
        let n = Number::integer(1.0..=8.0);
        assert_eq!(n.clamp(3.6), 4.0);
        assert_eq!(n.clamp(20.0), 8.0);
        assert_eq!(n.style, NumberStyle::Stepper);
        let s = Number::slider(0.0..=10.0).soft(0.0..=1.0);
        assert_eq!(s.span(), 0.0..=1.0);
        assert_eq!(s.clamp(4.25), 4.25);
        assert_eq!(Number::angle().unit, Unit::Angle);
        assert_eq!(Number::percent().decimals, 1);
    }
}
