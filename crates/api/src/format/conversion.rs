//! [`Conversion`]: a file's space in the project's.
use crate::Error;
use kinogaki_geometry::{Units, UpAxis, Vec3, Xform};

/// The change from a file's space into the project's (`C = s·Q`): a
/// uniform scale `s`, the project's units a file unit, and `Q`, what the
/// file's axes are among the project's: a turn between their up axes, or a
/// mirroring where the file is left-handed.
///
/// A file's value moves into the project whole: its points by `C`, its
/// directions (normals, tangents) by `Q`, its lengths by `s`. A placement
/// relative to a parent moves so that what it places lands where `C` puts
/// it: a group's or a mesh's as `Q·M·Q⁻¹` ([`placement`](Self::placement)),
/// its translation scaled, so no placement carries the change of units and
/// a hierarchy keeps its shape; a camera's or a light's as `Q·M`
/// ([`frame`](Self::frame)), since its value keeps the frame it looks down
/// (−Z, +Y up) in either space. [`inverse`](Self::inverse) is the way back,
/// for a writer: it divides by the scale its conversion multiplies by, so a
/// length read and written back is the number it was wherever the scale's
/// arithmetic allows.
///
/// An axis turn is a signed permutation, applied exactly: a coordinate is
/// moved and maybe negated, never summed with zeros.
///
/// ```
/// use kinogaki_studio_api::format::Conversion;
/// use kinogaki_studio_api::geometry::{Units, UpAxis, Vec3, Xform};
///
/// // A centimetre file, Z up, into a project in metres, Y up.
/// let file = Units::new(0.01, UpAxis::Z).unwrap();
/// let into = Conversion::new(file, Units::default());
/// assert_eq!(into.point(Vec3::new(0.0, 0.0, 100.0)), Vec3::new(0.0, 1.0, 0.0));
/// assert_eq!(into.direction(Vec3::Z), Vec3::Y);
/// let raised = into.placement(&Xform::translation(Vec3::new(0.0, 0.0, 50.0)));
/// assert_eq!(raised.origin(), Vec3::new(0.0, 0.5, 0.0));
/// // A camera at the file's origin looks down the file's −Z: the ground.
/// let camera = into.frame(&Xform::IDENTITY);
/// assert_eq!(camera.apply_vector(-Vec3::Z), -Vec3::Y);
/// assert_eq!(into.inverse().point(Vec3::new(0.0, 1.0, 0.0)), Vec3::new(0.0, 0.0, 100.0));
/// ```
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Conversion {
    /// The project's units a file unit.
    scale: f64,
    /// Whether this is the way back, from the project into the file:
    /// lengths divided by `scale`.
    back: bool,
    /// `Q` by rows: the file's axes are its columns.
    turn: [[f64; 3]; 3],
}

/// How far a given axis may stray from unit length and right angles.
const SLACK: f64 = 1e-9;

impl Conversion {
    /// From a file of `file`'s units and up axis (right-handed, as both
    /// are) into `project`'s.
    pub fn new(file: Units, project: Units) -> Conversion {
        let scale = file.meters_per_unit() / project.meters_per_unit();
        let axes = match (file.up(), project.up()) {
            (UpAxis::Y, UpAxis::Z) => [Vec3::X, Vec3::Z, -Vec3::Y],
            (UpAxis::Z, UpAxis::Y) => [Vec3::X, -Vec3::Z, Vec3::Y],
            _ => [Vec3::X, Vec3::Y, Vec3::Z],
        };
        Conversion::of(axes, scale)
    }

    /// From a file whose x, y and z axes are `axes` in the project's (unit
    /// and at right angles; left-handed for a mirroring), `scale` project
    /// units a file unit.
    ///
    /// # Errors
    /// Axes that aren't unit or at right angles, or a scale that isn't
    /// finite and above zero.
    pub fn of_axes(axes: [Vec3; 3], scale: f64) -> Result<Conversion, Error> {
        let unit = |v: Vec3| (v.dot(v) - 1.0).abs() <= SLACK;
        let square = |a: Vec3, b: Vec3| a.dot(b).abs() <= SLACK;
        let [x, y, z] = axes;
        let fine = axes.into_iter().all(unit) && square(x, y) && square(y, z) && square(x, z);
        if !fine {
            return Err(Error::invalid(
                "a file's axes must be unit and at right angles",
            ));
        }
        if !(scale.is_finite() && scale > 0.0) {
            return Err(Error::invalid("a file's unit must be a finite length"));
        }
        Ok(Conversion::of(axes, scale))
    }

