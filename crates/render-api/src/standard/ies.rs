//! IES light profiles (ANSI/IES LM-63, photometric type C), as UsdLux's
//! ShapingAPI reads them (`inputs:shaping:ies:file`): a profile's candela
//! by direction scale a light's emission, after its angle scale, divided by
//! the profile's power when it normalizes.
//!
//! A host reads a profile ([`Profile::parse`]) and sends it as a Texture
//! prim laid out as [`Profile::grid`] says, which engines sample:
//!
//! - texel (x, y) holds the candela at the vertical angle θ = (y + ½) / height
//!   × 180° and the horizontal angle φ = (x + ½) / width × 360°, read
//!   between the file's angles as UsdLux's reference renderer reads them
//!   (linearly in each; 0 past its vertical angles);
//! - channel 0 the candela (× the file's multiplier and ballast factors, as
//!   UsdLux's reference takes them), channel 1 the candela over the
//!   profile's power ([`Profile::power`]), for `ies:normalize`.
//!
//! An emission direction `e` in the light's space reads θ = acos(−e.z) (0:
//! straight down the light's −Z, the profile's nadir) and φ = atan2(−e.y,
//! −e.x) in [0°, 360°) ([`angles`]), as OpenUSD's hdEmbree does (which, its
//! authors say, matches Karma and RenderMan); the angle scale then moves θ
//! ([`scaled`]) before the profile is read.
use std::f64::consts::PI;

/// A parsed profile, in type C's angles: vertical 0° (down) to 180°,
/// horizontal 0° to 360°.
#[derive(Clone, Debug, PartialEq)]
pub struct Profile {
    /// Vertical angles, degrees, rising.
    vertical: Vec<f64>,
    /// Horizontal angles, degrees, rising, from 0 to 360.
    horizontal: Vec<f64>,
    /// Candela, one row a horizontal angle, one value a vertical angle.
    candela: Vec<Vec<f64>>,
}

/// A profile's texels, as engines sample them (the module docs).
#[derive(Clone, Debug, PartialEq)]
pub struct Grid {
    /// Texels across: horizontal angles.
    pub width: u32,
    /// Texels down: vertical angles.
    pub height: u32,
    /// Two values a texel, rows from θ near 0: the candela, then the candela
    /// over the power.
    pub texels: Vec<f32>,
}

/// The most texels a grid has along either side.
const MOST: usize = 720;

/// Whether two angles in degrees are the same.
fn same(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-3
}

impl Profile {
    /// The profile `text` holds.
    ///
    /// # Errors
    /// Why it isn't one: no `TILT=` line, numbers missing or not numbers,
    /// angles out of order, or a photometric type other than C.
    pub fn parse(text: &str) -> Result<Profile, String> {
        let tilt = text
            .find("TILT=")
            .ok_or_else(|| "no TILT= line: not an IES profile".to_owned())?;
        let rest = &text[tilt + 5..];
        let (tilt, rest) = rest.split_once('\n').unwrap_or((rest, ""));
        let mut numbers = rest
            .split(|c: char| c.is_whitespace() || c == ',')
            .filter(|s| !s.is_empty())
            .map(|s| {
                s.parse::<f64>()
                    .map_err(|_| format!("{s:?} isn't a number"))
            });
        let mut next = || {
            numbers
                .next()
                .unwrap_or(Err("the profile ends early".to_owned()))
        };
        if tilt.trim() == "INCLUDE" {
            // The lamp to luminaire geometry, then its angles and factors.
            next()?;
            let count = next()? as usize;
            for _ in 0..2 * count {
                next()?;
            }
        }
        let _lamps = next()?;
        let _lumens = next()?;
        let mut factor = next()?;
        let vertical = next()? as usize;
        let horizontal = next()? as usize;
        let kind = next()?;
        if kind != 1.0 {
            let name = if kind == 2.0 { "B" } else { "A" };
            return Err(format!("photometric type {name} isn't read, only C"));
        }
        for _ in 0..4 {
            next()?; // units, width, length, height
        }
        factor *= next()? * next()?; // ballast, ballast-lamp photometric
        next()?; // input watts
        if vertical == 0 || horizontal == 0 || vertical * horizontal > 1 << 20 {
            return Err(format!("{vertical} × {horizontal} angles"));
        }
        let mut angles = |n| (0..n).map(|_| next()).collect::<Result<Vec<f64>, String>>();
        let v = angles(vertical)?;
        let h = angles(horizontal)?;
        let mut candela = Vec::with_capacity(horizontal);
        for _ in 0..horizontal {
            let row: Result<Vec<f64>, String> = (0..vertical).map(|_| next()).collect();
            candela.push(row?.into_iter().map(|c| c * factor).collect());
        }
        let rising = |a: &[f64]| a.windows(2).all(|w| w[0] < w[1]);
        let finite = |a: &[f64]| a.iter().all(|x| x.is_finite());
        if !rising(&v) || !rising(&h) || !finite(&v) || !finite(&h) {
            return Err("its angles don't rise".to_owned());
        }
        Ok(Profile::full(v, h, candela))
    }

