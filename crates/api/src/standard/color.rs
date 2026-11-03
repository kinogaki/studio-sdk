//! The project's color configuration as a stored project value.
use crate::element::{ElementReader, ElementWriter, Field};
use crate::error::Error;
use crate::id::StoredId;
use crate::stored::{Scope, Stored};

/// Which OpenColorIO config manages a project's colors and the working
/// space its scene colors are in: the element `color` among the project's
/// settings, with the texts `config` (empty: the host's built-in ACES 2.0
/// CG config) and `working` (empty: the config's `scene_linear` role).
/// Core registers it and builds its color service from it; the render
/// package's Color section edits it. A project's colors mean what they
/// mean whichever user opens it.
///
/// ```
/// use kinogaki_studio_api::Stored;
/// use kinogaki_studio_api::element::MemoryElement;
/// use kinogaki_studio_api::standard::ColorConfiguration;
///
/// let linear = ColorConfiguration { config: None, working: "Linear Rec.709 (sRGB)".into() };
/// let mut e = MemoryElement::new("color");
/// e.write(|w| linear.write(w)).unwrap();
/// assert_eq!(e.read(|r| ColorConfiguration::read(r)).unwrap(), linear);
/// assert_eq!(ColorConfiguration::defaults(), ColorConfiguration::default());
/// ```
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ColorConfiguration {
    /// The `.ocio` config's path (relative to the project's folder when
    /// under it); none: the built-in ACES 2.0 CG config.
    pub config: Option<String>,
    /// The working space's name in the config; empty: its `scene_linear`
    /// role.
    pub working: String,
}

impl Stored for ColorConfiguration {
    fn id() -> StoredId {
        StoredId::declared("kinogaki.core.color")
    }
    fn scope() -> Scope {
        Scope::Project
    }
    fn defaults() -> Self {
        ColorConfiguration::default()
    }
    fn write(&self, to: &mut dyn ElementWriter) -> Result<(), Error> {
        let config = self.config.clone().unwrap_or_default();
        to.set("config", Field::Text(config))?;
        to.set("working", Field::Text(self.working.clone()))
    }
    fn read(from: &dyn ElementReader) -> Result<Self, Error> {
        let config = from.text("config")?;
        Ok(ColorConfiguration {
            config: (!config.is_empty()).then(|| config.to_owned()),
            working: from.text("working")?.to_owned(),
        })
    }
}
