//! [`Tolerances`]: the project's modeling tolerances, the contract every
//! exact body is built to (the project value `kinogaki.tolerances`).
use crate::error::{ErrorKind, GeometryError};
use crate::units::Units;

/// The positional tolerance's range, in millimeters.
pub(crate) const POSITIONAL_MM: (f64, f64) = (1e-5, 1.0);
/// The fit tolerance's range, as multiples of the positional one.
pub(crate) const FIT_OF_POSITIONAL: (f64, f64) = (1e-3, 10.0);
/// The tangent tolerance's range, in degrees.
pub(crate) const TANGENT_DEGREES: (f64, f64) = (0.001, 5.0);
/// The curvature tolerance's range, as a fraction.
pub(crate) const CURVATURE: (f64, f64) = (1e-4, 0.1);

/// The project's tolerances: one setting per project, chosen by the user,
/// absolute (never derived from a model's size). Lengths are kept in
/// millimeters with the project's [`Units`] beside them, so a kernel
/// converts once.
///
/// | setting | default | meaning |
/// |---|---|---|
/// | positional P | 0.01 mm | points within P are one point; the validity contract |
/// | fit F | 0.001 mm | how closely a fitted shape follows its exact definition |
/// | tangent T | 0.1° | directions within T are tangent (G1) |
/// | curvature K | 0.5 % | relative curvature difference within K is G2 |
///
/// Tolerances a kernel derives from these (an edge fit, a solver's
/// resolution) are its own.
///
/// ```
/// use kinogaki_geometry::{Tolerances, UpAxis, Units};
///
/// let inches = Units::new(0.0254, UpAxis::Y).unwrap();
/// let t = Tolerances::new(0.01, 0.001, 0.1, 0.005, inches).unwrap();
/// assert!((t.fit() - 0.001 / 25.4).abs() < 1e-18);
/// assert!(Tolerances::new(2.0, 0.001, 0.1, 0.005, inches).is_err()); // P above 1 mm
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerances {
    positional_mm: f64,
    fit_mm: f64,
    tangent_degrees: f64,
    curvature: f64,
    units: Units,
}

impl Tolerances {
    /// P and F in millimeters, T in degrees, K as a fraction, for a model
    /// in `units`, checked against the ranges (`POSITIONAL_MM`,
    /// `FIT_OF_POSITIONAL`, `TANGENT_DEGREES`, `CURVATURE`).
    ///
    /// # Errors
    /// The first setting out of its range.
    pub fn new(
        positional_mm: f64,
        fit_mm: f64,
        tangent_degrees: f64,
        curvature: f64,
        units: Units,
    ) -> Result<Self, GeometryError> {
        let refuse = |m: &str| Err(GeometryError::whole(ErrorKind::Range, m));
        if !within(positional_mm, POSITIONAL_MM) {
            return refuse("the positional tolerance must be between 0.00001 mm and 1 mm");
        }
        let fit = (
            positional_mm * FIT_OF_POSITIONAL.0,
            positional_mm * FIT_OF_POSITIONAL.1,
        );
        if !within(fit_mm, fit) {
            return refuse(
                "the fit tolerance must be between a thousandth of the positional tolerance and ten times it",
            );
        }
        if !within(tangent_degrees, TANGENT_DEGREES) {
            return refuse("the tangent tolerance must be between 0.001° and 5°");
        }
        if !within(curvature, CURVATURE) {
            return refuse("the curvature tolerance must be between 0.01 % and 10 %");
        }
        Ok(Self {
            positional_mm,
            fit_mm,
            tangent_degrees,
            curvature,
            units,
        })
    }

    /// P, in millimeters.
    pub fn positional_mm(&self) -> f64 {
        self.positional_mm
    }

    /// F, in millimeters.
    pub fn fit_mm(&self) -> f64 {
        self.fit_mm
    }

    /// T, in degrees.
    pub fn tangent_degrees(&self) -> f64 {
        self.tangent_degrees
    }

    /// K, as a fraction.
    pub fn curvature(&self) -> f64 {
        self.curvature
    }

    /// The model's units.
    pub fn units(&self) -> Units {
        self.units
    }

    /// F, in model units.
    pub fn fit(&self) -> f64 {
        self.fit_mm / self.units.millimeters_per_unit()
    }

    /// These tolerances for a model in `units` (the lengths in millimeters
    /// stay).
    pub fn with_units(&self, units: Units) -> Self {
        Self { units, ..*self }
    }
}

/// P 0.01 mm, F 0.001 mm, T 0.1°, K 0.5 %, for a model in millimeters.
impl Default for Tolerances {
    fn default() -> Self {
        Self::new(0.01, 0.001, 0.1, 0.005, Units::MILLIMETERS).expect("the defaults are in range")
    }
}

fn within(value: f64, (min, max): (f64, f64)) -> bool {
    (min..=max).contains(&value)
}
