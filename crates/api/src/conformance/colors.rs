//! [`Unconverted`]: a color service for checks that convert no color.
use kinogaki_render_api::color::{ColorService, ViewChoice, ViewTransform};

/// A color service taking every space for its working space, `ACEScg`:
/// conversions leave colors as they are, and it has no view transforms.
/// For checks and tests about anything but color; color management is the
/// host's.
#[derive(Clone, Copy, Debug, Default)]
pub struct Unconverted;

impl ColorService for Unconverted {
    fn working(&self) -> &str {
        kinogaki_render_api::RENDER_SPACE
    }
    fn convert(&self, _: &str, _: &str, _: &mut [[f64; 3]]) -> Result<(), String> {
        Ok(())
    }
    fn view_transform(&self, _: &ViewChoice, _: &str, _: u32, _: u32) -> Option<ViewTransform> {
        None
    }
    fn revision(&self) -> u64 {
        0
    }
}
