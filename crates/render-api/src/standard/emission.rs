//! Emission: where an emitter's spectrum comes from ([`Emission`]), and the
//! color it shows as in an RGB engine ([`Emission::color`]): linear ACEScg
//! ([`RENDER_SPACE`](crate::RENDER_SPACE)) at luminance 1.
//!
//! Spectra mean light relative to the equal-energy white E: the constant
//! spectrum 1 has XYZ (1, 1, 1) and shows as ACEScg (1, 1, 1). XYZ comes from
//! the CIE 1931 2° observer as Wyman, Sloan and Shirley fit it (JCGT 2013),
//! each channel normalized by its own sum over 360–830 nm at 1 nm, and reaches
//! ACEScg through [`FILM`]. An emitter's spectrum is normalized to luminance 1
//! before its intensity applies, so changing it changes the color and not the
//! brightness.
//!
//! The illuminants' colors are their published CIE tables (linear between
//! 5 nm nodes, zero past their ends) integrated so. The host checks every
//! number here against the spectral library a spectral engine renders with,
//! so one "D65" shows one color in every engine.
use std::sync::Arc;

/// A linear RGB or XYZ triple.
type Rgb = [f64; 3];

/// The shortest wavelength integrated, in nm.
const LAMBDA_MIN: f64 = 360.0;
/// The longest wavelength integrated, in nm.
const LAMBDA_MAX: f64 = 830.0;

/// XYZ relative to E to ACEScg, row-major: the inverse of ACEScg's (AP1
/// primaries') RGB to XYZ, Bradford-adapted from its white to E.
const FILM: [f64; 9] = [
    1.5942180931835674,
    -0.36784580741549633,
    -0.22637228576807109,
    -0.6905189684314205,
    1.6723035852656778,
    0.018215383165742522,
    0.011106827090932586,
    -0.006693547718058562,
    0.9955867206271256,
];

/// The published illuminants an emission may name: their CIE names (the keys
/// in files), labels, and colors at luminance 1, in the order a UI lists them.
pub const ILLUMINANTS: [(&str, &str, Rgb); 13] = [
    (
        "D50",
        "Daylight D50",
        [0.9828481613788387, 1.0213961395046878, 0.8254292720617514],
    ),
    (
        "D55",
        "Daylight D55",
        [0.9499206752996165, 1.0279654374360638, 0.921286066427995],
    ),
    (
        "D65",
        "Daylight D65",
        [0.9027829171306592, 1.0349969829742987, 1.0881481190052305],
    ),
    (
        "D75",
        "Daylight D75",
        [0.871060310051441, 1.0377555316333835, 1.2250920784764854],
    ),
    (
        "A",
        "Incandescent A",
        [1.2964657460373528, 0.9230542314584751, 0.3592515068845238],
    ),
    (
        "F2",
        "Fluorescent F2",
        [1.065759916008004, 0.9975025311514982, 0.6758897285355194],
    ),
    (
        "F7",
        "Fluorescent F7",
        [0.9049388560845797, 1.0341090702851148, 1.0875897144223192],
    ),
    (
        "F11",
        "Fluorescent F11",
        [1.0963236782381711, 0.9866344154287775, 0.6464975422597767],
    ),
    (
        "LED-B1",
        "LED B1",
        [1.342171108228664, 0.9049834191633667, 0.3380119034745688],
    ),
    (
        "LED-B2",
        "LED B2",
        [1.2750050100012307, 0.9282734590602328, 0.41001555893791436],
    ),
    (
        "LED-B3",
        "LED B3",
        [1.0932848904178918, 0.98537278697783, 0.6786743194522997],
    ),
    (
        "LED-B4",
        "LED B4",
        [0.9974853249719785, 1.0107344219817203, 0.8795113677504572],
    ),
    (
        "LED-B5",
        "LED B5",
        [0.9199882711895396, 1.0244576307761477, 1.1268257744080479],
    ),
];

/// Where an emitter's spectrum comes from. Each is normalized to
/// luminance 1 before its intensity applies, so changing it changes the
/// color and not the brightness.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Default)]
pub enum Emission {
    /// The emitter's color, upsampled to a smooth spectrum.
    #[default]
    Color,
    /// A blackbody at the emitter's color temperature.
    Temperature,
    /// A published illuminant's spectrum, by its CIE name
    /// ([`ILLUMINANTS`]).
    Illuminant(Arc<str>),
}

impl Emission {
    /// Its key in files (`color`, `temperature`, the CIE name).
    pub fn key(&self) -> &str {
        match self {
            Emission::Color => "color",
            Emission::Temperature => "temperature",
            Emission::Illuminant(name) => name,
        }
    }

    /// The emission of key `key`.
    pub fn of_key(key: &str) -> Option<Emission> {
        match key {
            "color" => Some(Emission::Color),
            "temperature" => Some(Emission::Temperature),
            _ => ILLUMINANTS
                .iter()
                .find(|(name, ..)| *name == key)
                .map(|(name, ..)| Emission::Illuminant(Arc::from(*name))),
        }
    }

