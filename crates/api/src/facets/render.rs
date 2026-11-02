//! [`Renderable`]: how a value reaches render engines.
use crate::cancel::CancelToken;
use crate::data::Data;
use crate::error::Error;
use crate::render::{Detail, RenderItem};
use crate::time::Time;
use kinogaki_geometry::Tolerances;

/// How a data type renders: as neutral [`RenderItem`]s every engine draws,
/// whatever kernel made the value.
pub trait Renderable: Send + Sync {
    /// `data` as render items at `cx`'s time and detail, placed relative to its object
    /// (the host places the object in the world). The host matches items
    /// of successive values by their `part` and compares their sources by
    /// identity, so a type returns the same source for an unchanged part
    /// and a derived one (same topology, [`moved_since`]) for a dragged
    /// one.
    ///
    /// [`moved_since`]: crate::render::TriangleSource::moved_since
    fn items(&self, data: &Data, cx: &RenderCx<'_>) -> Result<Vec<RenderItem>, Error>;
}

/// What a render asks of [`Renderable::items`].
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct RenderCx<'a> {
    /// The scene time it renders.
    pub time: Time,
    /// How fine to make the geometry.
    pub detail: Detail,
    /// The project's tolerances, which an approximate value (a B-rep's
    /// mesh) is made to, as its display is (the host fills them; the
    /// default otherwise).
    pub tolerances: Tolerances,
    /// Set when the render no longer wants the items.
    pub cancel: &'a CancelToken,
}

impl<'a> RenderCx<'a> {
    /// A render at `time` at the viewport's detail and the default
    /// tolerances.
    pub fn new(time: Time, cancel: &'a CancelToken) -> Self {
        Self {
            time,
            detail: Detail::Viewport,
            tolerances: Tolerances::default(),
            cancel,
        }
    }

    /// At `detail` instead.
    pub fn with_detail(self, detail: Detail) -> Self {
        Self { detail, ..self }
    }

    /// At the project's `tolerances`.
    pub fn with_tolerances(self, tolerances: Tolerances) -> Self {
        Self { tolerances, ..self }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_render_asks_at_the_viewports_detail_unless_told() {
        let cancel = CancelToken::default();
        let cx = RenderCx::new(Time::ZERO, &cancel);
        assert_eq!(cx.detail, Detail::Viewport);
        let fine = Detail::Render {
            chord: 0.01,
            angle: 5.0,
        };
        assert_eq!(cx.tolerances, Tolerances::default());
        let cx = cx.with_detail(fine);
        assert_eq!((cx.detail, cx.time), (fine, Time::ZERO));
        let inches = Tolerances::default().with_unit(25.4).unwrap();
        let cx = cx.with_tolerances(inches);
        assert_eq!((cx.tolerances, cx.detail), (inches, fine));
    }
}
