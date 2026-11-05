//! [`Unconverted`]: a color service for checks that convert no color.
use kinogaki_studio_api::color::ColorService;

/// A color service taking every space for its working space, `ACEScg`:
/// conversions leave colors as they are.
/// For checks and tests about anything but color; color management is the
/// host's.
#[derive(Clone, Copy, Debug, Default)]
pub struct Unconverted;

impl ColorService for Unconverted {
    fn working(&self) -> &str {
        kinogaki_render_api::RENDER_SPACE
    }
    fn convert(
        &self,
        _: &str,
        _: &str,
        _: &mut [[f64; 3]],
    ) -> Result<(), kinogaki_studio_api::Error> {
        Ok(())
    }
    fn revision(&self) -> u64 {
        0
    }
}
