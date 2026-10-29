//! [`Frame`]: a placement frame bound to a node type's Position, Up and
//! Turn, which a [`Handle`](super::Handle)'s places can be expressed in.
use super::binding::{Scalar, Vector};
use crate::math::{Affine3, Vec3};
use crate::params::Parameters;

/// A placement frame bound to parameters: an origin at `position`, its Y
/// axis along `up` and turned `turn` degrees about it. Studio's
/// primitives are placed this way (a box built on its base, then placed
/// where the user clicked, its Up along the surface there).
///
/// The axes follow one rule, [`Frame::place`]: Y is `up` at unit length;
/// X is world X made square to Y (world −Z when Y lies within about 25°
/// of ±X), then turned right-handedly about Y by `turn`; Z is X × Y. A
/// node type that places what it builds by the same rule gets handles
/// that sit on it.
///
/// ```
/// use kinogaki_studio_api::cook::{Frame, Vector};
/// use kinogaki_studio_api::{Parameters, Value, Vec3};
/// let frame = Frame::new(
///     Vector::components("position_x", "position_y", "position_z"),
///     Vector::components("up_x", "up_y", "up_z"),
///     "turn",
/// );
/// let mut values = Parameters::new();
/// for (name, v) in [("position_x", 1.0), ("position_y", 2.0), ("position_z", 3.0)] {
///     values.insert(name.into(), Value::Number(v));
/// }
/// for (name, v) in [("up_x", 0.0), ("up_y", 0.0), ("up_z", 1.0), ("turn", 0.0)] {
///     values.insert(name.into(), Value::Number(v));
/// }
/// // Up along Z: Y is Z, X stays X, Z is X × Y = −Y.
/// let placed = frame.read(&values).unwrap();
/// assert_eq!(placed.translation, Vec3::new(1.0, 2.0, 3.0));
/// assert_eq!(placed.linear, [
///     Vec3::new(1.0, 0.0, 0.0),
///     Vec3::new(0.0, 0.0, 1.0),
///     Vec3::new(0.0, -1.0, 0.0),
/// ]);
/// ```
#[derive(Clone, Debug, PartialEq)]
pub struct Frame {
    /// Where its origin is.
    pub position: Vector,
    /// Its Y axis (any length but zero).
    pub up: Vector,
    /// Its turn about Y, in degrees.
    pub turn: Scalar,
}

impl Frame {
    /// The frame at `position`, along `up`, turned by `turn` (a number
    /// parameter's name, or a fixed angle).
    pub fn new(position: Vector, up: Vector, turn: impl Into<Scalar>) -> Self {
        Self {
            position,
            up,
            turn: turn.into(),
        }
    }

    /// The parameters it reads, in order.
    pub fn names(&self) -> Vec<&str> {
        let mut names = self.position.names();
        names.extend(self.up.names());
        names.extend(self.turn.name());
        names
    }

    /// The placement at `position` along `up` turned `turn` degrees (the
    /// rule [`Frame`] describes): its linear part's columns are the
    /// frame's unit X, Y and Z axes. `None` when a number isn't finite or
    /// `up` has no direction.
    pub fn place(position: Vec3, up: Vec3, turn: f64) -> Option<Affine3> {
        let (p, u) = (position.to_array(), up.to_array());
        if !p.iter().chain(&u).all(|v| v.is_finite()) || !turn.is_finite() {
            return None;
        }
        let y = unit(u)?;
        let [x0, z0] = square(y)?;
        let (s, c) = turn.rem_euclid(360.0).to_radians().sin_cos();
        let x = add(scale(x0, c), scale(z0, -s));
        let z = add(scale(x0, s), scale(z0, c));
        Some(Affine3 {
            linear: [x, y, z].map(Vec3::from_array),
            translation: position,
        })
    }

    /// Its placement in `parameters`; `None` when a parameter is missing
    /// or holds something else, or [`Frame::place`] has none.
    pub fn read(&self, parameters: &Parameters) -> Option<Affine3> {
        Self::place(
            Vec3::from_array(self.position.read(parameters)?),
            Vec3::from_array(self.up.read(parameters)?),
            self.turn.read(parameters)?,
        )
    }

    /// Writes the Up (unit length) and Turn that turn the frame so its X
    /// and Y axes are `x` and `y` (`x` made square to `y`), as a rotate
    /// handle turning the frame does. The turn written is the one nearest
    /// the turn `parameters` held, so a drag carries on past ±180°. A
    /// fixed up or turn is left alone; nothing is written when `y` has no
    /// direction or `x` lies along it.
    pub fn orient(&self, parameters: &mut Parameters, x: Vec3, y: Vec3) {
        let Some(y) = unit(y.to_array()) else {
            return;
        };
        let x = x.to_array();
        let Some(x) = unit(add(x, scale(y, -dot(x, y)))) else {
            return;
        };
        let Some([x0, z0]) = square(y) else {
            return;
        };
        let turn = (-dot(x, z0)).atan2(dot(x, x0)).to_degrees();
        let held = self.turn.read(parameters).unwrap_or(0.0);
        let turn = turn + ((held - turn) / 360.0).round() * 360.0;
        self.up.write(parameters, y);
        self.turn.write(parameters, turn);
    }
}

