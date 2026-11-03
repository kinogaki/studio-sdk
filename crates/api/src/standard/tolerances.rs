//! The project's tolerances as a stored project value.
use super::invalid;
use crate::element::{ElementReader, ElementWriter, Field};
use crate::error::Error;
use crate::id::StoredId;
use crate::stored::{Scope, Stored};
use kinogaki_geometry::Tolerances;

/// The project value `kinogaki.tolerances`: the element `tolerances` among
/// the project's settings, with the reals `positional_mm`, `fit_mm`,
/// `tangent_degrees` and `curvature`. The model unit isn't saved here: the
/// project's units are its one source, and core hands kernels the value
/// with `millimeters_per_unit` from them ([`Tolerances::with_unit`]), so
/// the two never disagree. A value read back holds 1 mm a unit until then.
/// Core registers it; every kernel reads the same value.
impl Stored for Tolerances {
    fn id() -> StoredId {
        StoredId::declared("kinogaki.core.tolerances")
    }
    fn scope() -> Scope {
        Scope::Project
    }
    fn defaults() -> Self {
        Tolerances::default()
    }
    fn write(&self, to: &mut dyn ElementWriter) -> Result<(), Error> {
        to.set("positionalMm", Field::Float(self.positional_mm()))?;
        to.set("fitMm", Field::Float(self.fit_mm()))?;
        to.set("tangentDegrees", Field::Float(self.tangent_degrees()))?;
        to.set("curvature", Field::Float(self.curvature()))
    }
    fn read(from: &dyn ElementReader) -> Result<Self, Error> {
        Tolerances::new(
            from.float("positionalMm")?,
            from.float("fitMm")?,
            from.float("tangentDegrees")?,
            from.float("curvature")?,
            1.0,
        )
        .map_err(|e| invalid(&from.location(), e))
    }
}
