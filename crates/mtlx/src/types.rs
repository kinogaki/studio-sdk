//! MaterialX's types ([`Type`]) and the values of the ones that hold
//! numbers, flags and text ([`Value`]), read from and written as MaterialX's
//! attribute text (`"0.8, 0.8, 0.8"`).
use std::fmt;
use std::sync::Arc;

/// A MaterialX type.
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum Type {
    /// `boolean`.
    Boolean,
    /// `integer`.
    Integer,
    /// `float`.
    Float,
    /// `color3`: linear RGB.
    Color3,
    /// `color4`: linear RGB and alpha.
    Color4,
    /// `vector2`.
    Vector2,
    /// `vector3`.
    Vector3,
    /// `vector4`.
    Vector4,
    /// `matrix33`, by rows.
    Matrix33,
    /// `matrix44`, by rows.
    Matrix44,
    /// `string`.
    String,
    /// `filename`: a path, relative to the document.
    Filename,
    /// Any other, known by its name alone: the closures (`BSDF`, `EDF`,
    /// `VDF`), the shaders (`surfaceshader`, `displacementshader`,
    /// `volumeshader`, `lightshader`), `material`, arrays and structs.
    Named(Arc<str>),
}

impl Type {
    /// The type `name` names.
    pub fn of(name: &str) -> Type {
        match name {
            "boolean" => Type::Boolean,
            "integer" => Type::Integer,
            "float" => Type::Float,
            "color3" => Type::Color3,
            "color4" => Type::Color4,
            "vector2" => Type::Vector2,
            "vector3" => Type::Vector3,
            "vector4" => Type::Vector4,
            "matrix33" => Type::Matrix33,
            "matrix44" => Type::Matrix44,
            "string" => Type::String,
            "filename" => Type::Filename,
            other => Type::Named(Arc::from(other)),
        }
    }

    /// Its name.
    pub fn name(&self) -> &str {
        match self {
            Type::Boolean => "boolean",
            Type::Integer => "integer",
            Type::Float => "float",
            Type::Color3 => "color3",
            Type::Color4 => "color4",
            Type::Vector2 => "vector2",
            Type::Vector3 => "vector3",
            Type::Vector4 => "vector4",
            Type::Matrix33 => "matrix33",
            Type::Matrix44 => "matrix44",
            Type::String => "string",
            Type::Filename => "filename",
            Type::Named(name) => name,
        }
    }

    /// The numbers a value of it holds: 1 for a float, 3 for a color3, 16
    /// for a matrix44; 0 for a type holding no numbers.
    pub fn width(&self) -> usize {
        match self {
            Type::Float => 1,
            Type::Vector2 => 2,
            Type::Color3 | Type::Vector3 => 3,
            Type::Color4 | Type::Vector4 => 4,
            Type::Matrix33 => 9,
            Type::Matrix44 => 16,
            _ => 0,
        }
    }

    /// Whether it is a color (`color3`, `color4`).
    pub fn is_color(&self) -> bool {
        matches!(self, Type::Color3 | Type::Color4)
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A value of a type that holds one ([`Type::width`] numbers, a flag, an
/// integer or text).
#[derive(Clone, Debug, PartialEq)]
pub enum Value {
    /// A `boolean`.
    Boolean(bool),
    /// An `integer`.
    Integer(i64),
    /// The numbers of a `float`, a color, a vector or a matrix, in order.
    Numbers(Vec<f64>),
    /// A `string` or `filename`.
    Text(String),
}

impl Value {
    /// `text` as a value of `ty` (`"0.8, 0.8, 0.8"` for a color3); none when
    /// it doesn't read as one, or `ty` holds no value.
    pub fn parse(ty: &Type, text: &str) -> Option<Value> {
        Some(match ty {
            Type::Boolean => Value::Boolean(match text.trim() {
                "true" => true,
                "false" => false,
                _ => return None,
            }),
            Type::Integer => Value::Integer(text.trim().parse().ok()?),
            Type::String | Type::Filename => Value::Text(text.to_owned()),
            Type::Named(_) => return None,
            _ => {
                let numbers: Option<Vec<f64>> = (text.split(','))
                    .map(|n| n.trim().parse::<f64>().ok())
                    .collect();
                let numbers = numbers?;
                (numbers.len() == ty.width()).then_some(Value::Numbers(numbers))?
            }
        })
    }

    /// The numbers, when it holds numbers.
    pub fn numbers(&self) -> Option<&[f64]> {
        match self {
            Value::Numbers(n) => Some(n),
            _ => None,
        }
    }

    /// Its one number, when it is a float.
    pub fn float(&self) -> Option<f64> {
        match self {
            Value::Numbers(n) if n.len() == 1 => Some(n[0]),
            _ => None,
        }
    }
}

impl fmt::Display for Value {
    /// As MaterialX writes it: numbers in their shortest form, `, ` between.
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Value::Boolean(on) => write!(f, "{on}"),
            Value::Integer(n) => write!(f, "{n}"),
            Value::Text(text) => f.write_str(text),
            Value::Numbers(numbers) => {
                for (k, n) in numbers.iter().enumerate() {
                    if k > 0 {
                        f.write_str(", ")?;
                    }
                    // Values are f32 in MaterialX: written at that precision.
                    write!(f, "{}", *n as f32)?;
                }
                Ok(())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn values_read_and_write_as_materialx_text() {
        let color = Value::parse(&Type::Color3, "0.8, 0.8,0.8").unwrap();
        assert_eq!(color, Value::Numbers(vec![0.8; 3]));
        assert_eq!(color.to_string(), "0.8, 0.8, 0.8");
        assert_eq!(Value::parse(&Type::Color3, "1, 2"), None, "too few");
        assert_eq!(
            Value::parse(&Type::Float, "1.6").unwrap().float(),
            Some(1.6)
        );
        assert_eq!(
            Value::parse(&Type::Boolean, "true"),
            Some(Value::Boolean(true))
        );
        assert_eq!(Value::parse(&Type::Integer, "-3"), Some(Value::Integer(-3)));
        let bsdf = Type::of("BSDF");
        assert_eq!((bsdf.name(), bsdf.width()), ("BSDF", 0));
        assert_eq!(Value::parse(&bsdf, ""), None);
        assert!(Type::of("color4").is_color() && Type::of("matrix44").width() == 16);
    }
}