    /// Type C's angles as they are stored (a quarter, a half, a turn from
    /// 90°, one angle for a profile the same all round) as a whole turn.
    fn full(vertical: Vec<f64>, mut horizontal: Vec<f64>, mut candela: Vec<Vec<f64>>) -> Profile {
        if same(horizontal[0], 90.0) {
            horizontal.iter_mut().for_each(|h| *h -= 90.0);
        }
        if horizontal.len() == 1 {
            horizontal = vec![0.0, 360.0];
            candela.push(candela[0].clone());
        }
        // Mirrored about the last angle: 90° gives a half, 180° a turn.
        for mirror in [90.0, 180.0] {
            if same(*horizontal.last().expect("an angle"), mirror) {
                for i in (0..horizontal.len() - 1).rev() {
                    horizontal.push(2.0 * mirror - horizontal[i]);
                    candela.push(candela[i].clone());
                }
            }
        }
        if !same(*horizontal.last().expect("an angle"), 360.0) && same(horizontal[0], 0.0) {
            horizontal.push(360.0);
            candela.push(candela[0].clone());
        }
        Profile {
            vertical,
            horizontal,
            candela,
        }
    }

    /// The candela at vertical angle `theta` and horizontal angle `phi`
    /// (degrees): linear between the file's angles in each, 0 outside its
    /// vertical angles, the last vertical angle's at 180°.
    pub fn at(&self, theta: f64, phi: f64) -> f64 {
        let (v, h) = (&self.vertical, &self.horizontal);
        let phi = phi.rem_euclid(360.0);
        let span = |angles: &[f64], x: f64| {
            let i = angles.windows(2).position(|w| x >= w[0] && x < w[1])?;
            Some((i, (x - angles[i]) / (angles[i + 1] - angles[i])))
        };
        let vertical = if theta >= 180.0 && v.len() > 1 {
            Some((v.len() - 2, 1.0))
        } else if v.len() == 1 {
            same(theta, v[0]).then_some((0, 0.0))
        } else {
            span(v, theta)
        };
        let (Some((vi, dv)), Some((hi, dh))) = (vertical, span(h, phi).or(Some((0, 0.0)))) else {
            return 0.0;
        };
        let row = |r: &[f64]| match r.get(vi + 1) {
            Some(next) => r[vi] + (next - r[vi]) * dv,
            None => r[vi],
        };
        let a = row(&self.candela[hi]);
        match self.candela.get(hi + 1) {
            Some(next) => a + (row(next) - a) * dh,
            None => a,
        }
    }

    /// The mean candela over the sphere it covers (a hemisphere when its
    /// vertical angles span no more than about 90°), as OpenUSD's hdEmbree
    /// and Karma take a profile's power: what `ies:normalize` divides by.
    pub fn power(&self) -> f64 {
        let (v, h) = (&self.vertical, &self.horizontal);
        let mut sum = 0.0;
        for i in 0..h.len().saturating_sub(1) {
            for j in 0..v.len().saturating_sub(1) {
                let (dh, dv) = (
                    (h[i + 1] - h[i]).to_radians(),
                    (v[j + 1] - v[j]).to_radians(),
                );
                let c = &self.candela;
                let mean = (c[i][j] + c[i][j + 1] + c[i + 1][j] + c[i + 1][j + 1]) / 4.0;
                sum += dh * dv * (v[j].to_radians() + dv / 2.0).sin() * mean;
            }
        }
        let spread = v.last().copied().unwrap_or(0.0) - v[0];
        let sphere = spread.to_radians() > PI / 2.0 + 0.1;
        sum / (PI * if sphere { 4.0 } else { 2.0 })
    }

    /// Its texels, as engines sample them: a row for each of up to 720
    /// vertical angles (twice the file's, at least 180) and a column for
    /// each horizontal one (one for a profile the same all round).
    pub fn grid(&self) -> Grid {
        let round = self.candela.windows(2).all(|w| w[0] == w[1]);
        let height = (2 * self.vertical.len()).clamp(180, MOST);
        let width = if round {
            1
        } else {
            (2 * self.horizontal.len()).clamp(72, MOST)
        };
        let power = self.power();
        let mut texels = Vec::with_capacity(2 * width * height);
        for y in 0..height {
            let theta = (y as f64 + 0.5) / height as f64 * 180.0;
            for x in 0..width {
                let phi = (x as f64 + 0.5) / width as f64 * 360.0;
                let c = self.at(theta, phi);
                let normalized = if power > 0.0 { c / power } else { 0.0 };
                texels.extend([c as f32, normalized as f32]);
            }
        }
        Grid {
            width: width as u32,
            height: height as u32,
            texels,
        }
    }
}

