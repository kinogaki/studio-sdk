//! Handle geometry in screen space: the gizmo frame's projection, plane
//! squares and rotation rings, and drag deltas along axes and planes.
use crate::geom::{Pos, Rect, Vec2, vec2};
use crate::projection::Projection;
use crate::vec3::{add, dot, is_finite, scale as times, sub};
use kinogaki_studio_api::Vec3;

/// A gizmo handle. Axes are 0, 1 and 2 (the frame's X, Y and Z).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Handle {
    /// An axis's arrow (Move), square (Scale) or ring (Rotate).
    Axis(usize),
    /// The square in the plane normal to an axis.
    Plane(usize),
    /// The middle: free move, or uniform scale.
    Center,
    /// The Manipulator's rotation ring about an axis.
    Ring(usize),
    /// The Manipulator's scale cube on an axis.
    Stretch(usize),
}

/// The Manipulator's rings, as a fraction of the handle length (outside
/// the arrows' tips).
pub const MANIPULATOR_RING: f64 = 1.25;
/// The Manipulator's scale cubes' place along each arrow, as a fraction of
/// the handle length.
pub const MANIPULATOR_CUBE: f64 = 0.62;

/// Where a gizmo is: its middle and axes in the edited object's space,
/// how long its handles are, and the view it shows in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GizmoFrame {
    /// The middle.
    pub center: Vec3,
    /// The X, Y and Z axes' directions (unit length).
    pub axes: [Vec3; 3],
    /// The handles' length, in object units (the host sizes it to a
    /// constant length on screen).
    pub length: f64,
    /// The view's projection, for `rect`'s aspect.
    pub projection: Projection,
    /// The view's rectangle on screen.
    pub rect: Rect,
}

impl GizmoFrame {
    /// Where `p` (object space) shows on screen, if in view.
    pub fn project(&self, p: Vec3) -> Option<Pos> {
        let size = [self.rect.width() as f64, self.rect.height() as f64];
        self.projection
            .project(p, size)
            .map(|[x, y]| self.rect.min + vec2(x as f32, y as f32))
    }

    /// The plane square about `axis`: its four corners on screen.
    pub fn plane(&self, axis: usize) -> Option<Vec<Pos>> {
        [[0.23, 0.23], [0.40, 0.23], [0.40, 0.40], [0.23, 0.40]]
            .into_iter()
            .map(|[a, b]| {
                let across = add(
                    times(self.axes[(axis + 1) % 3], a),
                    times(self.axes[(axis + 2) % 3], b),
                );
                self.project(add(self.center, times(across, self.length)))
            })
            .collect()
    }

    /// The ring about `axis`, one handle length out: 65 points, the last
    /// the first again (`None` where a point is out of view).
    pub fn ring(&self, axis: usize) -> Vec<Option<Pos>> {
        self.ring_at(axis, 1.0)
    }

    /// The ring about `axis` at `radius` handle lengths.
    pub fn ring_at(&self, axis: usize, radius: f64) -> Vec<Option<Pos>> {
        (0..=64)
            .map(|i| {
                let angle = i as f64 * std::f64::consts::TAU / 64.0;
                let around = add(
                    times(self.axes[(axis + 1) % 3], angle.cos()),
                    times(self.axes[(axis + 2) % 3], angle.sin()),
                );
                self.project(add(self.center, times(times(around, self.length), radius)))
            })
            .collect()
    }

    /// How far a drag of `handle` from `start` to `at` (screen) moves, in
    /// object space: along the axis for an axis handle, in the plane for a
    /// plane square, and in the view's plane through the middle otherwise.
    pub fn delta(&self, start: Pos, at: Pos, handle: Handle) -> Vec3 {
        if let Handle::Axis(axis) = handle {
            let tip = add(self.center, times(self.axes[axis], self.length));
            let Some((a, b)) = self.project(self.center).zip(self.project(tip)) else {
                return Vec3::ZERO;
            };
            let line = b - a;
            if line.length_sq() < 1.0 {
                return Vec3::ZERO;
            }
            let along = ((at - start).dot(line) / line.length_sq()) as f64;
            return times(times(self.axes[axis], along), self.length);
        }
        let normal = if let Handle::Plane(axis) = handle {
            self.axes[axis]
        } else {
            self.projection.backward
        };
        self.on_plane(start, normal)
            .zip(self.on_plane(at, normal))
            .map_or(Vec3::ZERO, |(a, b)| sub(b, a))
    }

    /// Where the ray under screen point `p` meets the plane through the
    /// middle with `normal`.
    fn on_plane(&self, p: Pos, normal: Vec3) -> Option<Vec3> {
        let local = p - self.rect.min;
        let ray = self.projection.ray([
            (local.x / self.rect.width()) as f64,
            (local.y / self.rect.height()) as f64,
        ]);
        let facing = dot(ray.direction, normal);
        if facing.abs() < 1e-7 {
            return None;
        }
        let reach = dot(sub(self.center, ray.origin), normal);
        let d = times(ray.direction, reach);
        let point = add(
            ray.origin,
            Vec3::new(d.x / facing, d.y / facing, d.z / facing),
        );
        is_finite(point).then_some(point)
    }
}

/// The distance from `p` to the segment from `a` to `b`.
pub fn distance(p: Pos, a: Pos, b: Pos) -> f32 {
    let line = b - a;
    p.distance(a + line * ((p - a).dot(line) / line.length_sq().max(1e-8)).clamp(0.0, 1.0))
}