    /// `axes` (checked) as `Q`'s columns.
    fn of(axes: [Vec3; 3], scale: f64) -> Conversion {
        let columns = axes.map(Vec3::to_array);
        Conversion {
            scale,
            back: false,
            turn: std::array::from_fn(|row| std::array::from_fn(|col| columns[col][row])),
        }
    }

    /// The units it converts into a unit it converts from: the project's
    /// a file unit (on the way back, the file's a project unit).
    pub fn scale(&self) -> f64 {
        self.length(1.0)
    }

    /// Whether the file's handedness is the project's other one: its faces
    /// then wind the other way round.
    pub fn mirrors(&self) -> bool {
        let [x, y, z] = self.axes();
        x.cross(y).dot(z) < 0.0
    }

    /// `Q`'s columns: the file's axes in the project's.
    fn axes(&self) -> [Vec3; 3] {
        let t = &self.turn;
        std::array::from_fn(|col| Vec3::new(t[0][col], t[1][col], t[2][col]))
    }

    /// A point of the file in the project: `C·p`.
    pub fn point(&self, p: Vec3) -> Vec3 {
        let turned = self.direction(p);
        Vec3::new(
            self.length(turned.x),
            self.length(turned.y),
            self.length(turned.z),
        )
    }

    /// A direction of the file (a normal, a tangent) in the project:
    /// turned, not scaled.
    pub fn direction(&self, v: Vec3) -> Vec3 {
        Vec3::from(apply(&self.turn, v.to_array()))
    }

    /// A length of the file in the project's units.
    pub fn length(&self, length: f64) -> f64 {
        match self.back {
            true => length / self.scale,
            false => length * self.scale,
        }
    }

    /// `C` as a placement: what takes the file's points into the project.
    pub fn xform(&self) -> Xform {
        let [x, y, z] = self.axes().map(|a| a * self.scale());
        Xform::from_axes(x, y, z, Vec3::ZERO)
    }

    /// The placement `m` of a value (a group, a mesh, instances: what
    /// moves with its points) in the project: `Q·M·Q⁻¹`, its translation
    /// scaled.
    pub fn placement(&self, m: &Xform) -> Xform {
        let back = transpose(&self.turn);
        self.placed(&product(&product(&self.turn, &linear(m)), &back), m)
    }

    /// The placement `m` of a camera or a light (a value keeping the frame
    /// it looks down) in the project: `Q·M`, its translation scaled.
    pub fn frame(&self, m: &Xform) -> Xform {
        self.placed(&product(&self.turn, &linear(m)), m)
    }

    /// Linear part `l` and `m`'s origin moved by `C`.
    fn placed(&self, l: &[[f64; 3]; 3], m: &Xform) -> Xform {
        let origin = self.point(m.origin()).to_array();
        Xform::from_rows(std::array::from_fn(|row| {
            [l[row][0], l[row][1], l[row][2], origin[row]]
        }))
    }

    /// The change from the project's space back into the file's.
    pub fn inverse(&self) -> Conversion {
        Conversion {
            scale: self.scale,
            back: !self.back,
            turn: transpose(&self.turn),
        }
    }
}

/// `m`'s linear part, by rows.
fn linear(m: &Xform) -> [[f64; 3]; 3] {
    let r = m.rows();
    std::array::from_fn(|row| [r[row][0], r[row][1], r[row][2]])
}

fn transpose(m: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|row| std::array::from_fn(|col| m[col][row]))
}

/// `a·v`, its zero entries skipped: a signed permutation moves and negates
/// coordinates exactly (a negative zero stays one).
fn apply(a: &[[f64; 3]; 3], v: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|row| dot(a[row], |k| v[k]))
}

/// `a·b`, as [`apply`] does.
fn product(a: &[[f64; 3]; 3], b: &[[f64; 3]; 3]) -> [[f64; 3]; 3] {
    std::array::from_fn(|row| std::array::from_fn(|col| dot(a[row], |k| b[k][col])))
}

