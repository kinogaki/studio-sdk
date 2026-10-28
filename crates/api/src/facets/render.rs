//! [`Renderable`]: how a value reaches render engines.
use crate::cancel::CancelToken;
use crate::data::Data;
use crate::error::Error;
use crate::render::RenderItem;
use crate::time::Time;

/// How a data type renders: as neutral [`RenderItem`]s every engine draws,
/// whatever kernel made the value.
pub trait Renderable: Send + Sync {
    /// `data` as render items at `time`, placed relative to its object
    /// (the host places the object in the world). The host matches items
    /// of successive values by their `part` and compares their sources by
    /// identity, so a type returns the same source for an unchanged part
    /// and a derived one (same topology, [`moved_since`]) for a dragged
    /// one.
    ///
    /// [`moved_since`]: crate::render::TriangleSource::moved_since
    fn items(
        &self,
        data: &Data,
        time: Time,
        cancel: &CancelToken,
    ) -> Result<Vec<RenderItem>, Error>;
}
