//! [`PolygonsCodec`]: how [`Polygons`] values save.
use super::{attributes, invalid, polygons, selection};
use crate::codec::DataCodec;
use crate::components::Components;
use crate::data::Data;
use crate::element::{ElementReader, ElementWriter, Field};
use crate::error::Error;
use kinogaki_geometry::{Polygons, Topology};
use std::sync::Arc;

/// Saves [`Polygons`]: the field `positions` (rows of 3 reals), the child
/// `topology` (kind `topology`: `points`, `polygons`, `offsets`,
/// `vertices`), written once per save for every value sharing it, and the
/// attributes and groups as children. Reading checks every rule again;
/// values sharing a topology when saved share it when read. A round trip
/// is bit-exact.
///
/// Components save as the standard [`Selection`](kinogaki_geometry::Selection).
///
/// ```
/// use kinogaki_studio_api::DataCodec;
/// use kinogaki_studio_api::element::MemoryElement;
/// use kinogaki_studio_api::geometry::{Attributes, Polygons, Topology};
/// use kinogaki_studio_api::standard::{self, PolygonsCodec};
///
/// let t = Topology::new(3, vec![0, 3].into(), vec![0, 1, 2].into(), 1).unwrap();
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
        let topology = value.topology();
        let key = Arc::as_ptr(topology) as usize as u64;
        out.share("topology", "topology", key, &mut |w| {
            w.set("points", Field::Int(topology.points() as i64))?;
            w.set("polygons", Field::Int(topology.polygons() as i64))?;
            w.set("offsets", Field::U32s(topology.offsets().to_vec()))?;
            w.set("vertices", Field::U32s(topology.vertices().to_vec()))
        })?;
        attributes::write(value.attributes(), out)
    }

    fn read(&self, element: &dyn ElementReader, _base: Option<&Data>) -> Result<Data, Error> {
        let at = element.location();
        let topology = element.read_shared("topology", |t| {
            let at = t.location();
            Topology::new(
                attributes::count(t, "points")?,
                t.u32s("offsets")?.to_vec().into(),
                t.u32s("vertices")?.to_vec().into(),
                attributes::count(t, "polygons")?,
            )
            .map_err(|e| invalid(&at, e))
        })?;
        let positions = attributes::rows(element, "positions")?.into();
        let attributes = Arc::new(attributes::read(element)?);
        let value = Polygons::with_topology(positions, topology, attributes)
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
