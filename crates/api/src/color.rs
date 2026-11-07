//! Color management, the host's: one [`ColorService`] lent to formats
//! ([`Reading`](crate::Reading), [`Writing`](crate::Writing)), cooks
//! ([`CookCx::colors`](crate::CookCx::colors)), the UI and compiled
//! contexts' runners.
//!
//!
//! **One rule.** Authored colors (parameters, attributes, lights' and
//! settings' values, material constants) are linear in the project's
//! working space, wherever they are held: objects, Render Outputs, a
//! `Value::Color` in a body. Colors
//! reaching an engine (prims, materials, render settings) are linear
//! ACEScg (`kinogaki_render_api::RENDER_SPACE`), and engines output it: the host converts
//! each at that one boundary, as it sends it, through this service, and
//! sends them again when the working space or the configuration changes.
//! The same types carry both, so an adapter between authored kinds copies
//! channels as they are. Files carry colors in spaces of their own. A picture
//! is the one place past this rule: a Layout page composes display-referred
//! pieces, so its Text's color and a display-referred background are the target
//! display's code values, and a placed file keeps what it holds. The host
//! holds the color maths (its configuration's conversions and view
//! transforms, a file's space, display profiles) and lends the service to
//! formats (`Reading`, `Writing`), cooks (the project they read) and
//! runners; a package never carries a copy of it.
//!
//! Spaces are named as the host's configuration names them, or by its
//! aliases. A format names the spaces files mean by their interop names,
//! the Color Interop Forum's, which configurations alias: [`LIN_REC709`],
//! [`SRGB_TEXTURE`], [`SRGB_DISPLAY`]. A file format with names of its own
//! (MaterialX's `lin_rec709`, USD's `sRGB`) keeps them in its files and
//! maps them at its boundary.

use crate::{Diagnostic, Error};
use kinogaki_geometry::image::{Convert, Decoding, Encoding};
use std::fmt::Debug;

/// Linear Rec.709 (sRGB's primaries, linear light) by its interop name
/// (`lin_rec709_scene`): the space glTF's, FBX's, USD's and OBJ's
/// material colors and linear vertex colors are in.
pub const LIN_REC709: &str = "lin_rec709_scene";

/// sRGB-encoded Rec.709 by its interop name (`srgb_rec709_scene`): a
/// texture's colors as an 8-bit picture holds them (USD's `sRGB`
/// `sourceColorSpace`).
pub const SRGB_TEXTURE: &str = "srgb_rec709_scene";

/// The sRGB display's space by its interop name, which configurations
/// alias (the Color Interop Forum's `srgb_rec709_display`): what a file's
/// own sRGB tag says (a PNG's `sRGB` chunk), so a format tags such a file
/// with it, and what the host tags an image with for a file of sRGB code
/// values, so its writer writes that tag; an OBJ's vertex colors, which
/// are a display's.
pub const SRGB_DISPLAY: &str = "srgb_rec709_display";

/// `v`, a color a conversion gave, to twelve significant digits (eleven
/// places below 1; a zero stays zero, not a conversion's 10⁻¹⁸): what a
/// writer writes of a color it converted out of the working space, so a
/// color read and written back is the number it was, not one the
/// conversion there and back moved in its last bits.
///
/// ```
/// use kinogaki_studio_api::color::settled;
/// assert_eq!(settled(0.9999999999999963), 1.0);
/// assert_eq!(settled(123.456_789_012_345_6), 123.456_789_012);
/// ```
pub fn settled(v: f64) -> f64 {
    if !v.is_finite() {
        return v;
    }
    let scale = 10f64.powi(11 - v.abs().max(1.0).log10().floor() as i32);
    // + 0.0: never a negative zero.
    (v * scale).round() / scale + 0.0
}

/// The host's color management.
pub trait ColorService: Send + Sync + Debug {
    /// The project's working space, by its name in the configuration.
    fn working(&self) -> &str;

    /// Converts `rgb` in place from space `from` to space `to`.
    ///
    /// # Errors
    /// A space the configuration doesn't name, or no conversion between
    /// them; `rgb` is then unchanged.
    fn convert(&self, from: &str, to: &str, rgb: &mut [[f64; 3]]) -> Result<(), Error>;

    /// Converts `rgb`, single floats (a vertex color column, an image's
    /// run of pixels), in place from space `from` to space `to`, with no
    /// copy of the column: the conversion runs in doubles a color at a
    /// time.
    ///
    /// # Errors
    /// As [`convert`](Self::convert): a space the configuration doesn't
    /// name fails before anything changes.
    fn convert_f32(&self, from: &str, to: &str, rgb: &mut [[f32; 3]]) -> Result<(), Error> {
        self.convert(from, to, &mut [])?;
        let mut run = [[0.0f64; 3]; 256];
        for chunk in rgb.chunks_mut(run.len()) {
            let run = &mut run[..chunk.len()];
            for (d, c) in run.iter_mut().zip(chunk.iter()) {
                *d = c.map(f64::from);
            }
            self.convert(from, to, run)?;
            for (c, d) in chunk.iter_mut().zip(run.iter()) {
                *c = d.map(|v| v as f32);
            }
        }
        Ok(())
    }

