//! [`Units`]: the one length unit and up axis.
use crate::Error;
use crate::Vec3;

/// Which axis points up.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum UpAxis {
    /// Y up.
    #[default]
    Y,
    /// Z up.
    Z,
}

impl UpAxis {
    /// The unit vector pointing up.
    pub fn direction(self) -> Vec3 {
        match self {
            UpAxis::Y => Vec3::new(0.0, 1.0, 0.0),
            UpAxis::Z => Vec3::new(0.0, 0.0, 1.0),
        }
    }
}

/// A project's units: how long one scene unit is, and the axis that is up.
/// Every length the SDK exchanges is in scene units; this is the one place
/// that says what a unit is (tolerances, render engines' real sizes, a
/// file's lengths read into a project, glyph sizes). Its length is
/// checked: finite and above zero.
///
/// ```
/// use kinogaki_geometry::{UpAxis, Units};
///
/// let inches = Units::new(0.0254, UpAxis::Z).unwrap();
/// assert!((inches.millimeters_per_unit() - 25.4).abs() < 1e-12);
/// assert!(Units::new(0.0, UpAxis::Y).is_err());
/// assert_eq!(Units::default(), Units::METERS);
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Units {
    meters_per_unit: f64,
    up: UpAxis,
}

impl Default for Units {
    /// [`Units::METERS`].
    fn default() -> Self {
        Self::METERS
    }
}

impl Units {
    /// Metres, Y up.
    pub const METERS: Units = Units {
        meters_per_unit: 1.0,
        up: UpAxis::Y,
    };

    /// Millimetres, Y up.
    pub const MILLIMETERS: Units = Units {
        meters_per_unit: 0.001,
        up: UpAxis::Y,
    };

    /// One unit `meters_per_unit` metres long (0.001 for millimetres),
    /// `up` up.
    ///
    /// # Errors
    /// A length that isn't finite and above zero.
    pub fn new(meters_per_unit: f64, up: UpAxis) -> Result<Self, Error> {
        if !(meters_per_unit.is_finite() && meters_per_unit > 0.0) {
            return Err(Error::invalid("a unit must be a finite length above zero"));
        }
        Ok(Self {
            meters_per_unit,
            up,
        })
    }

    /// Metres in one unit.
    pub fn meters_per_unit(&self) -> f64 {
        self.meters_per_unit
    }

    /// Millimetres in one unit (1000 for metres).
    pub fn millimeters_per_unit(&self) -> f64 {
        self.meters_per_unit * 1000.0
    }

    /// The axis that is up.
    pub fn up(&self) -> UpAxis {
        self.up
    }
}
