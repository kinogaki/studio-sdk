//! [`Displayable`]: how a value is drawn in the viewport.
use crate::cancel::CancelToken;
use crate::components::Components;
use crate::data::Data;
use crate::error::Error;
use crate::viewport::{Elements, Level, Surface};
use std::any::Any;
use std::fmt;
use std::sync::Arc;

/// How finely to prepare a value for display. Types whose display is
/// approximate (a CAD body's tessellation, a subdivision preview) pick a
/// tolerance from it; exact types ignore it.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum DisplayQuality {
    /// While the user drags or plays: fast, coarse.
    Interactive,
    /// The resting display.
    #[default]
    Normal,
    /// Close inspection and captures.
    Fine,
}

/// A value prepared for the viewport: its surface, and whatever the type
/// keeps beside it to answer highlights and picks on that surface (a CAD
/// surface's face numbers to topology names).
#[derive(Clone)]
pub struct Drawn {
    /// The surface the viewport uploads and picks on.
    pub surface: Surface,
    detail: Option<Arc<dyn Any + Send + Sync>>,
}

impl Drawn {
    /// `surface` with no detail beside it.
    pub fn new(surface: Surface) -> Self {
        Self {
            surface,
            detail: None,
        }
    }

    /// With `detail` beside the packet, for the type's own later calls.
    pub fn with_detail<T: Any + Send + Sync>(mut self, detail: T) -> Self {
        self.detail = Some(Arc::new(detail));
        self
    }

    /// The detail, when it is a `T`.
    pub fn detail<T: Any>(&self) -> Option<&T> {
        self.detail.as_deref().and_then(|d| d.downcast_ref())
    }
}

impl fmt::Debug for Drawn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Drawn")
            .field("surface", &self.surface)
            .field("detail", &self.detail.is_some())
            .finish()
    }
}

/// How a data type shows in the viewport.
///
/// `prepare` runs on a worker; the others run on the UI thread with a
/// surface `prepare` made for the same value, so they must be quick
/// (proportional to the selection, not to the value).
pub trait Displayable: Send + Sync {
    /// The component levels values offer, in the order the level bar
    /// lists them (a mesh: points, edges, faces, vertices). Empty: only
    /// whole objects are selected.
    fn levels(&self) -> &[Level];

    /// `data`'s viewport surface at `quality`. Long preparations poll
    /// `cancel`. During a drag, a type that knows only points moved
    /// returns a [`Surface::derived`] from the last one it made.
    fn prepare(
        &self,
        data: &Data,
        quality: DisplayQuality,
        cancel: &CancelToken,
    ) -> Result<Drawn, Error>;

    /// The elements of `drawn`'s surface that show `components` (what the
    /// viewport outlines and tints as selected).
    fn highlight(&self, data: &Data, drawn: &Drawn, components: &Components) -> Elements;

    /// `components` at level `to`: faces as their edges or points, points
    /// as the faces they bound wholly. `None` when the type can't express
    /// the selection at that level.
    fn convert(&self, data: &Data, components: &Components, to: Level) -> Option<Components>;
}
