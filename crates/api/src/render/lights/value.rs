//! Lights as a data type: a light shaped as UsdLux shapes one
//! (render-api's standard [`Light`], unplaced: still at the identity).
//! Where a light is belongs to what holds it (an object's placement, a
//! file item's), never to the value. A dome's image is its file (an
//! asset path, as typed); the Texture prim a host decodes it to belongs
//! to one scene, never to a value.
//!
//! ```text
//! type (RectLight, DiskLight, DomeLight, DistantLight, SphereLight),
//! width, height (RectLight), radius (DiskLight, SphereLight),
//! groundColor, texture (DomeLight, each when it has one),
//! angle (DistantLight),
//! coneAngle, coneSoftness (SphereLight), emission, color, intensity,
//! exposure, normalize, colorTemperature, visible
//! ```
use crate::Error;
use crate::codec::DataCodec;
use crate::data::Data;
use crate::element::{ElementReader, ElementWriter, Field};
use crate::standard::light;
use kinogaki_geometry::{Sampled, Xform};
use kinogaki_render_api::standard::{Emission, Light, Shape};

/// The largest size, intensity or temperature a light takes.
const MAX_VALUE: f32 = 1e12;

/// `light` as a light value, when it is one: every number finite and
/// within range, sizes and radii positive, an angle and intensity of at
/// least 0, a color temperature above 0, a cone angle of 0 to 180 degrees
/// and a softness of 0 to 1, colors finite (a component below 0 is a
/// color outside the space's gamut, as a file in a narrower space than
/// the scene's holds), and a dome's image a file whose path isn't blank.
/// Its transform is dropped: where a light is isn't the value's. Make
/// every light value with it: [`LightCodec`] reads only what it accepts.
///
/// # Errors
/// A value out of range.
pub fn light_value(light: Light) -> Result<Light, Error> {
    let within = |v: f32| v.is_finite() && v.abs() <= MAX_VALUE;
    let positive = |v: f32| within(v) && v > 0.0;
    let at_least_0 = |v: f32| within(v) && v >= 0.0;
    let shape = match &light.shape {
        Shape::Rect { width, height } => positive(*width) && positive(*height),
        Shape::Disk { radius } => positive(*radius),
        Shape::Dome { image, ground } => {
            ground.is_none_or(|g| g.into_iter().all(within))
                && image.as_ref().is_none_or(|path| !path.trim().is_empty())
        }
        Shape::Distant { angle } => at_least_0(*angle) && *angle <= 180.0,
        Shape::Spot {
            radius,
            cone_angle,
            cone_softness,
        } => {
            positive(*radius)
                && (0.0..=180.0).contains(cone_angle)
                && (0.0..=1.0).contains(cone_softness)
        }
    };
    let ok = shape
        && light.color.into_iter().all(within)
        && at_least_0(light.intensity)
        && within(light.exposure)
        && positive(light.color_temperature);
    if !ok {
        return Err(Error::invalid("invalid light"));
    }
    Ok(Light {
        xform: Sampled::still(Xform::IDENTITY),
        ..light
    })
}

/// The codec of light values: the light as UsdLux's fields.
#[derive(Clone, Copy, Debug, Default)]
pub struct LightCodec;

