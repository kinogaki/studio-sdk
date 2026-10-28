//! Which handle a screen point is over.
use super::TransformTool;
use super::geometry::{GizmoFrame, Handle, MANIPULATOR_CUBE, MANIPULATOR_RING, distance, inside};
use crate::geom::Pos;
use crate::vec3::{add, scale};

/// Every axis of a gizmo (the 3D one).
pub const ALL_AXES: [bool; 3] = [true; 3];

/// Whether `axes` hold the plane square about `axis` (both its others).
fn plane_shown(axes: [bool; 3], axis: usize) -> bool {
    axes[(axis + 1) % 3] && axes[(axis + 2) % 3]
}

/// The handle of `tool`'s gizmo at screen point `p`, if any.
pub fn hit(frame: &GizmoFrame, tool: TransformTool, p: Pos) -> Option<Handle> {
    hit_axes(frame, tool, p, ALL_AXES)
}

/// [`hit`] on the handles of `axes` only (a 2D gizmo in a sketch plane:
/// its two in-plane arrows and the plane between them for Move, the ring
/// about its normal for Rotate). The Manipulator keeps every handle.
pub fn hit_axes(
    frame: &GizmoFrame,
    tool: TransformTool,
    p: Pos,
    axes: [bool; 3],
) -> Option<Handle> {
    if tool == TransformTool::Manipulator {
        return hit_manipulator(frame, p);
    }
    let start = frame.project(frame.center)?;
    if tool != TransformTool::Rotate {
        if p.distance(start) < 9.0 {
            return Some(Handle::Center);
        }
        for axis in (0..3).filter(|&a| plane_shown(axes, a)) {
            if frame.plane(axis).is_some_and(|points| inside(p, &points)) {
                return Some(Handle::Plane(axis));
            }
        }
    }
    let mut closest = 8.0;
    let mut found = None;
    for axis in (0..3).filter(|&a| axes[a]) {
        let points = if tool == TransformTool::Rotate {
            frame.ring(axis)
        } else {
            vec![
                frame.project(along(frame, axis, 0.18)),
                frame.project(along(frame, axis, 1.0)),
            ]
        };
        for segment in points.windows(2) {
            if let Some((a, b)) = segment[0].zip(segment[1]) {
                let d = distance(p, a, b);
                if d < closest {
                    closest = d;
                    found = Some(Handle::Axis(axis));
                }
            }
        }
    }
    found
}

/// The Manipulator: the middle (free move), a scale cube, a plane square,
/// an arrow, then a ring, in that order where they overlap.
fn hit_manipulator(frame: &GizmoFrame, p: Pos) -> Option<Handle> {
    let start = frame.project(frame.center)?;
    if p.distance(start) < 9.0 {
        return Some(Handle::Center);
    }
    for axis in 0..3 {
        if frame
            .project(along(frame, axis, MANIPULATOR_CUBE))
            .is_some_and(|c| (c - p).abs().max_elem() < 7.0)
        {
            return Some(Handle::Stretch(axis));
        }
    }
    for axis in 0..3 {
        if frame.plane(axis).is_some_and(|points| inside(p, &points)) {
            return Some(Handle::Plane(axis));
        }
    }
    let mut closest = (8.0, None);
    for axis in 0..3 {
        if let Some((a, b)) = frame
            .project(along(frame, axis, 0.18))
            .zip(frame.project(along(frame, axis, 1.0)))
        {
            let d = distance(p, a, b);
            if d < closest.0 {
                closest = (d, Some(Handle::Axis(axis)));
            }
        }
    }
    if closest.1.is_some() {
        return closest.1;
    }
    for axis in 0..3 {
        for segment in frame.ring_at(axis, MANIPULATOR_RING).windows(2) {
            if let Some((a, b)) = segment[0].zip(segment[1]) {
                let d = distance(p, a, b);
                if d < closest.0 {
                    closest = (d, Some(Handle::Ring(axis)));
                }
            }
        }
    }
    closest.1
}

/// The point `fraction` handle lengths out along `axis` (as Studio
/// computes it: the axis times the length, times the fraction).
fn along(frame: &GizmoFrame, axis: usize, fraction: f64) -> kinogaki_studio_api::Vec3 {
    add(
        frame.center,
        scale(scale(frame.axes[axis], frame.length), fraction),
    )
}

#[cfg(test)]
mod tests {
    use super::super::geometry::tests::frame;
    use super::*;
    use kinogaki_studio_api::Vec3;

    #[test]
    fn the_manipulator_hits_each_part_and_resolves_it_to_a_single_tool() {
        let frame = frame(false);
        let at = |p: Vec3| frame.project(p).unwrap();
        let tool = TransformTool::Manipulator;
        assert_eq!(hit(&frame, tool, at(Vec3::ZERO)), Some(Handle::Center));
        for axis in 0..3 {
            let a = frame.axes[axis];
            assert_eq!(
                hit(&frame, tool, at(scale(a, MANIPULATOR_CUBE))),
                Some(Handle::Stretch(axis))
            );
            assert_eq!(
                hit(&frame, tool, at(scale(a, 0.92))),
                Some(Handle::Axis(axis))
            );
        }
        // A point on the X ring away from the other rings: (0, cos, sin) · R.
        let on_x = scale(Vec3::new(0.0, 0.6, 0.8), MANIPULATOR_RING);
        assert_eq!(hit(&frame, tool, at(on_x)), Some(Handle::Ring(0)));
        assert_eq!(
            tool.resolve(Handle::Ring(1)),
            (TransformTool::Rotate, Handle::Axis(1))
        );
        assert_eq!(
            tool.resolve(Handle::Stretch(2)),
            (TransformTool::Scale, Handle::Axis(2))
        );
        assert_eq!(
            tool.resolve(Handle::Plane(0)),
            (TransformTool::Move, Handle::Plane(0))
        );
        assert_eq!(
            TransformTool::Rotate.resolve(Handle::Axis(1)),
            (TransformTool::Rotate, Handle::Axis(1))
        );
    }

    #[test]
    fn single_tools_hit_their_handles_and_hidden_axes_are_skipped() {
        let frame = frame(false);
        let at = |p: Vec3| frame.project(p).unwrap();
        assert_eq!(
            hit(&frame, TransformTool::Move, at(Vec3::ZERO)),
            Some(Handle::Center)
        );
        assert_eq!(
            hit(&frame, TransformTool::Move, at(Vec3::new(0.9, 0.0, 0.0))),
            Some(Handle::Axis(0))
        );
        assert_eq!(
            hit(&frame, TransformTool::Move, at(Vec3::new(0.3, 0.3, 0.0))),
            Some(Handle::Plane(2))
        );
        assert_eq!(
            hit(&frame, TransformTool::Rotate, at(Vec3::new(0.0, 0.6, 0.8))),
            Some(Handle::Axis(0))
        );
        assert_eq!(
            hit(&frame, TransformTool::Rotate, at(Vec3::ZERO)),
            None,
            "Rotate has no middle"
        );
        let in_plane = [true, true, false];
        assert_eq!(
            hit_axes(
                &frame,
                TransformTool::Move,
                at(Vec3::new(0.0, 0.0, 0.9)),
                in_plane
            ),
            None
        );
        assert_eq!(
            hit_axes(
                &frame,
                TransformTool::Move,
                at(Vec3::new(0.3, 0.3, 0.0)),
                in_plane
            ),
            Some(Handle::Plane(2))
        );
    }
}
