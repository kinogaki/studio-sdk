//! How a view maps the edited object's space to its pixels and back:
//! [`ClipMatrix`] and [`Projection`], filled in by the host from its
//! camera, for the pure viewport maths in [`gizmo`](crate::gizmo) and
//! [`labels`](crate::labels).
//!
//! Double precision throughout, as the host's camera computes it, so a
//! handle or label lands on the same pixel the host's own code puts it.
use crate::vec3::{dot, scale, sub};
use kinogaki_studio_api::Vec3;

/// A 4×4 matrix of `f64`, column-major (`cols[3]` is the translation):
/// clip space from the edited object's space, or back.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ClipMatrix {
    /// The four columns.
    pub cols: [[f64; 4]; 4],
}

impl ClipMatrix {
    /// The identity.
    pub const IDENTITY: ClipMatrix = ClipMatrix {
        cols: [
            [1.0, 0.0, 0.0, 0.0],
            [0.0, 1.0, 0.0, 0.0],
            [0.0, 0.0, 1.0, 0.0],
            [0.0, 0.0, 0.0, 1.0],
        ],
    };

    /// `v` through the matrix.
    pub fn mul_vec4(&self, v: [f64; 4]) -> [f64; 4] {
        let c = &self.cols;
        std::array::from_fn(|i| c[0][i] * v[0] + c[1][i] * v[1] + c[2][i] * v[2] + c[3][i] * v[3])
    }

    /// The point `p` (w = 1) through the matrix, divided by its w.
    pub fn project_point3(&self, p: Vec3) -> Vec3 {
        let [x, y, z, w] = self.mul_vec4([p.x, p.y, p.z, 1.0]);
        Vec3::new(x / w, y / w, z / w)
    }
}

impl Default for ClipMatrix {
    /// [`ClipMatrix::IDENTITY`].
    fn default() -> Self {
        Self::IDENTITY
    }
}

/// A ray in the edited object's space; `direction` has length 1 (NaN for a
/// degenerate view).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    /// Where it starts, on the near plane.
    pub origin: Vec3,
    /// Where it goes.
    pub direction: Vec3,
}

/// A view's camera as the edited object sees it, for one aspect ratio.
///
/// Clip space is the host's: x and y from −1 to 1 (y up), depth from 0 at
/// the near plane to 1 at the far one. The host builds a projection for a
/// view rectangle's aspect (width over height); one built for another
/// aspect puts things in the wrong place.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Projection {
    /// Clip space from the edited object's space (the camera's view and
    /// projection times the object's placement).
    pub clip_from_object: ClipMatrix,
    /// Its inverse.
    pub object_from_clip: ClipMatrix,
    /// The direction towards the viewer, in the object's space, length 1.
    pub backward: Vec3,
}

impl Projection {
    /// Where `p` shows in a view of `size` pixels (from its top left), or
    /// `None` when it is behind the eye or outside the depth range.
    pub fn project(&self, p: Vec3, size: [f64; 2]) -> Option<[f64; 2]> {
        let [x, y, z, w] = self.clip_from_object.mul_vec4([p.x, p.y, p.z, 1.0]);
        if w <= 0.0 {
            return None;
        }
        let (x, y, z) = (x / w, y / w, z / w);
        if !(0.0..=1.0).contains(&z) {
            return None;
        }
        Some([(x + 1.0) * 0.5 * size[0], (1.0 - y) * 0.5 * size[1]])
    }

    /// The ray through `uv`, the view's fraction across (0 left, 1 right)
    /// and down (0 top, 1 bottom), from the near plane to the far one.
    pub fn ray(&self, uv: [f64; 2]) -> Ray {
        let (x, y) = (uv[0] * 2.0 - 1.0, 1.0 - uv[1] * 2.0);
        let a = self.object_from_clip.project_point3(Vec3::new(x, y, 0.0));
        let b = self.object_from_clip.project_point3(Vec3::new(x, y, 1.0));
        let d = sub(b, a);
        Ray {
            origin: a,
            direction: scale(d, 1.0 / dot(d, d).sqrt()),
        }
    }
}

#[cfg(test)]
pub(crate) mod test_camera {
    //! A test orbit camera, as Studio's viewport builds its matrices
    //! (right-handed look-at; perspective or orthographic with depth from 0
    //! to 1), so the viewport maths' tests see real projections.
    use super::{ClipMatrix, Projection};
    use crate::vec3::{add, cross, dot, length, scale, sub};
    use kinogaki_studio_api::Vec3;