/// The profile's angles (θ, φ, degrees) for emission direction `e` (unit,
/// in the light's space).
pub fn angles(e: [f64; 3]) -> (f64, f64) {
    let theta = (-e[2]).clamp(-1.0, 1.0).acos().to_degrees();
    let phi = (-e[1]).atan2(-e[0]).to_degrees().rem_euclid(360.0);
    (theta, phi)
}

/// Vertical angle `theta` (degrees) after ShapingAPI's angle scale `scale`
/// (0: as it is; above 0 about 0°, below 0 about 180°); none before 0°.
pub fn scaled(theta: f64, scale: f64) -> Option<f64> {
    let t = if scale > 0.0 {
        theta / scale
    } else if scale < 0.0 {
        (theta - 180.0) / -scale + 180.0
    } else {
        theta
    };
    (t >= 0.0).then_some(t)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A downlight, the same all round: 1000 cd straight down, 500 at 45°,
    /// none from 90°.
    const ROUND: &str = "IESNA:LM-63-2002\n[TEST] round\nTILT=NONE\n\
        1 1000 1 3 1 1 2 0 0 0\n1 1 50\n0 45 90\n0\n1000 500 0\n";

    #[test]
    fn a_round_profile_reads_the_same_all_round() {
        let p = Profile::parse(ROUND).unwrap();
        assert_eq!(p.horizontal, [0.0, 360.0]);
        assert_eq!(p.at(0.0, 10.0), 1000.0);
        assert_eq!(p.at(22.5, 200.0), 750.0);
        assert_eq!(p.at(120.0, 0.0), 0.0);
        let g = p.grid();
        assert_eq!((g.width, g.height), (1, 180));
        // Its first row is half a degree down.
        assert!((f64::from(g.texels[0]) - p.at(0.5, 180.0)).abs() < 1e-3);
    }

    #[test]
    fn multiplier_and_ballast_scale_the_candela() {
        let doubled = ROUND
            .replace("1 1000 1 3", "1 1000 2 3")
            .replace("1 1 50", "0.5 3 50");
        assert_eq!(Profile::parse(&doubled).unwrap().at(0.0, 0.0), 3000.0);
    }

    #[test]
    fn a_quarter_mirrors_into_a_whole_turn() {
        let quarter = "TILT=NONE\n1 -1 1 2 2 1 2 0 0 0\n1 1 0\n0 90\n0 90\n\
            100 0\n300 0\n";
        let p = Profile::parse(quarter).unwrap();
        assert_eq!(p.horizontal, [0.0, 90.0, 180.0, 270.0, 360.0]);
        assert_eq!(p.at(0.0, 90.0), 300.0);
        assert_eq!(p.at(0.0, 180.0), 100.0);
        assert_eq!(p.at(0.0, 270.0), 300.0);
        assert_eq!(p.at(0.0, 135.0), 200.0);
    }

    #[test]
    fn power_is_the_mean_candela_over_its_sphere() {
        // 100 cd everywhere on the lower hemisphere: its mean there is 100,
        // by the midpoint rule over 45° bands.
        let even = "TILT=NONE\n1 -1 1 3 1 1 2 0 0 0\n1 1 0\n0 45 90\n0\n100 100 100\n";
        let p = Profile::parse(even).unwrap();
        assert!((p.power() - 100.0).abs() < 3.0, "{}", p.power());
    }

    #[test]
    fn tilt_data_is_skipped_and_other_types_refused() {
        let tilted = ROUND.replace("TILT=NONE\n", "TILT=INCLUDE\n1\n2\n0 90\n1 0.5\n");
        assert_eq!(Profile::parse(&tilted).unwrap().at(0.0, 0.0), 1000.0);
        let b = ROUND.replace("1 1000 1 3 1 1", "1 1000 1 3 1 2");
        assert!(Profile::parse(&b).unwrap_err().contains("type B"));
        assert!(Profile::parse("no profile").is_err());
        assert!(Profile::parse("TILT=NONE\n1 2").is_err());
    }

    #[test]
    fn directions_read_down_the_lights_axis_first() {
        assert_eq!(angles([0.0, 0.0, -1.0]).0, 0.0);
        let (theta, phi) = angles([-1.0, 0.0, 0.0]);
        assert!((theta - 90.0).abs() < 1e-9 && phi.abs() < 1e-9);
        assert_eq!(scaled(30.0, 0.5), Some(60.0));
        assert_eq!(scaled(170.0, -0.5), Some(160.0));
        assert_eq!(scaled(30.0, -0.5), None);
    }
}
