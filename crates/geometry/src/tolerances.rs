//! [`Tolerances`]: the project's modeling tolerances, the contract every
//! exact body is built to (the project value `kinogaki.tolerances`).
use crate::error::{ErrorKind, GeometryError};

/// The positional tolerance's range, in millimeters.
pub const POSITIONAL_MM: (f64, f64) = (1e-5, 1.0);
/// The fit tolerance's range, as multiples of the positional one.
pub const FIT_OF_POSITIONAL: (f64, f64) = (1e-3, 10.0);
/// The tangent tolerance's range, in degrees.
pub const TANGENT_DEGREES: (f64, f64) = (0.001, 5.0);
/// The curvature tolerance's range, as a fraction.
pub const CURVATURE: (f64, f64) = (1e-4, 0.1);

/// The project's tolerances: one setting per project, chosen by the user,
/// absolute (never derived from a model's size). Lengths are kept in
/// millimeters with the model unit beside them, so a kernel converts once.
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
/// use kinogaki_geometry::Tolerances;
///
/// let inches = Tolerances::new(0.01, 0.001, 0.1, 0.005, 25.4).unwrap();
/// assert!((inches.positional() - 0.01 / 25.4).abs() < 1e-15);
/// assert!(Tolerances::new(2.0, 0.001, 0.1, 0.005, 1.0).is_err()); // P above 1 mm
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerances {
    positional_mm: f64,
    fit_mm: f64,
    tangent_degrees: f64,
    curvature: f64,
    millimeters_per_unit: f64,
}

impl Tolerances {
    /// P and F in millimeters, T in degrees, K as a fraction, for a model
    /// whose unit is `millimeters_per_unit` mm (1 for millimeters, 25.4
    /// for inches), checked against the ranges ([`POSITIONAL_MM`],
    /// [`FIT_OF_POSITIONAL`], [`TANGENT_DEGREES`], [`CURVATURE`]).
    ///
    /// # Errors
    /// The first setting out of its range, or a unit that isn't a finite,
    /// positive length.
    pub fn new(
        positional_mm: f64,
        fit_mm: f64,
        tangent_degrees: f64,
        curvature: f64,
        millimeters_per_unit: f64,
    ) -> Result<Self, GeometryError> {
        let refuse = |m: &str| Err(GeometryError::whole(ErrorKind::Range, m));
        if !(millimeters_per_unit.is_finite() && millimeters_per_unit > 0.0) {
            return refuse("a model unit must be a finite, positive length");
        }
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
            millimeters_per_unit,
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

    /// How many millimeters one model unit is.
    pub fn millimeters_per_unit(&self) -> f64 {
        self.millimeters_per_unit
    }

    /// P, in model units.
    pub fn positional(&self) -> f64 {
        self.positional_mm / self.millimeters_per_unit
    }

    /// F, in model units.
    pub fn fit(&self) -> f64 {
        self.fit_mm / self.millimeters_per_unit
    }

    /// These tolerances for a model whose unit is `millimeters_per_unit`
    /// mm (the lengths in millimeters stay).
    ///
    /// # Errors
    /// A unit that isn't a finite, positive length.
    pub fn with_unit(&self, millimeters_per_unit: f64) -> Result<Self, GeometryError> {
        Self::new(
            self.positional_mm,
            self.fit_mm,
            self.tangent_degrees,
            self.curvature,
            millimeters_per_unit,
        )
    }
}

/// P 0.01 mm, F 0.001 mm, T 0.1°, K 0.5 %, in millimeters.
impl Default for Tolerances {
    fn default() -> Self {
        Self::new(0.01, 0.001, 0.1, 0.005, 1.0).expect("the defaults are in range")
    }
}

fn within(value: f64, (min, max): (f64, f64)) -> bool {
    (min..=max).contains(&value)
}
