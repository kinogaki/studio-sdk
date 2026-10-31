//! [`CurvesCodec`]: how [`Curves`] values save.
use super::{attributes, curves, invalid, selection};
use crate::codec::DataCodec;
use crate::components::Components;
use crate::data::Data;
use crate::element::{ElementReader, ElementWriter, Field};
use crate::error::Error;
use kinogaki_geometry::{Basis, Column, Curves, Group};

/// Saves [`Curves`]: `basis` (its [name](Basis::name)), `positions` (rows
/// of 3 reals), `offsets`, `indexed` and the point index `points` (empty
/// when not indexed), `degrees`, the `periodic` curves' bit words (each
/// `u64` as the `i64` with the same bits), for NURBS `knot_offsets` and
/// `knots`, `rational` and `weights` (empty when not rational), then the
/// attributes and groups as children. Reading checks every rule again; a
/// round trip is bit-exact.
///
/// ```
/// use kinogaki_studio_api::DataCodec;
/// use kinogaki_studio_api::element::MemoryElement;
/// use kinogaki_studio_api::geometry::{Basis, Curves};
/// use kinogaki_studio_api::standard::{self, CurvesCodec};
///
/// let curve = Curves::builder(Basis::BSpline, vec![[0.0; 3], [1.0; 3], [2.0; 3], [3.0; 3]].into(), vec![0, 4].into())
///     .build()
///     .unwrap();
/// let data = standard::curves().wrap(curve.clone());
/// let mut saved = MemoryElement::new("result");
/// saved.write(|w| CurvesCodec.write(&data, None, w)).unwrap();
/// let read = saved.read(|r| CurvesCodec.read(r, None)).unwrap();
/// assert_eq!(standard::curves().get(&read), Some(&curve));
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct CurvesCodec;

impl DataCodec for CurvesCodec {
    fn write(
        &self,
        data: &Data,
        _base: Option<&Data>,
        out: &mut dyn ElementWriter,
    ) -> Result<(), Error> {
        let c = curves()
            .get(data)
            .ok_or_else(|| Error::invalid("not curves"))?;
        out.set("basis", Field::Text(c.basis().name().into()))?;
        out.set_rows("positions", 3, Field::F64s(attributes::flat(c.positions())))?;
        out.set("offsets", Field::U32s(c.offsets().to_vec()))?;
        out.set("indexed", Field::Bool(c.points().is_some()))?;
        let points = c.points().map_or(Vec::new(), |p| p.to_vec());
        out.set("points", Field::U32s(points))?;
        out.set("degrees", Field::U8s(c.degrees().to_vec()))?;
        let periodic = c.periodic().words().iter().map(|&w| w as i64).collect();
        out.set("periodic", Field::I64s(periodic))?;
        if let Some(knots) = c.knots() {
            out.set("knot_offsets", Field::U32s(knots.offsets().to_vec()))?;
            out.set("knots", Field::F64s(knots.values().to_vec()))?;
        }
        out.set("rational", Field::Bool(c.weights().is_some()))?;
        let weights = c.weights().map_or(Vec::new(), |w| w.to_vec());
        out.set("weights", Field::F64s(weights))?;
        attributes::write(c.attributes(), out)
    }

    fn read(&self, element: &dyn ElementReader, _base: Option<&Data>) -> Result<Data, Error> {
        let at = element.location();
        let name = element.text("basis")?;
        let basis = Basis::from_name(name)
            .ok_or_else(|| Error::invalid(format_args!("{at}: no curve basis {name:?}")))?;
        let offsets: Column<u32> = element.u32s("offsets")?.to_vec().into();
        let curves_count = offsets.len().saturating_sub(1);
        let words: Vec<u64> = attributes::i64s(element, "periodic")?
            .iter()
            .map(|&w| w as u64)
            .collect();
        let periodic = Group::new(curves_count, words.into()).map_err(|e| invalid(&at, e))?;
        let mut builder = Curves::builder(
            basis,
            attributes::rows(element, "positions")?.into(),
            offsets,
        )
        .degrees(attributes::u8s(element, "degrees")?.to_vec().into())
        .periodic(periodic)
        .attributes(attributes::read(element)?);
        if element.bool("indexed")? {
            builder = builder.points(element.u32s("points")?.to_vec().into());
        }
        if basis == Basis::Nurbs {
            builder = builder.knots(
                element.u32s("knot_offsets")?.to_vec().into(),
                element.f64s("knots")?.to_vec().into(),
            );
        }
        if element.bool("rational")? {
            builder = builder.weights(element.f64s("weights")?.to_vec().into());
        }
        let value = builder.build().map_err(|e| invalid(&at, e))?;
        Ok(curves().wrap(value))
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
        let value = curves()
            .get(data)
            .ok_or_else(|| Error::invalid("not curves"))?;
        selection::read(element, data.ty(), |class| value.count(class))
    }
}