/// `row · (at(0), at(1), at(2))` over `row`'s entries that aren't zero.
fn dot(row: [f64; 3], at: impl Fn(usize) -> f64) -> f64 {
    let mut terms = (0..3).filter(|&k| row[k] != 0.0).map(|k| row[k] * at(k));
    let first = terms.next().unwrap_or(0.0);
    terms.fold(first, |sum, t| sum + t)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).length() < 1e-12
    }

    #[test]
    fn a_centimetre_z_up_file_lands_in_metres_y_up_with_its_hierarchy() {
        let into = Conversion::new(Units::new(0.01, UpAxis::Z).unwrap(), Units::default());
        let parent = into.placement(&Xform::translation(Vec3::new(0.0, 0.0, 100.0)));
        let child = into.placement(&Xform::translation(Vec3::new(50.0, 0.0, 0.0)));
        let at = parent.compose(&child).apply_point(Vec3::ZERO);
        assert!(close(at, Vec3::new(0.5, 1.0, 0.0)));
        // Placements carry no scale: the change of units is in the points.
        assert!(close(parent.axes()[1], Vec3::Y));
        // A mesh under a placed node lands where the file puts its points:
        // (Q M Q⁻¹)(C p) = C (M p).
        let m = Xform::from_axes(Vec3::Y, -Vec3::X, Vec3::Z, Vec3::new(1.0, 2.0, 3.0));
        let p = Vec3::new(0.5, -1.0, 2.0);
        assert!(close(
            into.placement(&m).apply_point(into.point(p)),
            into.point(m.apply_point(p))
        ));
        assert!(!into.mirrors());
    }

    #[test]
    fn a_frame_keeps_what_it_looks_down() {
        // A Y-up file into Z up: a camera at the file's identity looks down
        // the file's −Z, the project's +Y, with the file's up, +Z, up.
        let into = Conversion::new(Units::default(), Units::new(1.0, UpAxis::Z).unwrap());
        let camera = into.frame(&Xform::translation(Vec3::new(0.0, 2.0, 0.0)));
        assert!(close(camera.apply_vector(-Vec3::Z), Vec3::Y));
        assert!(close(camera.apply_vector(Vec3::Y), Vec3::Z));
        assert!(close(camera.origin(), Vec3::new(0.0, 0.0, 2.0)));
    }

    #[test]
    fn the_inverse_takes_points_and_placements_back_exactly() {
        let into = Conversion::new(
            Units::new(1.0, UpAxis::Y).unwrap(),
            Units::new(0.001, UpAxis::Z).unwrap(),
        );
        let back = into.inverse();
        let p = Vec3::new(-0.0, 1.5, -2.25);
        let there = into.point(p);
        assert_eq!(there, Vec3::new(-0.0, 2250.0, 1500.0));
        assert_eq!(back.point(there), p);
        assert!(
            back.point(there).x.is_sign_negative(),
            "a negative zero stays one"
        );
        let m = Xform::from_axes(Vec3::Z, Vec3::Y, -Vec3::X, Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(back.placement(&into.placement(&m)), m);
        assert_eq!(back.frame(&into.frame(&m)), m);
        // Lengths divide by what they were multiplied by.
        let mm = Conversion::new(Units::default(), Units::new(0.001, UpAxis::Y).unwrap());
        for length in [0.1, 0.3, 1.7, 1e-7] {
            assert_eq!(mm.inverse().length(mm.length(length)), length);
        }
        assert_eq!((mm.scale(), mm.inverse().scale()), (1000.0, 0.001));
        assert_eq!(mm.inverse().inverse(), mm);
    }

    #[test]
    fn left_handed_axes_mirror_and_bad_ones_are_refused() {
        let left = Conversion::of_axes([-Vec3::X, Vec3::Y, Vec3::Z], 0.01).unwrap();
        assert!(left.mirrors());
        // A placement of no mirroring stays one.
        let [x, y, z] = left.placement(&Xform::IDENTITY).axes();
        assert!(x.cross(y).dot(z) > 0.0);
        assert!(Conversion::of_axes([Vec3::X, Vec3::X, Vec3::Z], 1.0).is_err());
        assert!(Conversion::of_axes([Vec3::X * 2.0, Vec3::Y, Vec3::Z], 1.0).is_err());
        assert!(Conversion::of_axes([Vec3::X, Vec3::Y, Vec3::Z], 0.0).is_err());
        assert!(Conversion::of_axes([Vec3::X, Vec3::Y, Vec3::Z], f64::NAN).is_err());
    }
}