    /// Orbiting `target` at `distance` from yaw 0.7 and pitch 0.5 (the
    /// viewport's default view), Y up.
    pub(crate) fn projection(
        target: Vec3,
        distance: f64,
        orthographic: bool,
        aspect: f64,
    ) -> Projection {
        let (yaw, pitch) = (0.7f64, 0.5f64);
        let backward = Vec3::new(
            yaw.sin() * pitch.cos(),
            pitch.sin(),
            yaw.cos() * pitch.cos(),
        );
        let up = Vec3::new(
            -yaw.sin() * pitch.sin(),
            pitch.cos(),
            -yaw.cos() * pitch.sin(),
        );
        let reach = if orthographic {
            (distance * 0.41421356237 * 400.0).max(distance)
        } else {
            distance
        };
        let eye = add(target, scale(backward, reach));
        let (near, far) = if orthographic {
            (reach * 0.0001, reach * 2.0)
        } else {
            (
                (distance * 0.01).max(0.00001),
                (distance * 20.0).clamp(500.0, 1e9),
            )
        };
        let projection = if orthographic {
            let h = distance * 0.41421356237;
            orthographic_rh(-h * aspect, h * aspect, -h, h, near, far)
        } else {
            perspective_rh(2.0 * 0.41421356237f64.atan(), aspect, near, far)
        };
        let clip = multiply(&projection, &look_at_rh(eye, target, up));
        Projection {
            clip_from_object: clip,
            object_from_clip: inverse(&clip),
            backward,
        }
    }

    fn look_at_rh(eye: Vec3, center: Vec3, up: Vec3) -> ClipMatrix {
        let f = normalize(sub(center, eye));
        let s = normalize(cross(f, up));
        let u = cross(s, f);
        ClipMatrix {
            cols: [
                [s.x, u.x, -f.x, 0.0],
                [s.y, u.y, -f.y, 0.0],
                [s.z, u.z, -f.z, 0.0],
                [-dot(eye, s), -dot(eye, u), dot(eye, f), 1.0],
            ],
        }
    }

    fn perspective_rh(fov_y: f64, aspect: f64, near: f64, far: f64) -> ClipMatrix {
        let h = 1.0 / (0.5 * fov_y).tan();
        let r = far / (near - far);
        ClipMatrix {
            cols: [
                [h / aspect, 0.0, 0.0, 0.0],
                [0.0, h, 0.0, 0.0],
                [0.0, 0.0, r, -1.0],
                [0.0, 0.0, r * near, 0.0],
            ],
        }
    }

    fn orthographic_rh(
        left: f64,
        right: f64,
        bottom: f64,
        top: f64,
        near: f64,
        far: f64,
    ) -> ClipMatrix {
        let (rw, rh, r) = (
            1.0 / (right - left),
            1.0 / (top - bottom),
            1.0 / (near - far),
        );
        ClipMatrix {
            cols: [
                [rw + rw, 0.0, 0.0, 0.0],
                [0.0, rh + rh, 0.0, 0.0],
                [0.0, 0.0, r, 0.0],
                [-(left + right) * rw, -(top + bottom) * rh, r * near, 1.0],
            ],
        }
    }

    fn normalize(v: Vec3) -> Vec3 {
        scale(v, 1.0 / length(v))
    }

    fn multiply(a: &ClipMatrix, b: &ClipMatrix) -> ClipMatrix {
        ClipMatrix {
            cols: b.cols.map(|c| a.mul_vec4(c)),
        }
    }

