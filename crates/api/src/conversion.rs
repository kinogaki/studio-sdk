//! [`Conversion`]: declared conversions between data types.
//!
//! A conversion belongs to whichever package depends on both types (a
//! bridge): `kinogaki.cad.body` → `kinogaki.mesh.geometry` is Tessellate,
//! shipped by a package that requires both the CAD and the mesh kernels,
//! so neither kernel knows the other.
use crate::cancel::CancelToken;
use crate::data::Data;
use crate::error::Error;
use crate::id::{ConversionId, DataTypeId};

/// Turns values of one data type into another.
pub trait Conversion: Send + Sync {
    /// Its id (`kinogaki.cad.tessellate`).
    fn id(&self) -> ConversionId;

    /// What a UI calls it ("Tessellate").
    fn label(&self) -> &str;

    /// The type it reads.
    fn from(&self) -> DataTypeId;

    /// The type it makes.
    fn to(&self) -> DataTypeId;

    /// `data` (of type [`from`](Self::from)) as a value of type
    /// [`to`](Self::to). Long conversions poll `cancel`.
    fn convert(&self, data: &Data, cancel: &CancelToken) -> Result<Data, Error>;
}
