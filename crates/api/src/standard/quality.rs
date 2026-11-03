//! The project's display quality as a stored project value.
use crate::element::{ElementReader, ElementWriter, Field};
use crate::error::Error;
use crate::id::StoredId;
use crate::stored::{Scope, Stored};
use kinogaki_geometry::DisplayQuality;

/// The project value `display_quality`: the element `display_quality`
/// among the project's settings, with the text `quality` (a step's
/// [`label`](DisplayQuality::label)). It is how finely every object whose
/// display is tessellated shows unless the object overrides it. Core
/// registers it and gives each object's quality to its display and render
/// views ([`Detail::Viewport`](kinogaki_geometry::Detail::Viewport)); a
/// package's settings page may show it.
///
/// ```
/// use kinogaki_studio_api::Stored;
/// use kinogaki_studio_api::element::MemoryElement;
/// use kinogaki_studio_api::geometry::DisplayQuality;
///
/// let mut e = MemoryElement::new("display_quality");
/// e.write(|w| DisplayQuality::Fine.write(w)).unwrap();
/// assert_eq!(e.read(|r| DisplayQuality::read(r)).unwrap(), DisplayQuality::Fine);
/// assert_eq!(DisplayQuality::defaults(), DisplayQuality::Medium);
/// ```
impl Stored for DisplayQuality {
    fn id() -> StoredId {
        StoredId::declared("kinogaki.core.display_quality")
    }
    fn scope() -> Scope {
        Scope::Project
    }
    fn defaults() -> Self {
        DisplayQuality::default()
    }
    fn write(&self, to: &mut dyn ElementWriter) -> Result<(), Error> {
        to.set("quality", Field::Text(self.label().into()))
    }
    fn read(from: &dyn ElementReader) -> Result<Self, Error> {
        let label = from.text("quality")?;
        DisplayQuality::parse(label).ok_or_else(|| {
            Error::invalid(format_args!(
                "{}: {label:?} is no display quality",
                from.location()
            ))
        })
    }
}
