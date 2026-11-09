//! Lights as a data type: a light shaped as UsdLux shapes one
//! (render-api's standard [`Light`], unplaced: still at the identity).
//! Where a light is belongs to what holds it (an object's placement, a
//! file item's), never to the value. An image a light reads (a dome's, a
//! rect's color map, an IES profile) is an
//! [`ImageRef`](kinogaki_value::ImageRef) (a file, on disk or held, or an
//! Image node); the Texture prim a host decodes it to belongs to one scene,
//! never to a value.
//!
//! A value saves as its standard type and its prim's values by name, as an
//! engine's light object saves its prim ([`element::write_values`]):
//!
//! ```text
//! type "kinogaki.core.sphere_light"
//! def values "values" { radius, treatAsPoint, emission, color, … }
//! ```
use crate::Error;
use crate::codec::DataCodec;
use crate::data::Data;
use crate::element::{self, ElementReader, ElementWriter, Field};
use crate::standard::light;
use kinogaki_geometry::{Sampled, Xform};
use kinogaki_render_api::Declared;
use kinogaki_render_api::standard::{self, Light};

/// The element holding a light's values.
const VALUES: &str = "values";

/// `light` as a light value, when it is one: every value within its
/// type's limits (sizes above 0, colors finite: a component below 0 is a
/// color outside the space's gamut, as a file in a narrower space than the
/// scene's holds; an image naming an image), shaping only on a shape that
/// takes it, no Normalize on a dome. Its transform is dropped: where a
/// light is isn't the value's. The check of
/// [`standard::light`](crate::standard::light), which wraps every light
/// value, and of what [`LightCodec`] reads.
///
/// # Errors
/// A value its type refuses, or one its type doesn't hold.
pub(crate) fn light_value(light: Light) -> Result<Light, Error> {
    let light = Light {
        xform: Sampled::still(Xform::IDENTITY),
        ..light
    };
    let prim = light.declared();
    let fits = light.prim_type().schema().validate(&prim.values).is_ok();
    if !fits || Light::decode(&prim).as_ref() != Some(&light) {
        return Err(Error::invalid("invalid light"));
    }
    Ok(light)
}

/// The codec of light values: the light's type and its values by name.
#[derive(Clone, Copy, Debug, Default)]
pub struct LightCodec;

impl LightCodec {
    /// Writes `light` as its type and values.
    ///
    /// # Errors
    /// The element refusing a field.
    pub(crate) fn write_light(light: &Light, out: &mut dyn ElementWriter) -> Result<(), Error> {
        let prim = light.declared();
        out.set("type", Field::Text(prim.type_id.to_string()))?;
        element::write_values(&mut *out.child(VALUES, VALUES)?, &prim.values)
    }

    /// Reads the light [`write_light`](Self::write_light) wrote, strictly:
    /// a standard type, every value it has (one it lacks at its type's
    /// default, said: [`element::read_values`]), and a valid light
    /// ([`light_value`]).
    ///
    /// # Errors
    /// An unknown type, a value out of range, or an invalid light.
    pub(crate) fn read_light(e: &dyn ElementReader) -> Result<Light, Error> {
        let at = || e.location();
        let id = e.text("type")?;
        let ty = (standard::lights().into_iter())
            .find(|t| t.id.as_str() == id)
            .ok_or_else(|| Error::invalid(format_args!("{}: unknown light type {id:?}", at())))?;
        let values = element::read_values(&*e.required_child(VALUES)?, ty.schema().parameters())?;
        let light = Light::decode(&Declared::new(ty.id, values))
            .ok_or_else(|| Error::invalid(format_args!("{}: invalid light", at())))?;
        light_value(light).map_err(|_| Error::invalid(format_args!("{}: invalid light", at())))
    }
}

impl DataCodec for LightCodec {
    fn write(&self, data: &Data, out: &mut dyn ElementWriter) -> Result<(), Error> {
        let l = light()
            .get(data)
            .ok_or_else(|| Error::invalid("not a light"))?;
        Self::write_light(l, out)
    }