    /// The color it shows as, linear ACEScg: `color` itself, a blackbody at
    /// `kelvin` (at least 1) at luminance 1, or the illuminant's at luminance
    /// 1 (none for a name [`ILLUMINANTS`] doesn't hold).
    pub fn color(&self, color: [f64; 3], kelvin: f64) -> Option<[f64; 3]> {
        match self {
            Emission::Color => Some(color),
            Emission::Temperature => Some(blackbody(kelvin.max(1.0))),
            Emission::Illuminant(name) => illuminant(name),
        }
    }
}

/// The color of the CIE illuminant named `name` (`D65`, `A`, `F11`,
/// `LED-B3`, …) at luminance 1; `None` for a name that is none of them.
pub fn illuminant(name: &str) -> Option<[f64; 3]> {
    (ILLUMINANTS.iter())
        .find(|(n, ..)| *n == name)
        .map(|&(.., rgb)| rgb)
}

/// The color of a blackbody at `kelvin` (positive) at luminance 1.
pub fn blackbody(kelvin: f64) -> [f64; 3] {
    let spectrum = |lambda: f64| planck(lambda, kelvin);
    let xyz = xyz_of(spectrum);
    let scale = if xyz[1] > 0.0 { 1.0 / xyz[1] } else { 0.0 };
    let m = &FILM;
    [
        (m[0] * xyz[0] + m[1] * xyz[1] + m[2] * xyz[2]) * scale,
        (m[3] * xyz[0] + m[4] * xyz[1] + m[5] * xyz[2]) * scale,
        (m[6] * xyz[0] + m[7] * xyz[1] + m[8] * xyz[2]) * scale,
    ]
}

/// Planck's law at `lambda` nm and `kelvin`, divided by its value at its
/// peak: with x = c2 / (λT) and the peak at x = 4.965114,
/// B(λ)/B(peak) = (λpeak/λ)^5 (e^4.965114 − 1) / (e^x − 1).
fn planck(lambda: f64, kelvin: f64) -> f64 {
    let peak = 2.8977721e6 / kelvin;
    let x = 1.4387769e7 / (lambda * kelvin);
    (peak / lambda).powi(5) * ((4.965114f64.exp() - 1.0) / (x.exp() - 1.0))
}

fn lobe(x: f64, mean: f64, below: f64, above: f64) -> f64 {
    let t = (x - mean) / if x < mean { below } else { above };
    (-0.5 * t * t).exp()
}

/// The CIE 1931 2° observer as Wyman, Sloan and Shirley's multi-lobe fit,
/// unnormalized.
fn cmf(lambda: f64) -> Rgb {
    let x = 1.056 * lobe(lambda, 599.8, 37.9, 31.0) + 0.362 * lobe(lambda, 442.0, 16.0, 26.7)
        - 0.065 * lobe(lambda, 501.1, 20.4, 26.2);
    let y = 0.821 * lobe(lambda, 568.8, 46.9, 40.5) + 0.286 * lobe(lambda, 530.9, 16.3, 31.1);
    let z = 1.217 * lobe(lambda, 437.0, 11.8, 36.0) + 0.681 * lobe(lambda, 459.0, 26.0, 13.8);
    [x, y, z]
}

/// The XYZ of spectrum `f` relative to E (each channel by its own sum),
/// summed at 1 nm over 360–830 nm.
fn xyz_of(f: impl Fn(f64) -> f64) -> Rgb {
    let nanometers = (0..=(LAMBDA_MAX - LAMBDA_MIN) as u32).map(|k| LAMBDA_MIN + f64::from(k));
    let (mut sum, mut unit) = ([0.0; 3], [0.0; 3]);
    for lambda in nanometers {
        let (c, v) = (cmf(lambda), f(lambda));
        for k in 0..3 {
            sum[k] += c[k] * v;
            unit[k] += c[k];
        }
    }
    [sum[0] / unit[0], sum[1] / unit[1], sum[2] / unit[2]]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_flat_spectrum_is_white() {
        let e = xyz_of(|_| 1.0);
        assert!(e.iter().all(|v| (v - 1.0).abs() < 1e-12), "{e:?}");
    }

    #[test]
    fn blackbodies_warm_below_white_and_cool_above() {
        // A blackbody peaks at 1 near Wien's wavelength.
        assert!((planck(2.8977721e6 / 5000.0, 5000.0) - 1.0).abs() < 1e-5);
        let warm = blackbody(2000.0);
        let cool = blackbody(20000.0);
        assert!(warm[0] > warm[1] && warm[1] > warm[2], "{warm:?}");
        assert!(cool[2] > cool[1] && cool[1] > cool[0], "{cool:?}");
    }

    #[test]
    fn every_illuminant_has_a_color_by_its_key() {
        for (name, ..) in ILLUMINANTS {
            let emission = Emission::of_key(name).expect("a key");
            assert_eq!(emission.color([0.0; 3], 0.0), illuminant(name));
        }
        assert_eq!(illuminant("D66"), None);
        let white = Emission::Color.color([1.0; 3], 0.0);
        assert_eq!(white, Some([1.0; 3]));
    }
}
