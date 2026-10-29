//! [`Scalar`] and [`Vector`]: the numbers a [`Handle`](super::Handle)
//! reads and writes, each bound to a parameter or fixed.
use crate::params::{Parameters, Value};

/// One number a handle reads and writes: a number parameter, or a fixed
/// value dragging leaves alone.
#[derive(Clone, Debug, PartialEq)]
pub enum Scalar {
    /// A number parameter ([`Value::Number`]).
    Parameter(String),
    /// A fixed value.
    Fixed(f64),
}

impl Scalar {
    /// The parameter it names (none when fixed).
    pub fn name(&self) -> Option<&str> {
        match self {
            Self::Parameter(name) => Some(name),
            Self::Fixed(_) => None,
        }
    }

    /// Its value in `parameters`; `None` when its parameter is missing or
    /// holds something else.
    pub fn read(&self, parameters: &Parameters) -> Option<f64> {
        match self {
            Self::Parameter(name) => parameters.get(name)?.number(),
            Self::Fixed(value) => Some(*value),
        }
    }

    /// Writes `value` into `parameters` (nothing when fixed).
    pub fn write(&self, parameters: &mut Parameters, value: f64) {
        if let Self::Parameter(name) = self {
            parameters.insert(name.clone(), Value::Number(value));
        }
    }
}

impl From<&str> for Scalar {
    /// Parameter `name`.
    fn from(name: &str) -> Self {
        Self::Parameter(name.to_owned())
    }
}

impl From<String> for Scalar {
    /// Parameter `name`.
    fn from(name: String) -> Self {
        Self::Parameter(name)
    }
}

impl From<f64> for Scalar {
    /// The fixed `value`.
    fn from(value: f64) -> Self {
        Self::Fixed(value)
    }
}

/// Three numbers a handle reads and writes: a place, a size or a
/// direction.
#[derive(Clone, Debug, PartialEq)]
pub enum Vector {
    /// One parameter of three numbers ([`Value::Numbers`], a
    /// [`ParameterKind::Vector`](crate::params::ParameterKind::Vector)).
    Parameter(String),
    /// Three [`Scalar`]s, x, y and z, each a number parameter or fixed: a
    /// cylinder's top ring sits at (0, `height`, 0); a shape always
    /// centered on the origin is all fixed.
    Components([Scalar; 3]),
}

impl Vector {
    /// Parameter `name`, three numbers.
    pub fn parameter(name: impl Into<String>) -> Self {
        Self::Parameter(name.into())
    }

    /// `x`, `y` and `z`, each a number parameter's name or a fixed value.
    pub fn components(x: impl Into<Scalar>, y: impl Into<Scalar>, z: impl Into<Scalar>) -> Self {
        Self::Components([x.into(), y.into(), z.into()])
    }

    /// The fixed `value`.
    pub fn fixed(value: [f64; 3]) -> Self {
        Self::Components(value.map(Scalar::Fixed))
    }

    /// The parameters it names, in order (none when fixed).
    pub fn names(&self) -> Vec<&str> {
        match self {
            Self::Parameter(name) => vec![name],
            Self::Components(scalars) => scalars.iter().filter_map(Scalar::name).collect(),
        }
    }

    /// Whether dragging can change number `axis` (0, 1 or 2): it is a
    /// parameter's.
    pub fn writes(&self, axis: usize) -> bool {
        match self {
            Self::Parameter(_) => axis < 3,
            Self::Components(scalars) => scalars.get(axis).and_then(Scalar::name).is_some(),
        }
    }

    /// Its value in `parameters`; `None` when a parameter is missing or
    /// holds something else.
    pub fn read(&self, parameters: &Parameters) -> Option<[f64; 3]> {
        match self {
            Self::Parameter(name) => match parameters.get(name)?.numbers()? {
                &[x, y, z] => Some([x, y, z]),
                _ => None,
            },
            Self::Components([x, y, z]) => Some([
                x.read(parameters)?,
                y.read(parameters)?,
                z.read(parameters)?,
            ]),
        }
    }

    /// Writes `value` into `parameters`: each number a parameter holds
    /// (nothing when fixed).
    pub fn write(&self, parameters: &mut Parameters, value: [f64; 3]) {
        match self {
            Self::Parameter(name) => {
                parameters.insert(name.clone(), Value::Numbers(value.to_vec()));
            }
            Self::Components(scalars) => {
                for (scalar, v) in scalars.iter().zip(value) {
                    scalar.write(parameters, v);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vectors_read_and_write_one_or_three_parameters() {
        let mut values = Parameters::new();
        let one = Vector::parameter("center");
        let three = Vector::components("x", "y", "z");
        assert_eq!(one.read(&values), None);
        one.write(&mut values, [1.0, 2.0, 3.0]);
        three.write(&mut values, [4.0, 5.0, 6.0]);
        assert_eq!(one.read(&values), Some([1.0, 2.0, 3.0]));
        assert_eq!(three.read(&values), Some([4.0, 5.0, 6.0]));
        assert_eq!(values["y"], Value::Number(5.0));
        let fixed = Vector::fixed([0.0; 3]);
        fixed.write(&mut values, [9.0; 3]);
        assert_eq!(fixed.read(&values), Some([0.0; 3]));
        assert!(fixed.names().is_empty() && !fixed.writes(0));
        values.insert("center".into(), Value::Numbers(vec![1.0]));
        assert_eq!(one.read(&values), None, "a vector has three numbers");
    }

    /// A vector mixing parameters and fixed numbers reads them all and
    /// writes only its parameters.
    #[test]
    fn mixed_vectors_write_only_their_parameters() {
        let top = Vector::components(0.0, "height", 0.0);
        let mut values = Parameters::new();
        values.insert("height".into(), Value::Number(2.0));
        assert_eq!(top.read(&values), Some([0.0, 2.0, 0.0]));
        assert_eq!(top.names(), ["height"]);
        assert_eq!([0, 1, 2].map(|a| top.writes(a)), [false, true, false]);
        top.write(&mut values, [7.0, 3.0, 7.0]);
        assert_eq!(values.len(), 1);
        assert_eq!(top.read(&values), Some([0.0, 3.0, 0.0]));
        assert_eq!(Scalar::from(1.5).read(&values), Some(1.5));
        assert_eq!(Scalar::from("height").name(), Some("height"));
    }
}
