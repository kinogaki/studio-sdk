//! [`Subdivision`]: how a polygons value renders as a subdivision surface,
//! the setting UsdGeomMesh holds on a mesh, kept on the value as standard
//! Detail attributes so it travels with the geometry through edits, merges,
//! files and saves.
use crate::attributes::{Attribute, Attributes, Domain, Meaning, Text, names};
use crate::error::Error;
use crate::geometry::Geometry;

/// How a polygons value subdivides when it renders: UsdGeomMesh's setting,
/// kept on the value as standard Detail attributes.
///
/// | Attribute | Values | Absent |
/// |---|---|---|
/// | [`SUBDIVISION_SCHEME`](names::SUBDIVISION_SCHEME) | `catmull_clark`, `loop`, `bilinear` | no subdivision: polygons |
/// | [`SUBDIVISION_LEVEL`](names::SUBDIVISION_LEVEL) | refinements, 0 to [`MAX_LEVEL`](Self::MAX_LEVEL) | [`DEFAULT_LEVEL`](Self::DEFAULT_LEVEL) |
/// | [`SUBDIVISION_BOUNDARY`](names::SUBDIVISION_BOUNDARY) | `edge_and_corner`, `edge_only`, `none` | `edge_and_corner` |
/// | [`SUBDIVISION_FACE_VARYING`](names::SUBDIVISION_FACE_VARYING) | `none`, `corners_only`, `corners_plus1`, `corners_plus2`, `boundaries`, `all` | `corners_plus1` |
/// | [`SUBDIVISION_TRIANGLES`](names::SUBDIVISION_TRIANGLES) | `catmull_clark`, `smooth` | `catmull_clark` |
///
/// The absent values are USD's fallbacks, as are the meanings (OpenSubdiv's
/// options). What the surface is shaped by besides lives on the elements:
/// [`CREASE`](names::CREASE) on edges (sharpness) and points (corner
/// sharpness), and the faces of the [`SUBDIVISION_HOLE`](names::SUBDIVISION_HOLE)
/// group, which render as nothing.
///
/// ```
/// use kinogaki_geometry::{Attributes, Scheme, Subdivision};
///
/// let smooth = Subdivision::new(Scheme::CatmullClark).with_level(3);
/// let attributes = smooth.write(&Attributes::new()).unwrap();
/// assert_eq!(Subdivision::read(&attributes), Some(smooth));
/// assert_eq!(Subdivision::read(&Subdivision::clear(&attributes)), None);
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Subdivision {
    /// How new points are placed.
    pub scheme: Scheme,
    /// How many times it is refined.
    pub level: u32,
    /// How its border subdivides.
    pub boundary: Boundary,
    /// How Vertex attributes (USD's face-varying primvars) interpolate.
    pub face_varying: FaceVarying,
    /// Catmull-Clark's rule for triangles.
    pub triangles: Triangles,
}