/// X and Z of an unturned frame along unit `y`.
fn square(y: [f64; 3]) -> Option<[[f64; 3]; 2]> {
    let reference = if y[0].abs() > 0.9 {
        [0.0, 0.0, -1.0]
    } else {
        [1.0, 0.0, 0.0]
    };
    let x0 = unit(add(reference, scale(y, -dot(reference, y))))?;
    Some([x0, cross(x0, y)])
}

fn unit(v: [f64; 3]) -> Option<[f64; 3]> {
    let length = dot(v, v).sqrt();
    (length > 1e-12 && length.is_finite()).then(|| scale(v, 1.0 / length))
}

fn dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn add(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [a[0] + b[0], a[1] + b[1], a[2] + b[2]]
}

fn scale(a: [f64; 3], k: f64) -> [f64; 3] {
    a.map(|v| v * k)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::params::Value;

    fn near(a: Vec3, b: Vec3) -> bool {
        let d = [a.x - b.x, a.y - b.y, a.z - b.z];
        dot(d, d).sqrt() < 1e-9
    }

    fn frame() -> Frame {
        Frame::new(
            Vector::parameter("position"),
            Vector::components("up_x", "up_y", "up_z"),
            "turn",
        )
    }

    fn values(up: [f64; 3], turn: f64) -> Parameters {
        let mut values = Parameters::new();
        values.insert("position".into(), Value::Numbers(vec![0.0; 3]));
        for (name, v) in ["up_x", "up_y", "up_z"].into_iter().zip(up) {
            values.insert(name.into(), Value::Number(v));
        }
        values.insert("turn".into(), Value::Number(turn));
        values
    }

    /// The axes are unit length, square and right-handed for any up and
    /// turn; up along Y unturned is the identity; a quarter turn takes X
    /// to −Z; up near X starts from −Z.
    #[test]
    fn frames_are_right_handed_and_turn_about_up() {
        for up in [
            [0.0, 1.0, 0.0],
            [0.3, -0.8, 0.2],
            [1.0, 0.1, 0.0],
            [0.0, 0.0, -2.0],
        ] {
            for turn in [0.0, 37.0, -200.0] {
                let [x, y, z] = Frame::place(Vec3::ZERO, Vec3::from_array(up), turn)
                    .unwrap()
                    .linear
                    .map(Vec3::to_array);
                for (a, b) in [(x, y), (y, z), (x, z)] {
                    assert!(dot(a, b).abs() < 1e-12);
                }
                for a in [x, y, z] {
                    assert!((dot(a, a) - 1.0).abs() < 1e-12);
                }
                let handed = cross(x, y);
                assert!(near(Vec3::from_array(handed), Vec3::from_array(z)));
            }
        }
        let axes = |up: Vec3, turn| Frame::place(Vec3::ZERO, up, turn).unwrap().linear;
        assert_eq!(
            axes(Vec3::new(0.0, 1.0, 0.0), 0.0),
            Affine3::IDENTITY.linear
        );
        assert!(near(
            axes(Vec3::new(0.0, 1.0, 0.0), 90.0)[0],
            Vec3::new(0.0, 0.0, -1.0)
        ));
        assert!(near(
            axes(Vec3::new(1.0, 0.0, 0.0), 0.0)[0],
            Vec3::new(0.0, 0.0, -1.0)
        ));
        assert_eq!(Frame::place(Vec3::ZERO, Vec3::ZERO, 0.0), None);
        assert_eq!(
            Frame::place(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0), f64::NAN),
            None
        );
    }

    /// Orienting writes the up and turn that give the axes asked for, the
    /// turn nearest the one held.
    #[test]
    fn orienting_writes_the_up_and_turn_of_the_axes() {
        let frame = frame();
        let mut held = values([0.0, 1.0, 0.0], 0.0);
        let target = Frame::place(Vec3::ZERO, Vec3::new(0.2, 0.9, -0.4), 70.0).unwrap();
        frame.orient(&mut held, target.linear[0], target.linear[1]);
        let read = frame.read(&held).unwrap();
        for (a, b) in read.linear.iter().zip(target.linear) {
            assert!(near(*a, b), "{a:?} {b:?}");
        }
        assert!((held["turn"].number().unwrap() - 70.0).abs() < 1e-9);
        // Past a half turn, the turn carries on from the one held.
        let mut held = values([0.0, 1.0, 0.0], 170.0);
        let turned = Frame::place(Vec3::ZERO, Vec3::new(0.0, 1.0, 0.0), 200.0).unwrap();
        frame.orient(&mut held, turned.linear[0], turned.linear[1]);
        assert!((held["turn"].number().unwrap() - 200.0).abs() < 1e-9);
        // A fixed turn stays; the up still follows.
        let fixed = Frame::new(Vector::parameter("position"), Vector::parameter("up"), 0.0);
        let mut held = Parameters::new();
        fixed.orient(&mut held, target.linear[0], target.linear[1]);
        assert_eq!(held.len(), 1);
        assert_eq!(fixed.names(), ["position", "up"]);
        assert_eq!(frame.names(), ["position", "up_x", "up_y", "up_z", "turn"]);
    }
}
