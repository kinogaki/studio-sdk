//! How a surface's faces are drawn: shaded clay, or see-through in a tint
//! (a finished sketch's profiles beside the bodies of one CAD object). A
//! packet lists its see-through triangles as runs of its indices
//! ([`crate::SurfaceSource::face_runs`]); every other triangle is shaded.
//! See-through faces are picked, hovered and selected per face like any.
use std::ops::Range;

/// The look of a surface's (or a merged part's) faces.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub enum FaceLook {
    /// Lit clay (or the surface's colors), opaque.
    #[default]
    Shaded,
    /// Lit `tint` (sRGB) at `opacity` (0–1), drawn after every opaque
    /// surface, depth-tested against them and writing no depth.
    /// Drawn tinted over what is behind.
    SeeThrough {
        /// sRGB.
        tint: [u8; 3],
        /// 0–1.
        opacity: f32,
    },
}

/// Triangles drawn in a look other than shaded: `indices` is a range of
/// the packet's triangle indices (whole triangles, a multiple of three).
#[derive(Clone, Debug, PartialEq)]
pub struct FaceRun {
    /// The run's triangle indices: whole triangles, a multiple of three each end.
    pub indices: Range<u32>,
    /// How the run draws; never [`FaceLook::Shaded`].
    pub look: FaceLook,
}

/// Why `runs` can't be drawn over `indices` triangle indices: runs must
/// be whole triangles, in order, apart and inside the indices, each in a
/// look other than shaded.
pub fn check_runs(runs: &[FaceRun], indices: usize) -> Result<(), String> {
    let mut end = 0u32;
    for r in runs {
        if r.indices.start < end
            || r.indices.start > r.indices.end
            || r.indices.end as usize > indices
            || r.indices.start % 3 != 0
            || r.indices.end % 3 != 0
        {
            return Err(format!(
                "face run {:?} is not whole triangles in order within {indices} indices",
                r.indices
            ));
        }
        match r.look {
            FaceLook::Shaded => return Err("a face run lists shaded triangles".into()),
            FaceLook::SeeThrough { opacity, .. } if !(0.0..=1.0).contains(&opacity) => {
                return Err(format!("see-through opacity {opacity} is outside 0–1"));
            }
            FaceLook::SeeThrough { .. } => {}
        }
        end = r.indices.end;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(indices: Range<u32>) -> FaceRun {
        FaceRun {
            indices,
            look: FaceLook::SeeThrough {
                tint: [0, 0, 255],
                opacity: 0.3,
            },
        }
    }

    #[test]
    fn runs_are_whole_ordered_triangles_inside_the_indices() {
        assert!(check_runs(&[], 0).is_ok());
        assert!(check_runs(&[run(0..3), run(6..12)], 12).is_ok());
        assert!(check_runs(&[run(0..4)], 12).is_err());
        assert!(check_runs(&[run(6..9), run(0..3)], 12).is_err());
        assert!(check_runs(&[run(9..15)], 12).is_err());
        let mut opaque_beyond = run(0..3);
        opaque_beyond.look = FaceLook::SeeThrough {
            tint: [0; 3],
            opacity: 2.0,
        };
        assert!(check_runs(&[opaque_beyond], 3).is_err());
        let mut shaded = run(0..3);
        shaded.look = FaceLook::Shaded;
        assert!(check_runs(&[shaded], 3).is_err());
    }
}
