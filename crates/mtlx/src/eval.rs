//! Evaluating a node whose inputs all hold values: MaterialX's own formulas
//! (its GLSL implementations') for stdlib's arithmetic, channel, conditional
//! and adjustment leaves, component by component, a float spreading over a
//! wider operand as MaterialX's `FA` signatures do. What folds a material's
//! constants reads it, so an engine receives values where a network only
//! computed them.
use crate::library::NodeDef;
use crate::types::{Type, Value};

/// The nodes [`NodeDef::evaluate`] evaluates.
const EVALUATED: &[&str] = &[
    "dot",
    "constant",
    "add",
    "subtract",
    "multiply",
    "divide",
    "modulo",
    "power",
    "min",
    "max",
    "atan2",
    "absval",
    "sign",
    "floor",
    "ceil",
    "round",
    "fract",
    "sqrt",
    "exp",
    "ln",
    "sin",
    "cos",
    "tan",
    "asin",
    "acos",
    "invert",
    "clamp",
    "mix",
    "remap",
    "smoothstep",
    "dotproduct",
    "magnitude",
    "normalize",
    "crossproduct",
    "extract",
    "combine2",
    "combine3",
    "combine4",
    "separate2",
    "separate3",
    "separate4",
    "convert",
    "luminance",
    "rotate2d",
    "ifgreater",
    "ifgreatereq",
    "ifequal",
    "and",
    "or",
    "not",
];

impl NodeDef {
    /// Whether [`evaluate_output`](Self::evaluate_output) evaluates a node
    /// of this definition once its inputs hold values: stdlib's arithmetic,
    /// channel, conditional and adjustment leaves of numbers, flags and text
    /// (not a closure's or a matrix's).
    pub fn evaluates(&self) -> bool {
        let held = |ty: &Type| {
            let numbers = ty.width() > 0 && !matches!(ty, Type::Matrix33 | Type::Matrix44);
            numbers
                || matches!(
                    ty,
                    Type::Boolean | Type::Integer | Type::String | Type::Filename
                )
        };
        !self.outputs.is_empty()
            && self.outputs.iter().all(|o| held(&o.ty))
            && EVALUATED.contains(&self.node.as_str())
    }

    /// What output `output` of a node of this definition yields when its
    /// inputs hold `values`: [`evaluate`](Self::evaluate)'s value for a node
    /// of one output, a component for `separate`'s.
    pub fn evaluate_output(
        &self,
        output: &str,
        values: &dyn Fn(&str) -> Option<Value>,
    ) -> Option<Value> {
        if self.node.starts_with("separate") {
            let k = self.outputs.iter().position(|o| o.name == output)?;
            let whole = values("in").or_else(|| self.input("in")?.value.clone())?;
            let v = *whole.numbers()?.get(k)?;
            return v.is_finite().then(|| Value::Numbers(vec![v]));
        }
        let [one] = &self.outputs[..] else {
            return None;
        };
        (one.name == output).then(|| self.evaluate(values))?
    }

    /// What a node of this definition yields when its inputs hold `values`
    /// (by name; one `values` doesn't give holds its default): none for a
    /// node this doesn't evaluate (a texture, a geometric property, a shader,
    /// a matrix), or an input it reads holding no value (an unconnected
    /// geometric property), or a result that isn't finite (a zero vector
    /// normalized).
    pub fn evaluate(&self, values: &dyn Fn(&str) -> Option<Value>) -> Option<Value> {
        let value = self.compute(values)?;
        let finite = value
            .numbers()
            .is_none_or(|n| n.iter().all(|v| v.is_finite()));
        finite.then_some(value)
    }