/// Whether `p` is inside the convex `polygon` (either winding); a polygon
/// of almost no area holds nothing.
pub fn inside(p: Pos, polygon: &[Pos]) -> bool {
    let mut sign = 0.0_f32;
    let mut area = 0.0_f32;
    for (&a, &b) in polygon
        .iter()
        .zip(polygon.iter().cycle().skip(1))
        .take(polygon.len())
    {
        let edge = b - a;
        area += a.x * b.y - b.x * a.y;
        let cross = edge.x * (p.y - a.y) - edge.y * (p.x - a.x);
        if sign != 0.0 && sign * cross < -1e-4 {
            return false;
        }
        if cross.abs() > 1e-4 {
            sign = cross.signum();
        }
    }
    area.abs() > 4.0
}

/// The turn from angle `previous` to `current` (radians), the short way
/// round, so a drag across the ±π seam keeps its direction.
pub fn angle_change(previous: f64, current: f64) -> f64 {
    (current - previous + std::f64::consts::PI).rem_euclid(std::f64::consts::TAU)
        - std::f64::consts::PI
}

/// `value` rounded to a multiple of `step` when snapping.
pub fn stepped(value: f64, step: f64, snap: bool) -> f64 {
    if snap {
        (value / step).round() * step
    } else {
        value
    }
}

/// The scale factors a Scale drag of `handle` means: along its axis (the
/// object-space `delta` over the handle length), the two axes of a plane
/// square, or all three for the middle (by the screen drag `pixels`, 100
/// points doubling). Snapping steps by 0.1; factors stay above 0.001.
pub fn scale(frame: &GizmoFrame, handle: Handle, delta: Vec3, pixels: Vec2, snap: bool) -> Vec3 {
    let mut factors = [1.0; 3];
    for (axis, factor) in factors.iter_mut().enumerate() {
        if handle == Handle::Axis(axis)
            || handle == Handle::Center
            || matches!(handle, Handle::Plane(a) if a != axis)
        {
            let amount = if handle == Handle::Center {
                (pixels.x - pixels.y) as f64 / 100.0
            } else {
                dot(delta, frame.axes[axis]) / frame.length
            };
            *factor = stepped(1.0 + amount, 0.1, snap).max(0.001);
        }
    }
    Vec3::from_array(factors)
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use crate::geom::pos;
    use crate::projection::test_camera::projection;
    use crate::vec3::length;

    /// A unit gizmo at the origin of an 800 × 600 view, the default camera
    /// orbiting the origin.
    pub(crate) fn frame(orthographic: bool) -> GizmoFrame {
        GizmoFrame {
            center: Vec3::ZERO,
            axes: [
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
            ],
            length: 1.0,
            projection: projection(Vec3::ZERO, 9.0, orthographic, 800.0 / 600.0),
            rect: Rect::from_min_size(Pos::ZERO, vec2(800.0, 600.0)),
        }
    }

    #[test]
    fn dragging_to_axis_end_moves_one_handle_length_in_both_views() {
        for ortho in [false, true] {
            let f = frame(ortho);
            for axis in 0..3 {
                let start = f.project(f.center).unwrap();
                let end = f.project(add(f.center, f.axes[axis])).unwrap();
                let moved = f.delta(start, end, Handle::Axis(axis));
                assert!(length(sub(moved, f.axes[axis])) < 1e-5, "{moved:?}");
            }
        }
    }

    #[test]
    fn plane_drag_keeps_excluded_axis_and_scale_keeps_other_axes() {
        let f = frame(true);
        let start = f.project(Vec3::ZERO).unwrap();
        let end = f.project(Vec3::new(0.4, 0.7, 0.0)).unwrap();
        let delta = f.delta(start, end, Handle::Plane(2));
        assert!(length(sub(delta, Vec3::new(0.4, 0.7, 0.0))) < 1e-5);
        assert_eq!(
            scale(
                &f,
                Handle::Axis(0),
                Vec3::new(0.5, 0.0, 0.0),
                Vec2::ZERO,
                false
            ),
            Vec3::new(1.5, 1.0, 1.0)
        );
        assert_eq!(
            scale(&f, Handle::Center, Vec3::ZERO, vec2(25.0, -25.0), false),
            Vec3::new(1.5, 1.5, 1.5)
        );
    }

    #[test]
    fn a_free_drag_stays_in_the_view_plane() {
        let f = frame(false);
        let start = f.project(Vec3::ZERO).unwrap();
        let moved = f.delta(start, start + vec2(40.0, -30.0), Handle::Center);
        assert!(length(moved) > 0.1);
        assert!(dot(moved, f.projection.backward).abs() < 1e-9);
    }

    #[test]
    fn rotation_crosses_angle_seam_without_reversing() {
        let change = angle_change(179f64.to_radians(), -179f64.to_radians()).to_degrees();
        assert!((change - 2.0).abs() < 1e-10);
        assert!(
            (angle_change(-179f64.to_radians(), 179f64.to_radians()).to_degrees() + 2.0).abs()
                < 1e-10
        );
        assert_eq!(stepped(29.0, 15.0, true), 30.0);
    }

    #[test]
    fn points_measure_to_segments_and_polygons() {
        assert_eq!(distance(pos(5.0, 3.0), pos(0.0, 0.0), pos(10.0, 0.0)), 3.0);
        assert_eq!(distance(pos(-4.0, 3.0), pos(0.0, 0.0), pos(10.0, 0.0)), 5.0);
        let square = [
            pos(0.0, 0.0),
            pos(10.0, 0.0),
            pos(10.0, 10.0),
            pos(0.0, 10.0),
        ];
        assert!(inside(pos(5.0, 5.0), &square));
        let mut reversed = square;
        reversed.reverse();
        assert!(inside(pos(5.0, 5.0), &reversed));
        assert!(!inside(pos(15.0, 5.0), &square));
        assert!(!inside(
            pos(0.5, 0.5),
            &[pos(0.0, 0.0), pos(1.0, 0.0), pos(1.0, 1.0)]
        ));
    }
}