    fn read(&self, element: &dyn ElementReader) -> Result<Data, Error> {
        light().wrap(Self::read_light(element)?)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::MemoryElement;
    use kinogaki_geometry::EmbeddedFile;
    use kinogaki_geometry::image::{ImageColor, ImageFile, ImageSource};
    use kinogaki_render_api::standard::{DomeFormat, Emission, LightShape, Pole, Shadow, Shaping};
    use kinogaki_value::ImageRef;
    use std::sync::Arc;

    fn dome(image: Option<ImageRef>, ground: Option<[f32; 3]>) -> LightShape {
        LightShape::Dome {
            image,
            format: DomeFormat::LatLong,
            pole: Pole::Z,
            ground,
        }
    }

    /// One light of each shape, with values off their defaults.
    fn every_shape() -> Vec<Light> {
        let shapes = [
            LightShape::Rect {
                width: 2.0,
                height: 0.5,
                texture: Some(ImageRef::File(ImageSource::path("panel.png"))),
            },
            LightShape::Disk { radius: 0.25 },
            LightShape::Sphere {
                radius: 0.1,
                point: true,
            },
            LightShape::Cylinder {
                length: 0.8,
                radius: 0.015,
                line: false,
            },
            dome(None, Some([0.2, 0.1, 0.05])),
            dome(
                Some(ImageRef::File(ImageSource::path("sky/noon.exr"))),
                None,
            ),
            dome(
                Some(ImageRef::File(ImageSource::new(
                    ImageFile::Embedded(EmbeddedFile::new("noon.exr", vec![1, 2, 3])),
                    ImageColor::Data,
                ))),
                None,
            ),
            dome(Some(ImageRef::Node(Arc::from("/Library/Sky"))), None),
            LightShape::Distant { angle: 0.53 },
        ];
        (shapes.into_iter().enumerate())
            .map(|(i, shape)| {
                let mut light = Light {
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
                    color_temperature: 3200.0,
                    diffuse: 0.5,
                    specular: 2.0,
                    shadow: Shadow {
                        color: [0.5, 0.0, 0.0],
                        distance: 2.0,
                        ..Shadow::default()
                    },
                    camera: i % 2 == 1,
                    ..Light::default()
                };
                if light.shapes() {
                    light.normalize = i % 2 == 0;
                    light.shaping = Some(Shaping {
                        cone_angle: 25.0,
                        ies: Some(ImageRef::File(ImageSource::path("lamp.ies"))),
                        ..Shaping::default()
                    });
                }
                light
            })
            .collect()
    }

    #[test]
    fn every_shape_saves_and_reads_back() {
        for l in every_shape() {
            let l = light().check(l).unwrap();
            let mut e = MemoryElement::new("light");
            e.write(|w| LightCodec.write(&light().wrap(l.clone()).unwrap(), w))
                .unwrap();
            let back = e.read(|r| LightCodec.read(r)).unwrap();
            assert_eq!(light().get(&back), Some(&l));
        }
    }

    #[test]
    fn only_lights_in_range_are_light_values() {
        let rect = |width| LightShape::Rect {
            width,
            height: 1.0,
            texture: None,
        };
        let bad = [
            rect(0.0),
            LightShape::Disk { radius: f32::NAN },
            LightShape::Distant { angle: -1.0 },
            LightShape::Cylinder {
                length: 1.0,
                radius: -0.5,
                line: false,
            },
            dome(None, Some([f32::NAN, 0.0, 0.0])),
            dome(Some(ImageRef::File(ImageSource::path(" "))), None),
            dome(Some(ImageRef::Node(Arc::from("Sky"))), None),
        ];
        for shape in bad {
            let light = Light {
                shape,
                ..Light::default()
            };
            assert!(light_value(light).is_err());
        }
        let cone = |cone_angle, cone_softness| Light {
            shaping: Some(Shaping {
                cone_angle,
                cone_softness,
                ..Shaping::default()
            }),
            ..Light::default()
        };
        assert!(light().wrap(cone(190.0, 0.0)).is_err());
        assert!(light().wrap(cone(45.0, 1.5)).is_err());
        assert!(light().wrap(cone(45.0, 0.5)).is_ok());
        // Shaping a distant light, or normalizing a dome, is no light value.
        let shaped_sun = Light {
            shape: LightShape::Distant { angle: 0.53 },
            ..cone(45.0, 0.0)
        };
        let normalized_sky = Light {
            shape: dome(None, None),
            normalize: true,
            ..Light::default()
        };
        assert!(light().wrap(shaped_sun).is_err() && light().wrap(normalized_sky).is_err());
        let cold = Light {
            color_temperature: 0.0,
            ..Light::default()
        };
        let wild = Light {
            exposure: f32::INFINITY,
            ..Light::default()
        };
        assert!(light().wrap(cold).is_err() && light().wrap(wild).is_err());
        // Outside a space's gamut is a color still.
        let wide = Light {
            color: [-0.62, 1.14, -0.13],
            ..Light::default()
        };
        assert!(light().wrap(wide).is_ok());
    }

    #[test]
    fn a_value_is_unplaced_and_keeps_its_image_file() {
        let sky = dome(
            Some(ImageRef::File(ImageSource::path("sky/noon.exr"))),
            None,
        );
        let placed = Light {
            shape: sky.clone(),
            xform: Sampled::still(Xform::translation(kinogaki_geometry::Vec3::new(
                4.0, 5.0, 6.0,
            ))),
            ..Light::default()
        };
        let value = light().check(placed).unwrap();
        assert_eq!(
            (&value.xform, &value.shape),
            (&Sampled::still(Xform::IDENTITY), &sky)
        );
    }

    #[test]
    fn an_unknown_type_or_emission_is_refused() {
        let written = |ty: &str, values: &crate::Parameters| {
            let mut e = MemoryElement::new("light");
            e.write(|w| {
                w.set("type", Field::Text(ty.into()))?;
                element::write_values(&mut *w.child(VALUES, VALUES)?, values)
            })
            .unwrap();
            e.read(|r| LightCodec.read(r)).unwrap_err().to_string()
        };
        let mut prim = Light::default().declared();
        prim.values
            .insert("emission".into(), crate::Value::Choice("candle".into()));
        let refused = written(prim.type_id.as_str(), &prim.values);
        assert!(refused.contains("emission is out of range"), "{refused}");
        let refused = written("kinogaki.core.spot_light", &prim.values);
        assert!(refused.contains("unknown light type"), "{refused}");
    }
}