impl LightCodec {
    /// Writes `light` as fields.
    ///
    /// # Errors
    /// The element refusing a field.
    pub(crate) fn write_light(light: &Light, out: &mut dyn ElementWriter) -> Result<(), Error> {
        let float = |v: f32| Field::Float(f64::from(v));
        let ty = match &light.shape {
            Shape::Rect { width, height } => {
                out.set("width", float(*width))?;
                out.set("height", float(*height))?;
                "RectLight"
            }
            Shape::Disk { radius } => {
                out.set("radius", float(*radius))?;
                "DiskLight"
            }
            Shape::Dome { image, ground } => {
                if let Some(g) = ground {
                    out.set("groundColor", Field::F32s(g.to_vec()))?;
                }
                if let Some(path) = image {
                    out.set("texture", Field::Text(path.to_string()))?;
                }
                "DomeLight"
            }
            Shape::Distant { angle } => {
                out.set("angle", float(*angle))?;
                "DistantLight"
            }
            Shape::Spot {
                radius,
                cone_angle,
                cone_softness,
            } => {
                out.set("radius", float(*radius))?;
                out.set("coneAngle", float(*cone_angle))?;
                out.set("coneSoftness", float(*cone_softness))?;
                "SphereLight"
            }
        };
        out.set("type", Field::Text(ty.into()))?;
        out.set("emission", Field::Text(light.emission.key().into()))?;
        out.set("color", Field::F32s(light.color.to_vec()))?;
        out.set("intensity", float(light.intensity))?;
        out.set("exposure", float(light.exposure))?;
        out.set("normalize", Field::Bool(light.normalize))?;
        out.set("colorTemperature", float(light.color_temperature))?;
        out.set("visible", Field::Bool(light.camera))
    }

    /// Reads the light [`write_light`](Self::write_light) wrote, strictly:
    /// every field its type has, a known type and emission, and a valid
    /// light ([`light_value`]).
    ///
    /// # Errors
    /// A field missing or of another kind, or an invalid light.
    pub(crate) fn read_light(e: &dyn ElementReader) -> Result<Light, Error> {
        let at = || e.location();
        let scalar = |name| e.float(name).map(|v| v as f32);
        let color = |name| match e.f32s(name)? {
            &[r, g, b] => Ok([r, g, b]),
            _ => Err(Error::invalid(format_args!(
                "{}: {name} isn't three numbers",
                at()
            ))),
        };
        let shape = match e.text("type")? {
            "RectLight" => Shape::Rect {
                width: scalar("width")?,
                height: scalar("height")?,
            },
            "DiskLight" => Shape::Disk {
                radius: scalar("radius")?,
            },
            "DomeLight" => Shape::Dome {
                image: match e.field("texture") {
                    Some(_) => Some(e.text("texture")?.into()),
                    None => None,
                },
                ground: match e.field("groundColor") {
                    Some(_) => Some(color("groundColor")?),
                    None => None,
                },
            },
            "DistantLight" => Shape::Distant {
                angle: scalar("angle")?,
            },
            "SphereLight" => Shape::Spot {
                radius: scalar("radius")?,
                cone_angle: scalar("coneAngle")?,
                cone_softness: scalar("coneSoftness")?,
            },
            other => {
                return Err(Error::invalid(format_args!(
                    "{}: unknown light type {other:?}",
                    at()
                )));
            }
        };
        let emission = e.text("emission")?;
        let emission = Emission::of_key(emission).ok_or_else(|| {
            Error::invalid(format_args!("{}: unknown emission {emission:?}", at()))
        })?;
        let light = Light {
            shape,
            emission,
            color: color("color")?,
            intensity: scalar("intensity")?,
            exposure: scalar("exposure")?,
            normalize: e.bool("normalize")?,
            color_temperature: scalar("colorTemperature")?,
            xform: Sampled::still(Xform::IDENTITY),
            camera: e.bool("visible")?,
        };
        light_value(light).map_err(|_| Error::invalid(format_args!("{}: invalid light", at())))
    }
}

impl DataCodec for LightCodec {
    fn write(
        &self,
        data: &Data,
        _base: Option<&Data>,
        out: &mut dyn ElementWriter,
    ) -> Result<(), Error> {
        let l = light()
            .get(data)
            .ok_or_else(|| Error::invalid("not a light"))?;
        Self::write_light(l, out)
    }

