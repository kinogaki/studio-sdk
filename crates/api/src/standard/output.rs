//! How a project's pictures reach a display, as a stored project value.
use crate::Error;
use crate::element::{ElementReader, ElementWriter, Field};
use crate::id::StoredId;
use crate::stored::{Scope, Stored};

/// Where a picture's background joins it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum BackgroundSpace {
    /// Composited in linear light, then through the view with the pieces: its
    /// color is linear in the working space.
    Scene,
    /// Added after the pieces reach the display, blended in the display's
    /// encoding: its color is the display's code values, so 1 is display white.
    #[default]
    Display,
}

/// The target display and background of what the 2D canvas shows and final
/// outputs write: the element `output` among the project's settings. Core
/// registers it, and its canvas and final writes read it; a package's project
/// section edits it. Written files say which display they are for.
///
/// ```
/// use kinogaki_studio_api::Stored;
/// use kinogaki_studio_api::element::MemoryElement;
/// use kinogaki_studio_api::standard::{BackgroundSpace, DisplayOutput};
///
/// let black = DisplayOutput {
///     display: "Display P3 - Display".into(),
///     background: [0.0; 3],
///     background_space: BackgroundSpace::Scene,
/// };
/// let mut e = MemoryElement::new("output");
/// e.write(|w| black.write(w)).unwrap();
/// assert_eq!(e.read(|r| DisplayOutput::read(r)).unwrap(), black);
/// assert_eq!(DisplayOutput::defaults().background, [1.0; 3]);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct DisplayOutput {
    /// The target display's name in the color configuration; empty: the first
    /// it can target ([`ColorService::target_displays`]).
    ///
    /// [`ColorService::target_displays`]: crate::color::ColorService::target_displays
    pub display: String,
    /// The background's color, meant as [`background_space`](Self::background_space) says.
    pub background: [f64; 3],
    /// Where it joins.
    pub background_space: BackgroundSpace,
}

impl Stored for DisplayOutput {
    fn id() -> StoredId {
        StoredId::declared("kinogaki.core.output")
    }
    fn scope() -> Scope {
        Scope::Project
    }
    fn defaults() -> Self {
        DisplayOutput {
            display: String::new(),
            background: [1.0; 3],
            background_space: BackgroundSpace::Display,
        }
    }
    fn write(&self, to: &mut dyn ElementWriter) -> Result<(), Error> {
        to.set("display", Field::Text(self.display.clone()))?;
        to.set("background", Field::F64s(self.background.to_vec()))?;
        let space = match self.background_space {
            BackgroundSpace::Scene => "scene",
            BackgroundSpace::Display => "display",
        };
        to.set("backgroundSpace", Field::Text(space.to_owned()))
    }
    fn read(from: &dyn ElementReader) -> Result<Self, Error> {
        let background = (from.f64s("background")?.try_into())
            .map_err(|_| Error::invalid("a background has three channels"))?;
        let background_space = match from.text("backgroundSpace")? {
            "scene" => BackgroundSpace::Scene,
            "display" => BackgroundSpace::Display,
            other => {
                return Err(Error::invalid(format_args!("no background space {other}")));
            }
        };
        Ok(DisplayOutput {
            display: from.text("display")?.to_owned(),
            background,
            background_space,
        })
    }
}
