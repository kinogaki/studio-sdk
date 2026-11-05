//! [`NumberLook`]: how a row of numbers shows and moves. It is presentation,
//! carried beside a [`ValueKind`](crate::ValueKind) by what has a row (a
//! parameter, an engine's setting, a sheet's field), never part of the kind:
//! the kind says what a value holds and its limits.

/// How a number field looks and moves.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum NumberStyle {
    /// A field dragged to scrub.
    #[default]
    Field,
    /// The value's place in the soft span (or the limits) fills the field;
    /// dragging sets it from the pointer.
    Slider,
    /// A whole number with step arrows at the right.
    Stepper,
}

/// How a number, or each of a row's numbers, shows and moves. What it
/// leaves `None` the host chooses by the number's unit.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct NumberLook {
    /// How the field looks and moves.
    pub style: NumberStyle,
    /// Most decimal places shown; 0 rounds to whole numbers.
    pub decimals: Option<u8>,
    /// Stored units per point dragged (a tenth with Shift).
    pub speed: Option<f64>,
    /// Where a slider's fill starts (stored units): it fills between this
    /// and the value, so a value at its origin shows empty. `None` fills
    /// from the span's start.
    pub origin: Option<f64>,
}

impl NumberLook {
    /// A slider.
    pub fn slider() -> Self {
        Self {
            style: NumberStyle::Slider,
            ..Self::default()
        }
    }

    /// A whole number with step arrows.
    pub fn stepper() -> Self {
        Self {
            style: NumberStyle::Stepper,
            ..Self::default()
        }
    }

    /// Showing at most `places` decimals.
    pub fn decimals(mut self, places: u8) -> Self {
        self.decimals = Some(places);
        self
    }

    /// Changing by `speed` per point dragged.
    pub fn speed(mut self, speed: f64) -> Self {
        self.speed = Some(speed);
        self
    }

    /// A slider filled from `origin` to the value rather than from its
    /// start (a −100..100 slider about 0 shows empty at 0).
    pub fn origin(mut self, origin: f64) -> Self {
        self.origin = Some(origin);
        self
    }
}