    fn compute(&self, values: &dyn Fn(&str) -> Option<Value>) -> Option<Value> {
        let out = self.ty();
        let held = |name: &str| values(name).or_else(|| self.input(name)?.value.clone());
        let numbers = |name: &str| -> Option<Vec<f64>> {
            let width = self.input(name)?.ty.width();
            let n = held(name)?.numbers()?.to_vec();
            (width > 0 && n.len() == width).then_some(n)
        };
        // An operand spread to the output's width (a float over a color).
        let spread = |name: &str| -> Option<Vec<f64>> {
            let n = numbers(name)?;
            match (n.len(), out.width()) {
                (1, w) if w > 1 => Some(vec![n[0]; w]),
                (a, w) if a == w => Some(n),
                _ => None,
            }
        };
        let unary =
            |f: fn(f64) -> f64| Some(Value::Numbers(spread("in")?.into_iter().map(f).collect()));
        let binary = |f: fn(f64, f64) -> f64| {
            let (a, b) = (spread("in1")?, spread("in2")?);
            Some(Value::Numbers(
                a.iter().zip(&b).map(|(a, b)| f(*a, *b)).collect(),
            ))
        };
        let integers = |f: fn(i64, i64) -> i64| match (held("in1")?, held("in2")?) {
            (Value::Integer(a), Value::Integer(b)) => Some(Value::Integer(f(a, b))),
            _ => None,
        };
        if out == Type::Integer {
            return match self.node.as_str() {
                "add" => integers(|a, b| a.wrapping_add(b)),
                "subtract" => integers(|a, b| a.wrapping_sub(b)),
                "dot" | "constant" => held(if self.node == "dot" { "in" } else { "value" }),
                "ifgreater" | "ifgreatereq" | "ifequal" => self.choose(&held),
                _ => None,
            };
        }
        if matches!(out, Type::Matrix33 | Type::Matrix44) {
            return None;
        }
        match self.node.as_str() {
            "dot" => held("in"),
            "constant" => held("value"),
            // As MaterialX's graphs convert: a float spreads, a wider value
            // keeps its first numbers, a narrower one gains 0, then 1 (an
            // alpha, a w).
            "convert" => {
                let n = numbers("in")?;
                let each = (0..out.width()).map(|k| match (n.len(), n.get(k)) {
                    (1, _) => n[0],
                    (_, Some(v)) => *v,
                    _ => f64::from(u8::from(k == 3)),
                });
                Some(Value::Numbers(each.collect()))
            }
            "add" => binary(|a, b| a + b),
            "subtract" => binary(|a, b| a - b),
            "multiply" => binary(|a, b| a * b),
            "divide" => binary(|a, b| a / b),
            "modulo" => binary(|a, b| a - b * (a / b).floor()),
            "power" => binary(f64::powf),
            "min" => binary(f64::min),
            "max" => binary(f64::max),
            "atan2" => {
                let (y, x) = (spread("iny")?, spread("inx")?);
                Some(Value::Numbers(
                    y.iter().zip(&x).map(|(y, x)| y.atan2(*x)).collect(),
                ))
            }
            "absval" => unary(f64::abs),
            "sign" => unary(|a| {
                if a > 0.0 {
                    1.0
                } else if a < 0.0 {
                    -1.0
                } else {
                    0.0
                }
            }),
            "floor" => unary(f64::floor),
            "ceil" => unary(f64::ceil),
            "round" => unary(f64::round),
            "fract" => unary(|a| a - a.floor()),
            "sqrt" => unary(f64::sqrt),
            "exp" => unary(f64::exp),
            "ln" => unary(f64::ln),
            "sin" => unary(f64::sin),
            "cos" => unary(f64::cos),
            "tan" => unary(f64::tan),
            "asin" => unary(f64::asin),
            "acos" => unary(f64::acos),
            "invert" => {
                let (a, x) = (spread("amount")?, spread("in")?);
                Some(Value::Numbers(
                    a.iter().zip(&x).map(|(a, x)| a - x).collect(),
                ))
            }
            "clamp" => {
                let (x, lo, hi) = (spread("in")?, spread("low")?, spread("high")?);
                let each = (0..x.len()).map(|k| x[k].max(lo[k]).min(hi[k]));
                Some(Value::Numbers(each.collect()))
            }
            "mix" => {
                let (fg, bg, t) = (spread("fg")?, spread("bg")?, spread("mix")?);
                let each = (0..fg.len()).map(|k| bg[k] + (fg[k] - bg[k]) * t[k]);
                Some(Value::Numbers(each.collect()))
            }
            "remap" => {
                let x = spread("in")?;
                let (il, ih, ol, oh) = (
                    spread("inlow")?,
                    spread("inhigh")?,
                    spread("outlow")?,
                    spread("outhigh")?,
                );
                let each = (0..x.len())
                    .map(|k| ol[k] + (x[k] - il[k]) * (oh[k] - ol[k]) / (ih[k] - il[k]));
                Some(Value::Numbers(each.collect()))
            }
            "smoothstep" => {
                let (x, lo, hi) = (spread("in")?, spread("low")?, spread("high")?);
                let each = (0..x.len()).map(|k| {
                    let t = ((x[k] - lo[k]) / (hi[k] - lo[k])).clamp(0.0, 1.0);
                    match (x[k] >= hi[k], x[k] <= lo[k]) {
                        (true, _) => 1.0,
                        (_, true) => 0.0,
                        _ => t * t * (3.0 - 2.0 * t),
                    }
                });
                Some(Value::Numbers(each.collect()))
            }
            "dotproduct" => {
                let (a, b) = (numbers("in1")?, numbers("in2")?);
                Some(Value::Numbers(vec![
                    a.iter().zip(&b).map(|(a, b)| a * b).sum(),
                ]))
            }
            "magnitude" => {
                let a = numbers("in")?;
                Some(Value::Numbers(vec![
                    a.iter().map(|v| v * v).sum::<f64>().sqrt(),
                ]))
            }
            "normalize" => {
                let a = numbers("in")?;
                let length = a.iter().map(|v| v * v).sum::<f64>().sqrt();
                Some(Value::Numbers(a.iter().map(|v| v / length).collect()))
            }
            "crossproduct" => {
                let (a, b) = (numbers("in1")?, numbers("in2")?);
                let cross = [
                    a[1] * b[2] - a[2] * b[1],
                    a[2] * b[0] - a[0] * b[2],
                    a[0] * b[1] - a[1] * b[0],
                ];
                Some(Value::Numbers(cross.to_vec()))
            }
            "extract" => {
                let a = numbers("in")?;
                let Value::Integer(at) = held("index")? else {
                    return None;
                };
                Some(Value::Numbers(vec![*a.get(usize::try_from(at).ok()?)?]))
            }
            "combine2" | "combine3" | "combine4" => {
                let parts = ["in1", "in2", "in3", "in4"];
                let count = self.inputs.len();
                let all: Option<Vec<Vec<f64>>> =
                    parts[..count].iter().map(|p| numbers(p)).collect();
                let all: Vec<f64> = all?.concat();
                (all.len() == out.width()).then_some(Value::Numbers(all))
            }
            "luminance" => {
                let (c, w) = (numbers("in")?, numbers("lumacoeffs")?);
                let y: f64 = (0..3).map(|k| c[k] * w[k]).sum();
                let mut v = vec![y; 3];
                v.extend(c.get(3));
                Some(Value::Numbers(v))
            }
            "rotate2d" => {
                let (v, amount) = (numbers("in")?, held("amount")?.float()?);
                let (sa, ca) = amount.to_radians().sin_cos();
                Some(Value::Numbers(vec![
                    ca * v[0] + sa * v[1],
                    -sa * v[0] + ca * v[1],
                ]))
            }
            "ifgreater" | "ifgreatereq" | "ifequal" => self.choose(&held),
            "and" | "or" => match (held("in1")?, held("in2")?) {
                (Value::Boolean(a), Value::Boolean(b)) => {
                    Some(Value::Boolean(if self.node == "and" {
                        a && b
                    } else {
                        a || b
                    }))
                }
                _ => None,
            },
            "not" => match held("in")? {
                Value::Boolean(a) => Some(Value::Boolean(!a)),
                _ => None,
            },
            _ => None,
        }
    }