    /// The conversion from space `from` to space `to` as a curve on each
    /// channel then a matrix ([`Decoding`]), when it is exactly that (an
    /// sRGB-encoded texture's into a scene-linear space, a linear space's
    /// into another): what a GPU runs after it fetches texels. None, by
    /// default, and for any other conversion (a log curve's, a look's).
    fn decoding(&self, from: &str, to: &str) -> Option<Decoding> {
        let _ = (from, to);
        None
    }

    /// The primaries of `space`, when it is linear light over primaries or
    /// a curve of it: red, green, blue and white, x then y each (OpenEXR's
    /// `chromaticities`). None by default.
    fn primaries(&self, space: &str) -> Option<[f32; 8]> {
        let _ = space;
        None
    }

    /// Converts `rgb` in place from space `from` to the working space.
    ///
    /// # Errors
    /// As [`convert`](Self::convert).
    fn to_working(&self, from: &str, rgb: &mut [[f64; 3]]) -> Result<(), Error> {
        self.convert(from, self.working(), rgb)
    }

    /// Converts `rgb` in place from the working space to space `to`.
    ///
    /// # Errors
    /// As [`convert`](Self::convert).
    fn working_to(&self, to: &str, rgb: &mut [[f64; 3]]) -> Result<(), Error> {
        self.convert(self.working(), to, rgb)
    }

    /// Whether colors in `space` are display-referred: a display's code values,
    /// which reach a picture as they are rather than through a view. No, by
    /// default.
    fn display_referred(&self, _space: &str) -> bool {
        false
    }

    /// Changes whenever a conversion would.
    fn revision(&self) -> u64;

    /// The spaces a working space can be (the config's scene-linear
    /// ones), by name; by default the working space alone.
    fn spaces(&self) -> Vec<String> {
        vec![self.working().to_owned()]
    }

    /// The displays a picture can be made for: standard dynamic range, with a
    /// tag a written file carries; by name, none by default.
    fn target_displays(&self) -> Vec<String> {
        Vec::new()
    }

    /// What its configuration got wrong (an unreadable config, a space it
    /// lacks), with what stands in; none by default.
    fn problem(&self) -> Option<Diagnostic> {
        None
    }
}

/// What reading a layer encoded `from` into space `to` makes of each run of
/// its pixels, as [`Image::converted`] reads them: colors made straight
/// when premultiplied, converted through `colors` (by its
/// [`decoding`](ColorService::decoding) where it has one, in single floats),
/// and premultiplied when `premultiply`; data left as stored. The one
/// conversion a reader taking an image into its own form runs, a tile at a
/// time. A layer already in `to` only has its alpha applied or taken out
/// where `premultiply` asks another form, and nothing done to it otherwise.
///
/// # Errors
/// A color layer whose tag names no space of the configuration (the host
/// resolves every tag it hands out), or a conversion the configuration
/// lacks.
///
/// [`Image::converted`]: kinogaki_geometry::image::Image::converted
pub fn converting<'a>(
    colors: &'a dyn ColorService,
    from: &'a Encoding,
    to: &'a str,
    premultiply: bool,
) -> Result<Box<Convert<'a>>, Error> {
    let space = match from {
        Encoding::Raw => return Ok(Box::new(|_| Ok(()))),
        _ => from.space().ok_or_else(|| {
            Error::invalid("an image's colors name no space of the configuration")
        })?,
    };
    colors.convert(space, to, &mut [])?;
    let premultiplied = from.premultiplied();
    let decoding = colors.decoding(space, to);
    if decoding == Some(Decoding::IDENTITY) {
        return Ok(match (premultiplied, premultiply) {
            (true, true) | (false, false) => Box::new(|_| Ok(())),
            (false, true) => Box::new(|run| {
                run.iter_mut().for_each(|p| *p = alpha_applied(*p));
                Ok(())
            }),
            (true, false) => Box::new(|run| {
                run.iter_mut().for_each(|p| *p = straight(*p));
                Ok(())
            }),
        });
    }
    let unmultiplied = move |p: [f32; 4]| match premultiplied {
        true => straight(p),
        false => p,
    };
    let finished = move |p: [f32; 4]| match premultiply {
        true => alpha_applied(p),
        false => p,
    };
    Ok(Box::new(move |run: &mut [[f32; 4]]| -> Result<(), Error> {
        match &decoding {
            Some(decoding) => run.iter_mut().for_each(|p| {
                let [r, g, b, a] = unmultiplied(*p);
                let [r, g, b] = decoding.apply([r, g, b]);
                *p = finished([r, g, b, a]);
            }),
            None => {
                let mut rgb: Vec<[f32; 3]> = (run.iter())
                    .map(|p| {
                        let [r, g, b, _] = unmultiplied(*p);
                        [r, g, b]
                    })
                    .collect();
                colors.convert_f32(space, to, &mut rgb)?;
                for (p, [r, g, b]) in run.iter_mut().zip(rgb) {
                    *p = finished([r, g, b, p[3]]);
                }
            }
        }
        Ok(())
    }))
}

