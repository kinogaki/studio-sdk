//! [`PolygonsCodec`]: how [`Polygons`] values save.
use super::{attributes, invalid, polygons, selection};
use crate::codec::DataCodec;
use crate::components::Components;
use crate::data::Data;
use crate::element::{ElementReader, ElementWriter, Field};
use crate::error::Error;
use kinogaki_geometry::{CurveData, Kind, Knots, Polygons, Primitives};
use std::sync::Arc;

/// Saves [`Polygons`]: the field `positions` (rows of 3 reals), the child
/// `primitives` (kind `primitives`: `points`, `offsets`, `vertices`,
/// `kinds`, one [byte](kinogaki_geometry::Kind::byte) a primitive, and
/// when a primitive is a curve the child `curves`: `degrees`,
/// `knotOffsets`, `knots` and, when rational, `weights`), written once per
/// save for every value sharing it, and the attributes and groups as
/// children. Reading checks every rule again;
/// values sharing a topology when saved share it when read. A round trip
/// is bit-exact.
///
/// Components save as the standard [`Selection`](kinogaki_geometry::Selection).
///
/// ```
/// use kinogaki_studio_api::DataCodec;
/// use kinogaki_studio_api::element::MemoryElement;
/// use kinogaki_studio_api::geometry::{Attributes, Polygons, Primitives};
/// use kinogaki_studio_api::standard::{self, PolygonsCodec};
///
/// let t = Primitives::of_polygons(3, vec![0, 3].into(), vec![0, 1, 2].into()).unwrap();
/// let triangle = Polygons::new(vec![[0.0; 3], [1.0, 0.0, 0.0], [0.0, 1.0, 0.0]].into(), t, Attributes::new()).unwrap();
/// let data = standard::polygons().wrap(triangle.clone());
/// let mut saved = MemoryElement::new("result");
/// saved.write(|w| PolygonsCodec.write(&data, None, w)).unwrap();
/// let read = saved.read(|r| PolygonsCodec.read(r, None)).unwrap();
/// assert_eq!(standard::polygons().get(&read), Some(&triangle));
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct PolygonsCodec;

impl DataCodec for PolygonsCodec {
    fn write(
        &self,
        data: &Data,
        _base: Option<&Data>,
        out: &mut dyn ElementWriter,
    ) -> Result<(), Error> {
        let value = polygons()
            .get(data)
            .ok_or_else(|| Error::invalid("not polygons"))?;
        out.set_rows(
            "positions",
            3,
            Field::F64s(attributes::flat(value.positions())),
        )?;
        let t = value.primitives();
        let key = Arc::as_ptr(t) as usize as u64;
        out.share("primitives", "primitives", key, &mut |w| {
            w.set("points", Field::Int(t.points() as i64))?;
            w.set("offsets", Field::U32s(t.offsets().to_vec()))?;
            w.set("vertices", Field::U32s(t.vertices().to_vec()))?;
            let kinds = t.kinds().iter().map(|k| k.byte()).collect();
            w.set("kinds", Field::U8s(kinds))?;
            if let Some(c) = t.curves() {
                let mut child = w.child("curves", "curves")?;
                child.set("degrees", Field::U8s(c.degrees().to_vec()))?;
                child.set("knotOffsets", Field::U32s(c.knots().offsets().to_vec()))?;
                child.set("knots", Field::F64s(c.knots().values().to_vec()))?;
                if let Some(weights) = c.weights() {
                    child.set("weights", Field::F64s(weights.to_vec()))?;
                }
            }
            Ok(())
        })?;
        attributes::write(value.attributes(), out)
    }

    fn read(&self, element: &dyn ElementReader, _base: Option<&Data>) -> Result<Data, Error> {
        let at = element.location();
        let primitives = element.read_shared("primitives", |t| {
            let at = t.location();
            let kinds: Vec<Kind> = (attributes::u8s(t, "kinds")?.iter())
                .map(|&b| {
                    Kind::from_byte(b)
                        .ok_or_else(|| Error::invalid(format_args!("{at}: no primitive kind {b}")))
                })
                .collect::<Result<_, _>>()?;
            let curves = match t.child("curves") {
                None => None,
                Some(c) => {
                    let weights = match c.field("weights") {
                        Some(_) => Some(c.f64s("weights")?.to_vec().into()),
                        None => None,
                    };
                    let knots = Knots::new(
                        c.u32s("knotOffsets")?.to_vec().into(),
                        c.f64s("knots")?.to_vec().into(),
                    );
                    let degrees = attributes::u8s(&*c, "degrees")?.to_vec().into();
                    Some(CurveData::new(degrees, knots, weights))
                }
            };
            Primitives::new(
                attributes::count(t, "points")?,
                t.u32s("offsets")?.to_vec().into(),
                t.u32s("vertices")?.to_vec().into(),
                kinds.into(),
                curves,
            )
            .map_err(|e| invalid(&at, e))
        })?;
        let positions = attributes::rows(element, "positions")?.into();
        let attributes = Arc::new(attributes::read(element)?);
        let value = Polygons::with_primitives(positions, primitives, attributes)
            .map_err(|e| invalid(&at, e))?;
        Ok(polygons().wrap(value))
    }

    fn write_components(
        &self,
        components: &Components,
        _data: &Data,
        out: &mut dyn ElementWriter,
    ) -> Result<bool, Error> {
        selection::write(components, out)
    }

    fn read_components(
        &self,
        element: &dyn ElementReader,
        data: &Data,
    ) -> Result<Components, Error> {
        let value = polygons()
            .get(data)
            .ok_or_else(|| Error::invalid("not polygons"))?;
        selection::read(element, data.ty(), |domain| value.count(domain))
    }
}
