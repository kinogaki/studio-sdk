//! Ear clipping of one polygon: the one clipper behind both the display
//! and the strict triangulation.
use super::Triangle;
use crate::vector::{self, V2, V3, length_squared2, perp_dot, sub2, turn};

/// One polygon to clip: its points in order, its first vertex id and its
/// primitive id.
pub(super) struct Outline<'a> {
    pub points: &'a [u32],
    pub first: u32,
    pub primitive: u32,
}

/// Appends `outline`'s n − 2 triangles to `out`, or fails with `out` as it
/// was. The plane: the axis plane facing the polygon's area most, then the
/// others, then (for outlines that collapse in every axis, such as a cube
/// with a corner dissolved) the plane across the area normal.
///
/// `tolerant` (display) accepts outlines whose points coincide within
/// rounding or run straight, as cut results make: coincident points don't
/// block an ear, and a vertex without area closes as a flat triangle.
pub(super) fn clip(
    outline: &Outline<'_>,
    position: &impl Fn(u32) -> V3,
    tolerant: bool,
    out: &mut Vec<Triangle>,
) -> Result<(), ()> {
    let ids = outline.points;
    let origin = position(ids[0]);
    let area = ids
        .iter()
        .zip(ids.iter().cycle().skip(1))
        .take(ids.len())
        .map(|(&a, &b)| {
            vector::cross(
                vector::sub(position(a), origin),
                vector::sub(position(b), origin),
            )
        })
        .fold([0.0; 3], vector::add);
    let normal = area.map(f64::abs);
    let mut axes = [0, 1, 2];
    axes.sort_by(|&a, &b| normal[b].total_cmp(&normal[a]));
    let start = out.len();
    for axis in axes {
        if projected(outline, position, Projection::Axis(axis), tolerant, out).is_ok() {
            return Ok(());
        }
        out.truncate(start);
    }
    let largest = normal.into_iter().fold(0.0, f64::max);
    if largest > 0.0 {
        let n = vector::normalize_or_zero(vector::scale(area, 1.0 / largest));
        let helper = if n[0].abs() <= n[1].abs() && n[0].abs() <= n[2].abs() {
            [1.0, 0.0, 0.0]
        } else if n[1].abs() <= n[2].abs() {
            [0.0, 1.0, 0.0]
        } else {
            [0.0, 0.0, 1.0]
        };
        let x = vector::normalize_or_zero(vector::cross(n, helper));
        let plane = Projection::Plane(x, vector::cross(n, x));
        if projected(outline, position, plane, tolerant, out).is_ok() {
            return Ok(());
        }
        out.truncate(start);
    }
    Err(())
}

enum Projection {
    Axis(usize),
    Plane(V3, V3),
}

