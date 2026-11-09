//! [`Renderable`]: how a value reaches render engines.
use crate::Error;
use crate::cancel::CancelToken;
use crate::data::Data;
use crate::render::{Detail, RenderItem};
use crate::subdivider::Refined;
use kinogaki_geometry::{Geometry, Time, Tolerances};
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
    waiting: bool,
}

impl Rendered {
    /// `items` with no detail beside them.
    pub fn new(items: Vec<RenderItem>) -> Self {
        Self {
            items: items.into(),
            detail: None,
            waiting: false,
        }
    }

    /// Standing in for items the host is still making for the type (a
    /// subdivision surface it refines, [`RenderCx::refine`]): whatever keeps
    /// items by their value (the host's, a type holding values) asks the
    /// type again rather than keeping these, and the host renders the
    /// value again once its work lands.
    pub fn waiting(mut self) -> Self {
        self.waiting = true;
        self
    }

    /// Whether they stand in for items the host is still making
    /// ([`waiting`](Self::waiting)).
    pub fn is_waiting(&self) -> bool {
        self.waiting
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
#[derive(Clone, Copy)]
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
    /// What the host made of values other values hold, which a type
    /// rendering values its value holds asks for each (the host fills it;
    /// none otherwise, and the type renders them itself).
    pub held: Option<&'a dyn super::Held>,
    /// The host's refinement of polygons that subdivide, which a type
    /// rendering such polygons asks for them (the host fills it; none
    /// otherwise, and the type renders the cage).
    pub refine: Option<&'a dyn Refine>,
}

/// The host's refinement of polygons into the subdivision surfaces their
/// [`Subdivision`](kinogaki_geometry::Subdivision) says, made by the
/// installed [`Subdivider`](crate::Subdivider) off the render's thread.
pub trait Refine: Send + Sync {
    /// `cage` refined as its setting says. `previous` is the refinement the
    /// type rendered of the value before it (a drag's last step), which the
    /// host moves when `cage` is that one's cage moved. While the host
    /// works the type renders what it rendered before (else the cage),
    /// [`waiting`](Rendered::waiting), and is asked again once the work
    /// lands.
    fn refined(&self, cage: &Geometry, previous: Option<&Arc<dyn Refined>>) -> Refining;
}

/// What [`Refine::refined`] answers.
#[derive(Clone)]
pub enum Refining {
    /// The refinement.
    Ready(Arc<dyn Refined>),
    /// The host is making it: render a stand-in, waiting.
    Waiting,
    /// It won't be made (no subdivider installed, or the refinement
    /// failed, which the host says): render the cage.
    Cage,
}

impl std::fmt::Debug for RenderCx<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RenderCx")
            .field("time", &self.time)
            .field("detail", &self.detail)
            .field("tolerances", &self.tolerances)
            .field("cancelled", &self.cancel.is_cancelled())
            .field("held", &self.held.is_some())
            .field("refine", &self.refine.is_some())
            .finish()
    }
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
            held: None,
            refine: None,
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

    /// With the host's `held`.
    pub fn with_held(self, held: &'a dyn super::Held) -> Self {
        Self {
            held: Some(held),
            ..self
        }
    }

    /// With the host's `refine`.
    pub fn with_refine(self, refine: &'a dyn Refine) -> Self {
        Self {
            refine: Some(refine),
            ..self
        }
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
