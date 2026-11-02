//! [`Detail`]: how fine geometry is made for a view or a render; and
//! [`DisplayQuality`], how fine an interactive view makes it.
use crate::error::{ErrorKind, GeometryError};

/// How fine to make geometry for a render: a B-rep's tessellation, a
/// sampled surface. One Render Settings value gives it to every engine;
/// while a gesture is under way, render views use
/// [`Viewport`](Detail::Viewport) and refine when idle.
///
/// New levels may be added; a producer that doesn't know one makes
/// geometry at its [`Render`](Detail::Render)-like finest.
#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub enum Detail {
    /// As fine as the viewport draws the object, for interactive views
    /// and drags: the producer's own preset for the object's display
    /// quality, never finer than the project's positional tolerance
    /// allows. The project holds no display chord or angle for it.
    Viewport(DisplayQuality),
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

/// The viewport at the default display quality.
impl Default for Detail {
    fn default() -> Self {
        Detail::Viewport(DisplayQuality::default())
    }
}

/// How finely an object whose display is tessellated (a B-rep's mesh, a
/// sampled surface) is made for interactive views: one of a few named
/// steps, coarse to fine, each of which a producer maps to a preset of
/// its own. The project holds one (the project value `display_quality`)
/// and an object may override it; [`Detail::Viewport`] carries the
/// object's.
///
/// New steps may be added; a producer that doesn't know one makes
/// geometry at its finest step.
///
/// ```
/// use kinogaki_geometry::DisplayQuality;
/// assert_eq!(DisplayQuality::default(), DisplayQuality::Medium);
/// for q in DisplayQuality::ALL {
///     assert_eq!(DisplayQuality::parse(q.label()), Some(q));
/// }
/// ```
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum DisplayQuality {
    /// Facets that show: fastest.
    Coarse,
    /// Smooth at a glance: the default.
    #[default]
    Medium,
    /// Smooth up close.
    Fine,
    /// For close inspection; slowest.
    Finest,
}

impl DisplayQuality {
    /// Every step, coarse to fine.
    pub const ALL: [DisplayQuality; 4] = [
        DisplayQuality::Coarse,
        DisplayQuality::Medium,
        DisplayQuality::Fine,
        DisplayQuality::Finest,
    ];

    /// Its name, as a UI shows it and a project saves it ("Medium").
    pub fn label(self) -> &'static str {
        match self {
            DisplayQuality::Coarse => "Coarse",
            DisplayQuality::Medium => "Medium",
            DisplayQuality::Fine => "Fine",
            DisplayQuality::Finest => "Finest",
        }
    }

    /// The step named `label` ([`label`](Self::label)), if one is.
    pub fn parse(label: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|q| q.label() == label)
    }
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
