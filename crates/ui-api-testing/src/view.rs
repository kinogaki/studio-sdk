//! Views as the host hands them to a view call, for plugin tests:
//! [`view`] and [`camera`], to change as a test needs (its index, its
//! response, what the pointer is over).
use kinogaki_studio_api::Vec3;
use studio_ui_api::host::{Ground, PointerOver, View, ViewCamera};
use studio_ui_api::{Projection, Rect, Response, Slot};

/// The only view, at `rect` (the view pane too): the active one, nothing
/// modal, the pointer free over it, no clicks or drags this frame, seen
/// by [`camera`].
pub fn view(rect: Rect) -> View {
    View {
        rect,
        pane: rect,
        index: 0,
        interactive: true,
        modal: false,
        response: Response::new(Default::default(), rect),
        over: PointerOver::Free,
        under: Slot(0),
        camera: camera(),
    }
}

/// A perspective camera 10 units up Z looking at the origin, its ground
/// the XZ plane stepped by 1 (its projection the identity: tests that
/// project points set their own).
pub fn camera() -> ViewCamera {
    ViewCamera {
        projection: Projection {
            clip_from_object: Default::default(),
            object_from_clip: Default::default(),
            backward: Vec3::new(0.0, 0.0, 1.0),
        },
        eye: Vec3::new(0.0, 0.0, 10.0),
        target: Vec3::ZERO,
        orthographic: false,
        ground: Ground {
            origin: Vec3::ZERO,
            normal: Vec3::new(0.0, 1.0, 0.0),
            step: 1.0,
        },
    }
}