fn projected(
    outline: &Outline<'_>,
    position: &impl Fn(u32) -> V3,
    projection: Projection,
    tolerant: bool,
    out: &mut Vec<Triangle>,
) -> Result<(), ()> {
    let ids = outline.points;
    let origin = position(ids[0]);
    let project = |&i: &u32| -> V2 {
        let v = vector::sub(position(i), origin);
        match projection {
            Projection::Axis(0) => [v[1], v[2]],
            Projection::Axis(1) => [v[2], v[0]],
            Projection::Axis(_) => [v[0], v[1]],
            Projection::Plane(x, y) => [vector::dot(v, x), vector::dot(v, y)],
        }
    };
    let mut small = [[0.0; 2]; 4];
    let large;
    let p: &[V2] = if ids.len() <= small.len() {
        for (to, from) in small.iter_mut().zip(ids) {
            *to = project(from);
        }
        &small[..ids.len()]
    } else {
        large = ids.iter().map(project).collect::<Vec<_>>();
        &large
    };
    let n = p.len();
    let scale = p
        .iter()
        .map(|v| v[0].abs().max(v[1].abs()))
        .fold(0.0, f64::max);
    let epsilon = (scale * scale * 1e-12).max(f64::MIN_POSITIVE);
    let area = p
        .iter()
        .zip(p.iter().cycle().skip(1))
        .take(n)
        .map(|(a, b)| perp_dot(*a, *b))
        .sum::<f64>();
    if area.abs() <= epsilon {
        return Err(());
    }
    let winding = area.signum();
    let mut emit = |a: usize, b: usize, c: usize| {
        let first = outline.first;
        out.push(Triangle {
            vertices: [first + a as u32, first + b as u32, first + c as u32],
            points: [ids[a], ids[b], ids[c]],
            primitive: outline.primitive,
        });
    };
    // Convex polygons take a fan; concave ones bounded ear clipping.
    let convex = (0..n).all(|i| turn(p[(i + n - 1) % n], p[i], p[(i + 1) % n]) * winding > epsilon);
    if convex {
        // Same-sign turns alone accept a pentagram. A simple convex outline
        // turns once; a triangle or quad with strict same-sign turns is
        // simple, larger rings may wind more than once.
        let turning = if n > 4 {
            (0..n)
                .map(|i| {
                    let a = sub2(p[i], p[(i + n - 1) % n]);
                    let b = sub2(p[(i + 1) % n], p[i]);
                    perp_dot(a, b).atan2(vector::dot2(a, b))
                })
                .sum::<f64>()
        } else {
            std::f64::consts::TAU
        };
        if (turning.abs() - std::f64::consts::TAU).abs() > 1e-8 {
            return Err(());
        }
        for i in 1..n - 1 {
            emit(0, i, i + 1);
        }
        return Ok(());
    }
    // Bound the quadratic simplicity check and the cubic worst-case ear
    // search; large convex polygons take the linear path above.
    if n > 1024 {
        return Err(());
    }
    let near = |a: V2, b: V2| length_squared2(sub2(a, b)) <= epsilon;
    for i in 0..n {
        let next_i = (i + 1) % n;
        for j in i + 1..n {
            let next_j = (j + 1) % n;
            if next_i == j || next_j == i {
                continue;
            }
            // Sides meeting at coincident points (a rounding step apart)
            // touch like neighbors; only a proper crossing is refused.
            let touching = tolerant
                && [p[i], p[next_i]]
                    .iter()
                    .any(|&x| near(x, p[j]) || near(x, p[next_j]));
            if segments_intersect(p[i], p[next_i], p[j], p[next_j], epsilon, touching) {
                return Err(());
            }
        }
    }
    // Points a rounding step apart (a cut's constructed points) count as
    // one place: they never block an ear there.
    let same = |i: usize, j: usize| tolerant && near(p[i], p[j]);
    let mut ring: Vec<usize> = (0..n).collect();
    while ring.len() > 3 {
        let vertex = |k: usize| {
            (
                ring[(k + ring.len() - 1) % ring.len()],
                ring[k],
                ring[(k + 1) % ring.len()],
            )
        };
        let ear = (0..ring.len()).find(|&k| {
            let (a, b, c) = vertex(k);
            turn(p[a], p[b], p[c]) * winding > epsilon
                && !ring.iter().any(|&i| {
                    i != a
                        && i != b
                        && i != c
                        && !same(i, a)
                        && !same(i, b)
                        && !same(i, c)
                        && turn(p[a], p[b], p[i]) * winding >= -epsilon
                        && turn(p[b], p[c], p[i]) * winding >= -epsilon
                        && turn(p[c], p[a], p[i]) * winding >= -epsilon
                })
        });
        // Without a proper ear, a vertex with no area (a point on a straight
        // run, or one of two coincident points) closes as a flat triangle:
        // the outline keeps its shape and every vertex keeps a triangle.
        let ear = ear
            .or_else(|| {
                (0..ring.len()).filter(|_| tolerant).find(|&k| {
                    let (a, b, c) = vertex(k);
                    turn(p[a], p[b], p[c]).abs() <= epsilon
                })
            })
            .ok_or(())?;
        let (a, b, c) = vertex(ear);
        emit(a, b, c);
        ring.remove(ear);
    }
    let last = turn(p[ring[0]], p[ring[1]], p[ring[2]]) * winding;
    let refused = if tolerant {
        last < -epsilon
    } else {
        last <= epsilon
    };
    if refused {
        return Err(());
    }
    emit(ring[0], ring[1], ring[2]);
    Ok(())
}

/// Whether `ab` and `cd` meet; with `crossing_only`, only a proper
/// crossing counts.
fn segments_intersect(a: V2, b: V2, c: V2, d: V2, epsilon: f64, crossing_only: bool) -> bool {
    let orient = |x, y, z| {
        let v = turn(x, y, z);
        if v.abs() <= epsilon {
            0
        } else if v > 0.0 {
            1
        } else {
            -1
        }
    };
    let on = |x: V2, y: V2, z: V2| (0..2).all(|k| z[k] >= x[k].min(y[k]) && z[k] <= x[k].max(y[k]));
    let [ab_c, ab_d, cd_a, cd_b] = [
        orient(a, b, c),
        orient(a, b, d),
        orient(c, d, a),
        orient(c, d, b),
    ];
    (ab_c * ab_d < 0 && cd_a * cd_b < 0)
        || !crossing_only && (ab_c == 0 && on(a, b, c))
        || !crossing_only
            && ((ab_d == 0 && on(a, b, d))
                || (cd_a == 0 && on(c, d, a))
                || (cd_b == 0 && on(c, d, b)))
}