    /// A conditional's choice: `in1` while `value1` compares to `value2` as
    /// its node says, else `in2` (a boolean signature yields the comparison).
    fn choose(&self, held: &dyn Fn(&str) -> Option<Value>) -> Option<Value> {
        let scalar = |v: Value| -> Option<f64> {
            match v {
                Value::Integer(n) => Some(n as f64),
                Value::Boolean(b) => Some(f64::from(u8::from(b))),
                other => other.float(),
            }
        };
        let (a, b) = (scalar(held("value1")?)?, scalar(held("value2")?)?);
        let holds = match self.node.as_str() {
            "ifgreater" => a > b,
            "ifgreatereq" => a >= b,
            _ => a == b,
        };
        match self.input("in1") {
            Some(_) => held(if holds { "in1" } else { "in2" }),
            None => Some(Value::Boolean(holds)),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{Value, library};

    fn eval(def: &str, inputs: &[(&str, Value)]) -> Option<Value> {
        let def = library().def(def).unwrap_or_else(|| panic!("{def}"));
        def.evaluate(&|name| {
            inputs
                .iter()
                .find(|(n, _)| *n == name)
                .map(|(_, v)| v.clone())
        })
    }

    fn n(values: &[f64]) -> Value {
        Value::Numbers(values.to_vec())
    }

    #[test]
    fn arithmetic_follows_materialx_formulas() {
        let c = n(&[0.2, 0.4, 0.8]);
        assert_eq!(
            eval(
                "ND_multiply_color3FA",
                &[("in1", c.clone()), ("in2", n(&[0.5]))]
            ),
            Some(n(&[0.1, 0.2, 0.4]))
        );
        assert_eq!(
            eval(
                "ND_modulo_float",
                &[("in1", n(&[-1.25])), ("in2", n(&[1.0]))]
            ),
            Some(n(&[0.75]))
        );
        // mix(fg, bg, t) = bg + (fg - bg) t, unclamped.
        let mixed = eval(
            "ND_mix_float",
            &[("fg", n(&[1.0])), ("bg", n(&[0.0])), ("mix", n(&[1.5]))],
        );
        assert_eq!(mixed, Some(n(&[1.5])));
        assert_eq!(
            eval("ND_invert_float", &[("in", n(&[0.25]))]),
            Some(n(&[0.75])),
            "amount defaults to 1"
        );
        assert_eq!(
            eval(
                "ND_extract_color3",
                &[("in", c.clone()), ("index", Value::Integer(2))]
            ),
            Some(n(&[0.8]))
        );
        let combined = eval(
            "ND_combine3_vector3",
            &[("in1", n(&[1.0])), ("in2", n(&[2.0])), ("in3", n(&[3.0]))],
        );
        assert_eq!(combined, Some(n(&[1.0, 2.0, 3.0])));
        let rotated = eval(
            "ND_rotate2d_vector2",
            &[("in", n(&[1.0, 0.0])), ("amount", n(&[90.0]))],
        )
        .unwrap();
        let r = rotated.numbers().unwrap();
        assert!(r[0].abs() < 1e-12 && (r[1] + 1.0).abs() < 1e-12, "{r:?}");
        // Converting: a float spreads, a color gains alpha 1, a vector drops z.
        let convert = |signature, v: &[f64]| eval(signature, &[("in", n(v))]);
        assert_eq!(
            convert("ND_convert_float_color3", &[0.5]),
            Some(n(&[0.5; 3]))
        );
        let opaque = convert("ND_convert_color3_color4", &[0.1, 0.2, 0.3]);
        assert_eq!(opaque, Some(n(&[0.1, 0.2, 0.3, 1.0])));
        assert_eq!(
            convert("ND_convert_vector3_vector2", &[1.0, 2.0, 3.0]),
            Some(n(&[1.0, 2.0]))
        );
        let flat = convert("ND_convert_vector2_vector3", &[1.0, 2.0]);
        assert_eq!(flat, Some(n(&[1.0, 2.0, 0.0])));
        // `separate` yields each component at its own output.
        let separate = library().def("ND_separate3_color3").unwrap();
        let held = |name: &str| (name == "in").then(|| c.clone());
        assert_eq!(separate.evaluate_output("outb", &held), Some(n(&[0.8])));
        assert!(separate.evaluates() && separate.evaluate(&held).is_none());
    }

    #[test]
    fn conditionals_choose_and_the_rest_wait() {
        let pick = eval(
            "ND_ifgreater_color3",
            &[
                ("value1", n(&[1.0])),
                ("value2", n(&[0.0])),
                ("in1", n(&[1.0; 3])),
                ("in2", n(&[0.0; 3])),
            ],
        );
        assert_eq!(pick, Some(n(&[1.0; 3])));
        let equal = eval(
            "ND_ifequal_float",
            &[
                ("value1", n(&[0.0])),
                ("value2", n(&[0.0])),
                ("in1", n(&[7.0])),
            ],
        );
        assert_eq!(equal, Some(n(&[7.0])));
        assert_eq!(
            eval("ND_dot_string", &[("in", Value::Text("a".into()))]),
            Some(Value::Text("a".into()))
        );
        // A texture or a geometric property waits for its engine.
        assert_eq!(eval("ND_image_color3", &[]), None);
        assert_eq!(eval("ND_texcoord_vector2", &[]), None);
        assert_eq!(
            eval("ND_normalize_vector3", &[]),
            None,
            "a zero vector has no direction"
        );
    }
}
