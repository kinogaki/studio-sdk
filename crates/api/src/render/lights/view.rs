//! How a [`DeclaredKind`](super::DeclaredKind)'s lights show in the
//! viewport: their line glyphs (an area light's shape with an arrow down its
//! −Z, a dome's globe, a distant light's parallel rays, a spot's cone,
//! another type a ring) and their handles (an area light's size, a spot's
//! cone and softness, a distant light's angle), by their standard shapes.
//! Studio places, picks, frames and drags them.
use super::kind::DeclaredBody;
use crate::cook::{Handle, HandleKind, Scalar, Vector};
use crate::facets::{GlyphScale, Glyphs};
use crate::{ObjectBody, Vec3};
use kinogaki_render_api::Declared;
use kinogaki_render_api::standard::{self, LightShape};

/// Segments around a circle.
const CIRCLE: usize = 32;
/// How far short of a half space a cone draws, in degrees.
const WIDEST: f64 = 80.0;

const X: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
const Z: Vec3 = Vec3::new(0.0, 0.0, 1.0);
const DOWN: Vec3 = Vec3::new(0.0, 0.0, -1.0);

/// A circle of `radius` about `center` in the plane of `u` and `v`.
fn circle(out: &mut Vec<[Vec3; 2]>, center: Vec3, u: Vec3, v: Vec3, radius: f64) {
    let at = |i: usize| {
        let a = i as f64 / CIRCLE as f64 * std::f64::consts::TAU;
        center + (u * a.cos() + v * a.sin()) * radius
    };
    out.extend((0..CIRCLE).map(|i| [at(i), at(i + 1)]));
}

/// An arrow `length` long from `from` down `along` (unit), its head spread
/// along `side`.
fn arrow(out: &mut Vec<[Vec3; 2]>, from: Vec3, along: Vec3, side: Vec3, length: f64) {
    let tip = from + along * length;
    let back = tip - along * (0.25 * length);
    out.push([from, tip]);
    out.push([tip, back + side * (0.12 * length)]);
    out.push([tip, back - side * (0.12 * length)]);
}

/// A light kind's glyphs.
pub(super) struct LightGlyphs;

impl Glyphs for LightGlyphs {
    fn lines(&self, body: &dyn ObjectBody, scale: GlyphScale) -> Vec<[Vec3; 2]> {
        let Some(light) = body.downcast_ref::<DeclaredBody>() else {
            return Vec::new();
        };
        let size = scale.size;
        let mut out = Vec::new();
        match standard::Light::decode(&light.prim).map(|l| l.shape) {
            Some(LightShape::Rect { width, height }) => {
                let (w, h) = (f64::from(width) * 0.5, f64::from(height) * 0.5);
                let corners =
                    [[-w, -h], [w, -h], [w, h], [-w, h]].map(|[x, y]| Vec3::new(x, y, 0.0));
                out.extend((0..4).map(|i| [corners[i], corners[(i + 1) % 4]]));
                arrow(&mut out, Vec3::ZERO, DOWN, X, size);
            }
            Some(LightShape::Disk { radius }) => {
                circle(&mut out, Vec3::ZERO, X, Y, f64::from(radius));
                arrow(&mut out, Vec3::ZERO, DOWN, X, size);
            }
            Some(LightShape::Dome { .. }) => {
                circle(&mut out, Vec3::ZERO, X, Z, size);
                circle(&mut out, Vec3::ZERO, X, Y, size);
                circle(&mut out, Vec3::ZERO, Y, Z, size);
                // Its top (+Y, where a sky image's top goes).
                arrow(&mut out, Y * size, Y, X, size * 0.4);
            }
            Some(LightShape::Distant { .. }) => {
                // A disk with parallel rays down −Z: the sun's direction.
                let r = size * 0.25;
                circle(&mut out, Vec3::ZERO, X, Y, r);
                for side in [Vec3::ZERO, X * r, X * -r, Y * r, Y * -r] {
                    arrow(&mut out, side, DOWN, X, size);
                }
            }
            Some(LightShape::Spot {
                radius, cone_angle, ..
            }) => {
                // Its sphere's outline, and the cone `size` deep (capped short
                // of a half space).
                circle(&mut out, Vec3::ZERO, X, Y, f64::from(radius));
                let rim = size * f64::from(cone_angle).clamp(0.0, WIDEST).to_radians().tan();
                let base = DOWN * size;
                circle(&mut out, base, X, Y, rim);
                for edge in [X, X * -1.0, Y, Y * -1.0] {
                    out.push([Vec3::ZERO, base + edge * rim]);
                }
            }
            // Another type, with no place or direction: a ring with a lit
            // edge, two circles, one wider.
            None => {
                let r = size * 0.25;
                circle(&mut out, Vec3::ZERO, X, Y, r);
                circle(&mut out, Vec3::ZERO, X, Y, r * 1.3);
            }
        }
        out
    }
}

