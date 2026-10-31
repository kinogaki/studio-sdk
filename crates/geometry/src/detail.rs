//! [`Detail`]: how fine geometry is made for a render.
use crate::error::{ErrorKind, GeometryError};

/// How fine to make geometry for a render: a B-rep's tessellation, a
/// sampled surface. One Render Settings value gives it to every engine;
/// while a gesture is under way, render views use
/// [`Viewport`](Detail::Viewport) and refine when idle.
///
/// New levels may be added; a producer that doesn't know one makes
/// geometry at its [`Render`](Detail::Render)-like finest.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[non_exhaustive]
pub enum Detail {
    /// As fine as the viewport draws it: interactive views and drags.
    #[default]
    Viewport,
    /// For a final render. Build it with [`Detail::render`], which checks
    /// the ranges.
    Render {
        /// How far a facet may stray from the exact surface, in scene
        /// units: above 0.
        chord: f64,
        /// How sharply neighboring facets may turn, in degrees: above 0,
        /// at most 90.
        angle: f64,
    },
}

impl Detail {
    /// [`Detail::Render`] at `chord` and `angle`, checked.
    ///
    /// ```
    /// use kinogaki_geometry::Detail;
    /// assert!(Detail::render(0.01, 5.0).is_ok());
    /// assert!(Detail::render(0.0, 5.0).is_err() && Detail::render(0.01, 120.0).is_err());
    /// ```
    ///
    /// # Errors
    /// A chord not above 0, or an angle not in (0, 90] degrees.
    pub fn render(chord: f64, angle: f64) -> Result<Detail, GeometryError> {
        let detail = Detail::Render { chord, angle };
        detail.check()?;
        Ok(detail)
    }

    /// Whether its numbers are in range (a [`Detail::Render`] written
    /// directly is checked here).
    ///
    /// # Errors
    /// As [`Detail::render`].
    pub fn check(&self) -> Result<(), GeometryError> {
        if let Detail::Render { chord, angle } = *self {
            if !(chord.is_finite() && chord > 0.0) {
                return Err(GeometryError::whole(
                    ErrorKind::Range,
                    format!("a render chord of {chord} is not above 0"),
                ));
            }
            if !(angle > 0.0 && angle <= 90.0) {
                return Err(GeometryError::whole(
                    ErrorKind::Range,
                    format!("a render angle of {angle}° is not above 0° and at most 90°"),
                ));
            }
        }
        Ok(())
    }
}