    /// The inverse by cofactors.
    fn inverse(m: &ClipMatrix) -> ClipMatrix {
        let a: [f64; 16] = std::array::from_fn(|i| m.cols[i / 4][i % 4]);
        let mut inv = [0.0; 16];
        inv[0] = a[5] * a[10] * a[15] - a[5] * a[11] * a[14] - a[9] * a[6] * a[15]
            + a[9] * a[7] * a[14]
            + a[13] * a[6] * a[11]
            - a[13] * a[7] * a[10];
        inv[4] = -a[4] * a[10] * a[15] + a[4] * a[11] * a[14] + a[8] * a[6] * a[15]
            - a[8] * a[7] * a[14]
            - a[12] * a[6] * a[11]
            + a[12] * a[7] * a[10];
        inv[8] = a[4] * a[9] * a[15] - a[4] * a[11] * a[13] - a[8] * a[5] * a[15]
            + a[8] * a[7] * a[13]
            + a[12] * a[5] * a[11]
            - a[12] * a[7] * a[9];
        inv[12] = -a[4] * a[9] * a[14] + a[4] * a[10] * a[13] + a[8] * a[5] * a[14]
            - a[8] * a[6] * a[13]
            - a[12] * a[5] * a[10]
            + a[12] * a[6] * a[9];
        inv[1] = -a[1] * a[10] * a[15] + a[1] * a[11] * a[14] + a[9] * a[2] * a[15]
            - a[9] * a[3] * a[14]
            - a[13] * a[2] * a[11]
            + a[13] * a[3] * a[10];
        inv[5] = a[0] * a[10] * a[15] - a[0] * a[11] * a[14] - a[8] * a[2] * a[15]
            + a[8] * a[3] * a[14]
            + a[12] * a[2] * a[11]
            - a[12] * a[3] * a[10];
        inv[9] = -a[0] * a[9] * a[15] + a[0] * a[11] * a[13] + a[8] * a[1] * a[15]
            - a[8] * a[3] * a[13]
            - a[12] * a[1] * a[11]
            + a[12] * a[3] * a[9];
        inv[13] = a[0] * a[9] * a[14] - a[0] * a[10] * a[13] - a[8] * a[1] * a[14]
            + a[8] * a[2] * a[13]
            + a[12] * a[1] * a[10]
            - a[12] * a[2] * a[9];
        inv[2] = a[1] * a[6] * a[15] - a[1] * a[7] * a[14] - a[5] * a[2] * a[15]
            + a[5] * a[3] * a[14]
            + a[13] * a[2] * a[7]
            - a[13] * a[3] * a[6];
        inv[6] = -a[0] * a[6] * a[15] + a[0] * a[7] * a[14] + a[4] * a[2] * a[15]
            - a[4] * a[3] * a[14]
            - a[12] * a[2] * a[7]
            + a[12] * a[3] * a[6];
        inv[10] = a[0] * a[5] * a[15] - a[0] * a[7] * a[13] - a[4] * a[1] * a[15]
            + a[4] * a[3] * a[13]
            + a[12] * a[1] * a[7]
            - a[12] * a[3] * a[5];
        inv[14] = -a[0] * a[5] * a[14] + a[0] * a[6] * a[13] + a[4] * a[1] * a[14]
            - a[4] * a[2] * a[13]
            - a[12] * a[1] * a[6]
            + a[12] * a[2] * a[5];
        inv[3] = -a[1] * a[6] * a[11] + a[1] * a[7] * a[10] + a[5] * a[2] * a[11]
            - a[5] * a[3] * a[10]
            - a[9] * a[2] * a[7]
            + a[9] * a[3] * a[6];
        inv[7] = a[0] * a[6] * a[11] - a[0] * a[7] * a[10] - a[4] * a[2] * a[11]
            + a[4] * a[3] * a[10]
            + a[8] * a[2] * a[7]
            - a[8] * a[3] * a[6];
        inv[11] = -a[0] * a[5] * a[11] + a[0] * a[7] * a[9] + a[4] * a[1] * a[11]
            - a[4] * a[3] * a[9]
            - a[8] * a[1] * a[7]
            + a[8] * a[3] * a[5];
        inv[15] = a[0] * a[5] * a[10] - a[0] * a[6] * a[9] - a[4] * a[1] * a[10]
            + a[4] * a[2] * a[9]
            + a[8] * a[1] * a[6]
            - a[8] * a[2] * a[5];
        let det = a[0] * inv[0] + a[1] * inv[4] + a[2] * inv[8] + a[3] * inv[12];
        ClipMatrix {
            cols: std::array::from_fn(|c| std::array::from_fn(|r| inv[c * 4 + r] / det)),
        }
    }

    #[test]
    fn the_inverse_undoes_the_matrix() {
        let p = projection(Vec3::new(0.0, 0.8, 0.0), 9.0, false, 1.5);
        let q = Vec3::new(0.3, -0.2, 0.7);
        let back = p
            .object_from_clip
            .project_point3(p.clip_from_object.project_point3(q));
        assert!(length(sub(back, q)) < 1e-9, "{back:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::test_camera::projection;
    use super::*;
    use crate::vec3::length;

    #[test]
    fn the_target_projects_to_the_middle_and_rays_pass_through_it() {
        for orthographic in [false, true] {
            let target = Vec3::new(0.0, 0.8, 0.0);
            let p = projection(target, 9.0, orthographic, 800.0 / 600.0);
            let [x, y] = p.project(target, [800.0, 600.0]).unwrap();
            assert!((x - 400.0).abs() < 1e-6 && (y - 300.0).abs() < 1e-6);
            let ray = p.ray([0.5, 0.5]);
            assert!((length(ray.direction) - 1.0).abs() < 1e-12);
            assert!(
                (dot(ray.direction, p.backward) + 1.0).abs() < 1e-9,
                "looks away from the viewer"
            );
            let to_target = sub(target, ray.origin);
            let along = dot(to_target, ray.direction);
            assert!(length(sub(to_target, scale(ray.direction, along))) < 1e-6);
        }
    }

    #[test]
    fn points_behind_the_eye_do_not_project() {
        let p = projection(Vec3::ZERO, 4.0, false, 1.0);
        let behind = scale(p.backward, 10.0);
        assert_eq!(p.project(behind, [100.0, 100.0]), None);
    }
}
