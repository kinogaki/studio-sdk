//! [`Camera`]: a camera after UsdGeomCamera.
use kinogaki_geometry::{Sampled, Xform};

/// How a camera projects.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Projection {
    /// Rays from the camera's position.
    #[default]
    Perspective,
    /// Parallel rays across the film back.
    Orthographic,
}

/// A camera after UsdGeomCamera. It looks down its local -Z with +Y up, and
/// its film back fits the image horizontally.
#[derive(Clone, Debug, PartialEq)]
pub struct Camera {
    /// The projection.
    pub projection: Projection,
    /// The lens's focal length, in mm.
    pub focal_length: f32,
    /// The film back's width, in mm; orthographic: the view's width, a
    /// length in mm on the scene's scale (the render settings'
    /// [`units`](crate::SceneSettings::units) turn mm into scene units).
    pub horizontal_aperture: f32,
    /// The film back's height, in mm (orthographic: the view's height).
    pub vertical_aperture: f32,
    /// The film back's horizontal shift, in mm.
    pub horizontal_aperture_offset: f32,
    /// The film back's vertical shift, in mm.
    pub vertical_aperture_offset: f32,
    /// Near and far clipping distances, in scene units.
    pub clipping_range: [f32; 2],
    /// The lens's f-number; 0 is a pinhole (no depth of field).
    pub f_stop: f32,
    /// The distance in focus, in scene units.
    pub focus_distance: f32,
    /// When the shutter opens and closes, in frames relative to the frame
    /// rendered (`shutter:open`, `shutter:close`); equal: no motion blur.
    pub shutter: [f32; 2],
    /// Camera to world across the shutter.
    pub xform: Sampled<Xform>,
}

impl Camera {
    /// Whether its shutter stays open a while: what blurs motion.
    pub fn blurs(&self) -> bool {
        self.shutter[1] > self.shutter[0]
    }
}

impl Default for Camera {
    fn default() -> Camera {
        Camera {
            projection: Projection::Perspective,
            focal_length: 50.0,
            horizontal_aperture: 36.0,
            vertical_aperture: 24.0,
            horizontal_aperture_offset: 0.0,
            vertical_aperture_offset: 0.0,
            clipping_range: [0.1, 10_000.0],
            f_stop: 0.0,
            focus_distance: 5.0,
            shutter: [0.0, 0.0],
            xform: Sampled::still(Xform::IDENTITY),
        }
    }
}