/// Defines a setting's choices and their keys, the text its attribute holds.
macro_rules! keyed {
    ($(#[$doc:meta])* $name:ident { $($(#[$v:meta])* $variant:ident = $key:literal,)+ }) => {
        $(#[$doc])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name {
            $($(#[$v])* $variant,)+
        }

        impl $name {
            /// Every choice.
            pub const ALL: &[$name] = &[$($name::$variant),+];

            /// The text its attribute holds.
            pub fn key(self) -> &'static str {
                match self {
                    $($name::$variant => $key,)+
                }
            }

            /// The choice `key` names.
            pub fn parse(key: &str) -> Option<Self> {
                match key {
                    $($key => Some($name::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

keyed! {
    /// How new points are placed (UsdGeomMesh's `subdivisionScheme`).
    Scheme {
        /// Catmull-Clark: smooth, for any polygons.
        CatmullClark = "catmull_clark",
        /// Loop: smooth, for triangles (other polygons split into them).
        Loop = "loop",
        /// The same cuts as Catmull-Clark, nothing rounding.
        Bilinear = "bilinear",
    }
}

keyed! {
    /// How the border subdivides (`interpolateBoundary`).
    Boundary {
        /// The border a sharp curve, sharp at its one-face corners.
        EdgeAndCorner = "edge_and_corner",
        /// The border a sharp curve, smooth through its corners.
        EdgeOnly = "edge_only",
        /// No border: the faces touching it are holes.
        None = "none",
    }
}

keyed! {
    /// How Vertex attributes interpolate (`faceVaryingLinearInterpolation`).
    FaceVarying {
        /// Smooth everywhere.
        None = "none",
        /// Linear at the corners of their seams.
        CornersOnly = "corners_only",
        /// As corners only, and where three or more values meet.
        CornersPlus1 = "corners_plus1",
        /// As corners plus 1, and at darts and seam corners.
        CornersPlus2 = "corners_plus2",
        /// Linear along their seams.
        Boundaries = "boundaries",
        /// Linear everywhere.
        All = "all",
    }
}

keyed! {
    /// Catmull-Clark's rule for triangles (`triangleSubdivisionRule`).
    Triangles {
        /// The standard rule.
        CatmullClark = "catmull_clark",
        /// The smoother weights for triangles.
        Smooth = "smooth",
    }
}

impl Subdivision {
    /// The level when none is given.
    pub const DEFAULT_LEVEL: u32 = 2;
    /// The most refinements.
    pub const MAX_LEVEL: u32 = 8;

    /// `scheme` at the default level, USD's fallbacks for the rest.
    pub fn new(scheme: Scheme) -> Self {
        Self {
            scheme,
            level: Self::DEFAULT_LEVEL,
            boundary: Boundary::EdgeAndCorner,
            face_varying: FaceVarying::CornersPlus1,
            triangles: Triangles::CatmullClark,
        }
    }

    /// At `level` refinements, at most [`MAX_LEVEL`](Self::MAX_LEVEL).
    pub fn with_level(self, level: u32) -> Self {
        Self {
            level: level.min(Self::MAX_LEVEL),
            ..self
        }
    }

    /// `value`'s setting: `None` when it holds no scheme (or one no
    /// [`Scheme`] names), so it is polygons.
    pub fn of(value: &Geometry) -> Option<Self> {
        Self::read(value.attributes())
    }

    /// The setting `attributes` hold (as [`of`](Self::of)): an absent or
    /// unknown value is its fallback, a level out of range the nearest.
    pub fn read(attributes: &Attributes) -> Option<Self> {
        let text = |name| {
            let text = attributes.text(Domain::Detail, name)?;
            text.get(0)
        };
        let scheme = Scheme::parse(text(names::SUBDIVISION_SCHEME)?)?;
        let level = (attributes.tuples::<i32>(Domain::Detail, names::SUBDIVISION_LEVEL))
            .and_then(|l| l.first().copied())
            .map_or(Self::DEFAULT_LEVEL, |l| {
                l.clamp(0, Self::MAX_LEVEL as i32) as u32
            });
        let base = Self::new(scheme).with_level(level);
        Some(Self {
            boundary: text(names::SUBDIVISION_BOUNDARY)
                .and_then(Boundary::parse)
                .unwrap_or(base.boundary),
            face_varying: text(names::SUBDIVISION_FACE_VARYING)
                .and_then(FaceVarying::parse)
                .unwrap_or(base.face_varying),
            triangles: text(names::SUBDIVISION_TRIANGLES)
                .and_then(Triangles::parse)
                .unwrap_or(base.triangles),
            ..base
        })
    }

    /// `attributes` holding this setting: its scheme and level, and each
    /// other value where it isn't the fallback (removed where it is).
    ///
    /// # Errors
    /// `attributes` hold one of the names as another type (which the names
    /// table refuses).
    pub fn write(&self, attributes: &Attributes) -> Result<Attributes, Error> {
        let text = |key: &str| Attribute::text(Text::from_strs([key]));
        let mut out = Self::write_scheme(attributes, self.scheme, self.level)?;
        let base = Self::new(self.scheme);
        for (name, key, fallback) in [
            (
                names::SUBDIVISION_BOUNDARY,
                self.boundary.key(),
                base.boundary.key(),
            ),
            (
                names::SUBDIVISION_FACE_VARYING,
                self.face_varying.key(),
                base.face_varying.key(),
            ),
            (
                names::SUBDIVISION_TRIANGLES,
                self.triangles.key(),
                base.triangles.key(),
            ),
        ] {
            out = match key == fallback {
                true => out.without(Domain::Detail, name),
                false => out.with(Domain::Detail, name, text(key))?,
            };
        }
        Ok(out)
    }

    /// `attributes` subdividing by `scheme` at `level` (at most
    /// [`MAX_LEVEL`](Self::MAX_LEVEL)), their other values as they are.
    ///
    /// # Errors
    /// As [`write`](Self::write).
    pub fn write_scheme(
        attributes: &Attributes,
        scheme: Scheme,
        level: u32,
    ) -> Result<Attributes, Error> {
        let level = level.min(Self::MAX_LEVEL) as i32;
        let level = Attribute::tuples(vec![level], Meaning::None)?;
        let scheme = Attribute::text(Text::from_strs([scheme.key()]));
        attributes
            .with(Domain::Detail, names::SUBDIVISION_SCHEME, scheme)?
            .with(Domain::Detail, names::SUBDIVISION_LEVEL, level)
    }

    /// `attributes` without a scheme or level: polygons again. The other
    /// values stay, for when a scheme is set again.
    pub fn clear(attributes: &Attributes) -> Attributes {
        attributes
            .without(Domain::Detail, names::SUBDIVISION_SCHEME)
            .without(Domain::Detail, names::SUBDIVISION_LEVEL)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn detail(attributes: &Attributes, name: &str) -> Option<String> {
        let text = attributes.text(Domain::Detail, name)?;
        text.get(0).map(str::to_owned)
    }

    #[test]
    fn a_setting_writes_its_fallbacks_nowhere_and_reads_back() {
        let plain = Subdivision::new(Scheme::Loop).with_level(1);
        let written = plain.write(&Attributes::new()).unwrap();
        assert_eq!(
            detail(&written, names::SUBDIVISION_SCHEME).as_deref(),
            Some("loop")
        );
        assert_eq!(
            written.tuples::<i32>(Domain::Detail, names::SUBDIVISION_LEVEL),
            Some(&[1][..])
        );
        assert!(
            written
                .get(Domain::Detail, names::SUBDIVISION_BOUNDARY)
                .is_none()
        );
        assert_eq!(Subdivision::read(&written), Some(plain));
        let every = Subdivision {
            boundary: Boundary::None,
            face_varying: FaceVarying::All,
            triangles: Triangles::Smooth,
            ..Subdivision::new(Scheme::CatmullClark)
        };
        let written = every.write(&written).unwrap();
        assert_eq!(
            detail(&written, names::SUBDIVISION_FACE_VARYING).as_deref(),
            Some("all")
        );
        assert_eq!(Subdivision::read(&written), Some(every));
        // Back to the fallbacks: their attributes go.
        let back = Subdivision::new(Scheme::Bilinear).write(&written).unwrap();
        assert!(
            back.get(Domain::Detail, names::SUBDIVISION_TRIANGLES)
                .is_none()
        );
        assert_eq!(
            Subdivision::read(&back),
            Some(Subdivision::new(Scheme::Bilinear))
        );
    }

    #[test]
    fn no_scheme_is_polygons_and_the_rest_falls_back() {
        assert_eq!(Subdivision::read(&Attributes::new()), None);
        let named = |key: &str| {
            let text = Attribute::text(Text::from_strs([key]));
            Attributes::new()
                .with(Domain::Detail, names::SUBDIVISION_SCHEME, text)
                .unwrap()
        };
        assert_eq!(Subdivision::read(&named("none")), None);
        assert_eq!(Subdivision::read(&named("butterfly")), None);
        let only = Subdivision::read(&named("catmull_clark")).unwrap();
        assert_eq!(only, Subdivision::new(Scheme::CatmullClark));
        assert_eq!(only.level, Subdivision::DEFAULT_LEVEL);
        let deep = Attribute::tuples(vec![40_i32], Meaning::None).unwrap();
        let deep = named("loop")
            .with(Domain::Detail, names::SUBDIVISION_LEVEL, deep)
            .unwrap();
        assert_eq!(
            Subdivision::read(&deep).unwrap().level,
            Subdivision::MAX_LEVEL
        );
        let cleared = Subdivision::clear(&deep);
        assert!(cleared.is_empty());
    }

    #[test]
    fn every_key_parses_back() {
        for s in Scheme::ALL {
            assert_eq!(Scheme::parse(s.key()), Some(*s));
        }
        for b in Boundary::ALL {
            assert_eq!(Boundary::parse(b.key()), Some(*b));
        }
        for f in FaceVarying::ALL {
            assert_eq!(FaceVarying::parse(f.key()), Some(*f));
        }
        for t in Triangles::ALL {
            assert_eq!(Triangles::parse(t.key()), Some(*t));
        }
    }

    #[test]
    fn the_names_hold_their_standard_types_alone() {
        let wrong = Attribute::tuples(vec![1.0_f32], Meaning::None).unwrap();
        assert!(
            Attributes::new()
                .with(Domain::Detail, names::SUBDIVISION_SCHEME, wrong.clone())
                .is_err()
        );
        let level = Attribute::tuples(vec![2_i32], Meaning::None).unwrap();
        assert!(
            Attributes::new()
                .with(Domain::Point, names::SUBDIVISION_LEVEL, level)
                .is_err()
        );
        assert!(
            Attributes::new()
                .with(Domain::Vertex, names::CREASE, wrong)
                .is_err()
        );
    }
}