    fn read(&self, element: &dyn ElementReader, _base: Option<&Data>) -> Result<Data, Error> {
        Ok(light().wrap(Self::read_light(element)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::MemoryElement;
    use std::sync::Arc;

    /// One light of each shape, with values off their defaults.
    fn every_shape() -> Vec<Light> {
        let shapes = [
            Shape::Rect {
                width: 2.0,
                height: 0.5,
            },
            Shape::Disk { radius: 0.25 },
            Shape::Dome {
                image: None,
                ground: Some([0.2, 0.1, 0.05]),
            },
            Shape::Dome {
                image: Some(Arc::from("sky/noon.exr")),
                ground: None,
            },
            Shape::Distant { angle: 0.53 },
            Shape::Spot {
                radius: 0.1,
                cone_angle: 30.0,
                cone_softness: 0.2,
            },
        ];
        (shapes.into_iter().enumerate())
            .map(|(i, shape)| Light {
                shape,
                emission: [
                    Emission::Color,
                    Emission::Temperature,
                    Emission::Illuminant(Arc::from("D50")),
                ][i % 3]
                    .clone(),
                color: [0.9, 0.5, 0.25],
                intensity: 3.5,
                exposure: -1.0,
                normalize: i % 2 == 0,
                color_temperature: 3200.0,
                camera: i % 2 == 1,
                ..Light::default()
            })
            .collect()
    }

    #[test]
    fn every_shape_saves_and_reads_back() {
        for l in every_shape() {
            let l = light_value(l).unwrap();
            let mut e = MemoryElement::new("light");
            e.write(|w| LightCodec.write(&light().wrap(l.clone()), None, w))
                .unwrap();
            let back = e.read(|r| LightCodec.read(r, None)).unwrap();
            assert_eq!(light().get(&back), Some(&l));
        }
    }

    #[test]
    fn only_lights_in_range_are_light_values() {
        let bad = [
            Shape::Rect {
                width: 0.0,
                height: 1.0,
            },
            Shape::Disk { radius: f32::NAN },
            Shape::Distant { angle: -1.0 },
            Shape::Spot {
                radius: 1.0,
                cone_angle: 190.0,
                cone_softness: 0.0,
            },
            Shape::Spot {
                radius: 1.0,
                cone_angle: 45.0,
                cone_softness: 1.5,
            },
            Shape::Dome {
                image: None,
                ground: Some([f32::NAN, 0.0, 0.0]),
            },
            Shape::Dome {
                image: Some(Arc::from(" ")),
                ground: None,
            },
        ];
        for shape in bad {
            assert!(
                light_value(Light {
                    shape,
                    ..Light::default()
                })
                .is_err()
            );
        }
        let cold = Light {
            color_temperature: 0.0,
            ..Light::default()
        };
        let wild = Light {
            exposure: f32::INFINITY,
            ..Light::default()
        };
        assert!(light_value(cold).is_err() && light_value(wild).is_err());
        // Outside a space's gamut is a color still.
        let wide = Light {
            color: [-0.62, 1.14, -0.13],
            ..Light::default()
        };
        assert!(light_value(wide).is_ok());
    }

    #[test]
    fn a_value_is_unplaced_and_keeps_its_image_file() {
        let sky = Shape::Dome {
            image: Some(Arc::from("sky/noon.exr")),
            ground: None,
        };
        let placed = Light {
            shape: sky.clone(),
            xform: Sampled::still(Xform::translation(kinogaki_geometry::Vec3::new(
                4.0, 5.0, 6.0,
            ))),
            ..Light::default()
        };
        let value = light_value(placed).unwrap();
        assert_eq!(
            (&value.xform, &value.shape),
            (&Sampled::still(Xform::IDENTITY), &sky)
        );
    }

    #[test]
    fn an_unknown_type_or_emission_is_refused() {
        let mut e = MemoryElement::new("light");
        e.write(|w| LightCodec::write_light(&Light::default(), w))
            .unwrap();
        e.write(|w| w.set("emission", Field::Text("candle".into())))
            .unwrap();
        let refused = e.read(|r| LightCodec.read(r, None)).unwrap_err();
        assert!(
            refused.to_string().contains("unknown emission"),
            "{refused}"
        );
        e.write(|w| w.set("type", Field::Text("CylinderLight".into())))
            .unwrap();
        let refused = e.read(|r| LightCodec.read(r, None)).unwrap_err();
        assert!(
            refused.to_string().contains("unknown light type"),
            "{refused}"
        );
    }
}
