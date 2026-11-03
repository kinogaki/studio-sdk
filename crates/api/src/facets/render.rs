//! [`Renderable`]: how a value reaches render engines.
use crate::Error;
use crate::cancel::CancelToken;
use crate::data::Data;
use crate::render::{Detail, RenderItem};
use kinogaki_geometry::Time;
use kinogaki_geometry::Tolerances;
use std::any::Any;
use std::sync::Arc;

/// How a data type renders: as neutral [`RenderItem`]s every engine draws,
/// whatever kernel made the value.
pub trait Renderable: Send + Sync {
    /// `data` as render items at `cx`'s time and detail, relative to its
    /// object (the host places the object in the world), given
    /// what this facet answered for the value the object rendered before,
    /// when there was one (as [`Displayable::prepare`] is): a type whose
    /// values change a little at a time (a drag's next state) builds on
    /// it. The host matches items of successive values by their `part` and
    /// compares their sources by identity, so a type returns the same
    /// source for an unchanged part and a derived one (same topology,
    /// [`moved_since`]) for a dragged one.
    ///
    /// [`Displayable::prepare`]: crate::facets::Displayable::prepare
    /// [`moved_since`]: crate::render::ShapeSource::moved_since
    fn items(
        &self,
        data: &Data,
        previous: Option<&Rendered>,
        cx: &RenderCx<'_>,
    ) -> Result<Rendered, Error>;
}

/// What [`Renderable::items`] answers: the items, and beside them any
/// detail of the type's own its next call reads (the sources it made, so
/// the next value's are made after them).
#[derive(Clone)]
pub struct Rendered {
    /// The render items, in order.
    pub items: Arc<[RenderItem]>,
    detail: Option<Arc<dyn Any + Send + Sync>>,
}

impl Rendered {
    /// `items` with no detail beside them.
    pub fn new(items: Vec<RenderItem>) -> Self {
        Self {
            items: items.into(),
            detail: None,
        }
    }

    /// With `detail` beside the items, for the type's own later calls.
    pub fn with_detail<T: Any + Send + Sync>(mut self, detail: T) -> Self {
        self.detail = Some(Arc::new(detail));
        self
    }

    /// The detail, when it is a `T`.
    pub fn detail<T: Any>(&self) -> Option<&T> {
        self.detail.as_deref().and_then(|d| d.downcast_ref())
    }
}

impl std::fmt::Debug for Rendered {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_list().entries(self.items.iter()).finish()
    }
}

/// What a render asks of [`Renderable::items`].
#[derive(Clone, Copy, Debug)]
#[non_exhaustive]
pub struct RenderCx<'a> {
    /// The scene time it renders.
    pub time: Time,
    /// How fine to make the geometry: for a render view, the object's
    /// [`Detail::Viewport`] at its display quality (the host fills it).
    pub detail: Detail,
    /// The project's tolerances, which an approximate value (a B-rep's
    /// mesh) is made to, as its display is (the host fills them; the
    /// default otherwise).
    pub tolerances: Tolerances,
    /// Set when the render no longer wants the items.
    pub cancel: &'a CancelToken,
}

impl<'a> RenderCx<'a> {
    /// A render at `time` at the viewport's detail of the default display
    /// quality and the default tolerances.
    pub fn new(time: Time, cancel: &'a CancelToken) -> Self {
        Self {
            time,
            detail: Detail::default(),
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
        assert_eq!(cx.detail, Detail::Viewport(Default::default()));
        let fine = Detail::Render {
            chord: 0.01,
            angle: 5.0,
        };
        assert_eq!(cx.tolerances, Tolerances::default());
        let cx = cx.with_detail(fine);
        assert_eq!((cx.detail, cx.time), (fine, Time::ZERO));
        let inches = Tolerances::default().with_units(kinogaki_geometry::Units::METERS);
        let cx = cx.with_tolerances(inches);
        assert_eq!((cx.tolerances, cx.detail), (inches, fine));
    }
}