/// A premultiplied pixel made straight where its alpha isn't 0.
fn straight([r, g, b, a]: [f32; 4]) -> [f32; 4] {
    match a > 0.0 {
        true => [r / a, g / a, b / a, a],
        false => [r, g, b, a],
    }
}

/// A straight pixel premultiplied.
fn alpha_applied([r, g, b, a]: [f32; 4]) -> [f32; 4] {
    [r * a, g * a, b * a, a]
}

#[cfg(test)]
mod tests {
    use super::{ColorService, Decoding, Encoding, Error, converting, settled};

    /// Spaces "a" and "b": b's light is half a's, by a decoding from a to b
    /// ("by matrix") or by conversion alone ("by conversion").
    #[derive(Debug)]
    struct Halves;

    impl ColorService for Halves {
        fn working(&self) -> &str {
            "a"
        }

        fn convert(&self, from: &str, to: &str, rgb: &mut [[f64; 3]]) -> Result<(), Error> {
            let by = match (from, to) {
                (f, t) if f == t => 1.0,
                ("a", "by matrix" | "by conversion") => 0.5,
                _ => return Err(Error::invalid("no such conversion")),
            };
            rgb.iter_mut().for_each(|c| *c = c.map(|v| v * by));
            Ok(())
        }

        fn decoding(&self, from: &str, to: &str) -> Option<Decoding> {
            let half = [[0.5, 0.0, 0.0], [0.0, 0.5, 0.0], [0.0, 0.0, 0.5]];
            match (from, to) {
                (f, t) if f == t => Some(Decoding::IDENTITY),
                ("a", "by matrix") => Some(Decoding {
                    matrix: half,
                    ..Decoding::IDENTITY
                }),
                _ => None,
            }
        }

        fn revision(&self) -> u64 {
            0
        }
    }

    /// `pixel`, encoded `from`, through the conversion into `to`.
    fn run(from: &Encoding, to: &str, premultiply: bool, pixel: [f32; 4]) -> [f32; 4] {
        let convert = converting(&Halves, from, to, premultiply).expect("a conversion");
        let mut run = [pixel];
        convert(&mut run).expect("converted");
        run[0]
    }

    #[test]
    fn a_layer_converts_straight_and_comes_out_as_asked() {
        let (straight, premultiplied) = (Encoding::color("a", false), Encoding::color("a", true));
        let pixel = [0.4, 0.2, 0.1, 0.5];
        assert_eq!(
            run(&straight, "a", false, pixel),
            pixel,
            "in its space already"
        );
        assert_eq!(run(&premultiplied, "a", true, pixel), pixel);
        assert_eq!(run(&straight, "a", true, pixel), [0.2, 0.1, 0.05, 0.5]);
        assert_eq!(run(&premultiplied, "a", false, pixel), [0.8, 0.4, 0.2, 0.5]);
        for to in ["by matrix", "by conversion"] {
            assert_eq!(
                run(&straight, to, false, pixel),
                [0.2, 0.1, 0.05, 0.5],
                "{to}"
            );
            assert_eq!(
                run(&premultiplied, to, true, pixel),
                [0.2, 0.1, 0.05, 0.5],
                "{to}"
            );
        }
        assert_eq!(
            run(&Encoding::Raw, "a", true, pixel),
            pixel,
            "data as stored"
        );
        assert!(converting(&Halves, &straight, "nowhere", false).is_err());
    }

    #[test]
    fn a_converted_color_settles_where_a_conversion_left_its_last_bits() {
        assert_eq!(settled(0.9999999999999963), 1.0);
        assert_eq!(settled(1.0000000000000038), 1.0);
        let zero = settled(-3.4694469519500004e-18);
        assert!(zero == 0.0 && zero.is_sign_positive(), "{zero}");
        assert_eq!(settled(0.25), 0.25);
        assert_eq!(settled(123.456_789_012_345_6), 123.456_789_012);
    }
}
