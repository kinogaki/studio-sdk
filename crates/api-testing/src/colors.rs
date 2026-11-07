//! [`Unconverted`]: a color service for checks that convert no color.
use kinogaki_studio_api::color::ColorService;
use kinogaki_studio_api::geometry::image::Decoding;

/// A color service taking every space for its working space, `ACEScg`:
/// conversions leave colors as they are, and every space has ACEScg's
/// primaries.
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
    fn decoding(&self, _: &str, _: &str) -> Option<Decoding> {
        Some(Decoding::IDENTITY)
    }
    fn primaries(&self, _: &str) -> Option<[f32; 8]> {
        Some([0.713, 0.293, 0.165, 0.830, 0.128, 0.044, 0.32168, 0.33767])
    }
    fn revision(&self) -> u64 {
        0
    }
}