/// A light `prim`'s handles: an area light's size, a spot's cone and
/// softness, a distant light's angle; none for a dome or another type.
pub(super) fn handles(prim: &Declared) -> Vec<Handle> {
    let origin = || Vector::fixed(Vec3::ZERO);
    let angle = |label: &str, name: &str, within: Option<&str>| {
        Handle::new(
            label,
            HandleKind::Angle {
                angle: name.into(),
                origin: origin(),
                axis: Vector::fixed(DOWN),
                length: 1.0,
                within: within.map(Scalar::from),
            },
        )
    };
    match standard::Light::decode(prim).map(|l| l.shape) {
        Some(LightShape::Rect { .. }) => {
            let size = Vector::components("width", "height", 0.0);
            let size = HandleKind::Box {
                center: origin(),
                size,
            };
            vec![Handle::new("Size", size).plane(2).range(1e-6, 1e12)]
        }
        Some(LightShape::Disk { .. }) => {
            let radius = HandleKind::Radius {
                radius: "radius".into(),
                center: origin(),
            };
            vec![Handle::new("Radius", radius).plane(2).range(1e-6, 1e12)]
        }
        Some(LightShape::Spot { .. }) => vec![
            angle("Cone angle", "coneAngle", None).range(0.0, 90.0),
            angle("Cone softness", "coneSoftness", Some("coneAngle")).range(0.0, 1.0),
        ],
        Some(LightShape::Distant { .. }) => vec![angle("Angle", "angle", None).range(0.0, 180.0)],
        Some(LightShape::Dome { .. }) | None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ObjectKindId, Units, Value};
    use kinogaki_render_api::PrimType;

    fn light(ty: &PrimType, values: &[(&str, Value)]) -> DeclaredBody {
        let mut held = ty.schema().values();
        held.extend(
            values
                .iter()
                .map(|(name, v)| ((*name).to_owned(), v.clone())),
        );
        DeclaredBody {
            kind: ObjectKindId::declared("acme.ray.light"),
            prim: Declared::new(ty.id, held),
        }
    }

    /// The standard light types: area, dome, distant and spot.
    fn types() -> [PrimType; 4] {
        standard::lights().try_into().expect("four standard lights")
    }

    fn unit(size: f64) -> GlyphScale {
        GlyphScale {
            size,
            units: Units::default(),
        }
    }

    fn extent(lines: &[[Vec3; 2]]) -> [f64; 3] {
        let mut m = [0.0f64; 3];
        for p in lines.iter().flatten() {
            for (k, v) in [p.x, p.y, p.z].into_iter().enumerate() {
                m[k] = m[k].max(v.abs());
            }
        }
        m
    }

    #[test]
    fn glyphs_follow_the_light_shape() {
        let [area, dome, _, spot] = types();
        let rect = light(
            &area,
            &[
                ("width", Value::Number(4.0)),
                ("height", Value::Number(2.0)),
            ],
        );
        // Its outline, and an arrow one grid step down −Z.
        assert_eq!(
            extent(&LightGlyphs.lines(&rect, unit(1.0))),
            [2.0, 1.0, 1.0]
        );
        // A spot's cone opens as its angle: 45° is as wide as it is deep.
        let spot = light(&spot, &[("coneAngle", Value::Number(45.0))]);
        let [x, _, z] = extent(&LightGlyphs.lines(&spot, unit(1.0)));
        assert!((x - 1.0).abs() < 1e-9 && (z - 1.0).abs() < 1e-9);
        // A dome is a globe one grid step round, sized by the grid.
        let dome = light(&dome, &[]);
        assert!((extent(&LightGlyphs.lines(&dome, unit(10.0)))[0] - 10.0).abs() < 1e-9);
    }

    #[test]
    fn handles_bind_to_the_light_shape() {
        let [area, dome, distant, spot] = types();
        let names = |body: &DeclaredBody| -> Vec<Vec<String>> {
            (handles(&body.prim).iter())
                .map(|h| h.parameters().into_iter().map(str::to_owned).collect())
                .collect()
        };
        assert_eq!(
            names(&light(&spot, &[])),
            [vec!["coneAngle"], vec!["coneSoftness", "coneAngle"]]
        );
        assert_eq!(names(&light(&area, &[])), [vec!["width", "height"]]);
        let disk = light(&area, &[("shape", standard::choice("disk"))]);
        assert_eq!(names(&disk), [vec!["radius"]]);
        assert_eq!(names(&light(&distant, &[])), [vec!["angle"]]);
        assert!(handles(&light(&dome, &[]).prim).is_empty());
    }
}
