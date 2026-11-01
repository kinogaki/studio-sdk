//! Cameras as a data type: a lens shaped as UsdGeomCamera shapes one
//! (render-api's [`Camera`], unplaced). It looks down its local −Z with
//! +Y up; its lens and film back are millimetres, its clipping range and
//! focus distance project units. Where a camera is belongs to what holds
//! it (an object's placement, a file item's), never to the value.
//!
//! ```text
//! projection, focalLength, horizontalAperture, verticalAperture,
//! horizontalApertureOffset, verticalApertureOffset, clippingRange,
//! focusDistance, fStop
//! ```
use super::camera;
use crate::codec::DataCodec;
use crate::data::Data;
use crate::element::{ElementReader, ElementWriter, Field};
use crate::error::Error;
use kinogaki_render_api::{Camera, IDENTITY, Projection};

/// The largest lens, film or distance value a camera takes.
const MAX_VALUE: f32 = 1e12;

/// The lens of `camera`, when it is one: positive focal length, film back
/// and near distance, a far distance beyond the near one, a focus distance
/// and f-number of at least 0, every value finite and within range. Its
/// transform and scale are dropped (the identity, a metre a unit): what a
/// camera value holds.
///
/// # Errors
/// A value out of range.
pub fn lens(camera: Camera) -> Result<Camera, Error> {
    let [near, far] = camera.clipping_range;
    let positive = [
        camera.focal_length,
        camera.horizontal_aperture,
        camera.vertical_aperture,
        near,
    ];
    let others = [
        camera.horizontal_aperture_offset,
        camera.vertical_aperture_offset,
        far,
        camera.focus_distance,
        camera.f_stop,
    ];
    let ok = positive.iter().all(|v| *v > 0.0)
        && (positive.iter().chain(&others)).all(|v| v.is_finite() && v.abs() <= MAX_VALUE)
        && far > near
        && camera.focus_distance >= 0.0
        && camera.f_stop >= 0.0;
    if !ok {
        return Err(Error::invalid("invalid camera"));
    }
    Ok(Camera {
        transform: IDENTITY,
        mm_per_unit: Camera::default().mm_per_unit,
        ..camera
    })
}

/// How a projection is spelt.
fn projection_name(p: Projection) -> &'static str {
    match p {
        Projection::Perspective => "Perspective",
        Projection::Orthographic => "Orthographic",
    }
}

/// The codec of camera values: the lens as UsdGeomCamera's fields. Core's
/// Camera objects save their lens with it too.
#[derive(Clone, Copy, Debug, Default)]
pub struct CameraCodec;

impl CameraCodec {
    /// Writes `camera`'s lens as fields.
    ///
    /// # Errors
    /// The element refusing a field.
    pub fn write_lens(camera: &Camera, out: &mut dyn ElementWriter) -> Result<(), Error> {
        let c = camera;
        out.set(
            "projection",
            Field::Text(projection_name(c.projection).into()),
        )?;
        let scalars = [
            ("focalLength", c.focal_length),
            ("horizontalAperture", c.horizontal_aperture),
            ("verticalAperture", c.vertical_aperture),
            ("horizontalApertureOffset", c.horizontal_aperture_offset),
            ("verticalApertureOffset", c.vertical_aperture_offset),
            ("focusDistance", c.focus_distance),
            ("fStop", c.f_stop),
        ];
        for (name, v) in scalars {
            out.set(name, Field::Float(f64::from(v)))?;
        }
        out.set("clippingRange", Field::F32s(c.clipping_range.to_vec()))
    }

    /// Reads the lens [`write_lens`](Self::write_lens) wrote, strictly:
    /// every field, a known projection, and a valid lens ([`lens`]).
    ///
    /// # Errors
    /// A field missing or of another kind, or an invalid lens.
    pub fn read_lens(e: &dyn ElementReader) -> Result<Camera, Error> {
        let at = || e.location();
        let projection = match e.text("projection")? {
            "Perspective" => Projection::Perspective,
            "Orthographic" => Projection::Orthographic,
            other => {
                return Err(Error::invalid(format_args!(
                    "{}: unknown projection {other:?}",
                    at()
                )));
            }
        };
        let scalar = |name| e.float(name).map(|v| v as f32);
        let clipping_range = match e.f32s("clippingRange")? {
            &[near, far] => [near, far],
            _ => {
                return Err(Error::invalid(format_args!(
                    "{}: clippingRange isn't two numbers",
                    at()
                )));
            }
        };
        let camera = Camera {
            projection,
            focal_length: scalar("focalLength")?,
            horizontal_aperture: scalar("horizontalAperture")?,
            vertical_aperture: scalar("verticalAperture")?,
            horizontal_aperture_offset: scalar("horizontalApertureOffset")?,
            vertical_aperture_offset: scalar("verticalApertureOffset")?,
            clipping_range,
            focus_distance: scalar("focusDistance")?,
            f_stop: scalar("fStop")?,
            ..Camera::default()
        };
        lens(camera).map_err(|_| Error::invalid(format_args!("{}: invalid camera", at())))
    }
}

impl DataCodec for CameraCodec {
    fn write(
        &self,
        data: &Data,
        _base: Option<&Data>,
        out: &mut dyn ElementWriter,
    ) -> Result<(), Error> {
        let c = camera()
            .get(data)
            .ok_or_else(|| Error::invalid("not a camera"))?;
        Self::write_lens(c, out)
    }

    fn read(&self, element: &dyn ElementReader, _base: Option<&Data>) -> Result<Data, Error> {
        Ok(camera().wrap(Self::read_lens(element)?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::MemoryElement;

    #[test]
    fn a_lens_saves_and_reads_back() {
        let lens = lens(Camera {
            projection: Projection::Orthographic,
            focal_length: 35.0,
            horizontal_aperture_offset: 1.5,
            clipping_range: [0.5, 900.0],
            f_stop: 2.8,
            ..Camera::default()
        })
        .unwrap();
        let mut e = MemoryElement::new("camera");
        e.write(|w| CameraCodec.write(&camera().wrap(lens), None, w))
            .unwrap();
        let back = e.read(|r| CameraCodec.read(r, None)).unwrap();
        assert_eq!(camera().get(&back), Some(&lens));
    }

    #[test]
    fn only_lenses_are_cameras() {
        let reversed = Camera {
            clipping_range: [10.0, 1.0],
            ..Camera::default()
        };
        let blind = Camera {
            focal_length: 0.0,
            ..Camera::default()
        };
        let infinite = Camera {
            focus_distance: f32::INFINITY,
            ..Camera::default()
        };
        for bad in [reversed, blind, infinite] {
            assert!(lens(bad).is_err());
        }
        let placed = Camera {
            transform: [
                [2.0, 0.0, 0.0, 0.0],
                [0.0, 2.0, 0.0, 0.0],
                [0.0, 0.0, 2.0, 0.0],
                [1.0, 2.0, 3.0, 1.0],
            ],
            mm_per_unit: 10.0,
            ..Camera::default()
        };
        assert_eq!(lens(placed).unwrap(), Camera::default(), "unplaced");
    }
}
