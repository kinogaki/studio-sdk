//! A standard light as a typed value ([`Light`]): its shape, one UsdLux
//! kind's ([`LightShape`]), LightAPI's emission and multipliers, ShapingAPI's
//! shaping ([`Shaping`]), ShadowAPI's ([`Shadow`]) and whether the camera
//! sees it; decoded from a prim of its type and encoded as one. What a type
//! has no row for (a portal's emission) decodes as [`Light::default`]'s.
use super::schema::{
    cylinder_light, disk_light, distant_light, dome_light, geometry_light, portal_light,
    rect_light, sphere_light,
};
use super::{
    CYLINDER_LIGHT, DISK_LIGHT, DISTANT_LIGHT, DOME_LIGHT, Emission, GEOMETRY_LIGHT, PORTAL_LIGHT,
    RECT_LIGHT, SPHERE_LIGHT, choice,
};
use crate::declared::{Declared, PrimType};
use kinogaki_geometry::{Sampled, Xform};
use kinogaki_value::{ImageRef, ObjectRef, Value};

/// How a dome's image is laid out (UsdLux's `inputs:texture:format`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum DomeFormat {
    /// As the file says; a file that says nothing is lat-long.
    #[default]
    Automatic,
    /// Longitude across, latitude down, as OpenEXR's lat-long maps.
    LatLong,
    /// A mirrored ball seen from afar.
    MirroredBall,
    /// An angular (light probe) map.
    Angular,
    /// A cube map's faces laid out as a vertical cross.
    CubeMapVerticalCross,
}

impl DomeFormat {
    /// Every format, by its USD token.
    pub const ALL: [(DomeFormat, &'static str); 5] = [
        (DomeFormat::Automatic, "automatic"),
        (DomeFormat::LatLong, "latlong"),
        (DomeFormat::MirroredBall, "mirroredBall"),
        (DomeFormat::Angular, "angular"),
        (DomeFormat::CubeMapVerticalCross, "cubeMapVerticalCross"),
    ];

    /// Its key, USD's token (`latlong`).
    pub fn key(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(f, _)| *f == self)
            .map_or("", |f| f.1)
    }

    /// The format of key `key`.
    pub fn of_key(key: &str) -> Option<DomeFormat> {
        Self::ALL.iter().find(|(_, k)| *k == key).map(|f| f.0)
    }
}

/// Where a dome's top points (DomeLight_1's `poleAxis`; a DomeLight's is
/// its own +Y).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pole {
    /// Along the scene's up axis.
    #[default]
    Scene,
    /// Along its own +Y.
    Y,
    /// Along its own +Z: its image turned a quarter about X, latitude 0,
    /// longitude 0 down its -Y.
    Z,
}

impl Pole {
    /// Its key, USD's token (`scene`, `Y`, `Z`).
    pub fn key(self) -> &'static str {
        match self {
            Pole::Scene => "scene",
            Pole::Y => "Y",
            Pole::Z => "Z",
        }
    }

    /// The pole of key `key`.
    pub fn of_key(key: &str) -> Option<Pole> {
        [Pole::Scene, Pole::Y, Pole::Z]
            .into_iter()
            .find(|p| p.key() == key)
    }
}

/// What a geometry light's mesh shows and what its light takes from the
/// mesh's material (UsdLux's `light:materialSyncMode`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MaterialSync {
    /// Rays see the material; its light is the light's emission times the
    /// material's glow, where the material glows (MeshLightAPI's fallback).
    #[default]
    GlowTintsLight,
    /// Rays see the material; its light is the light's emission alone.
    Independent,
    /// The mesh is the light alone: rays see its emission, and it scatters
    /// nothing.
    NoMaterialResponse,
}

impl MaterialSync {
    /// Every mode, by its USD token.
    pub const ALL: [(MaterialSync, &'static str); 3] = [
        (MaterialSync::GlowTintsLight, "materialGlowTintsLight"),
        (MaterialSync::Independent, "independent"),
        (MaterialSync::NoMaterialResponse, "noMaterialResponse"),
    ];

    /// Its key, USD's token (`independent`).
    pub fn key(self) -> &'static str {
        Self::ALL
            .iter()
            .find(|(m, _)| *m == self)
            .map_or("", |m| m.1)
    }

    /// The mode of key `key`.
    pub fn of_key(key: &str) -> Option<MaterialSync> {
        Self::ALL.iter().find(|(_, k)| *k == key).map(|m| m.0)
    }
}

/// A standard light's shape: one UsdLux kind.
#[derive(Clone, Debug, PartialEq)]
pub enum LightShape {
    /// A RectLight: `width` × `height` in its XY plane, emitting down -Z.
    Rect {
        /// Along X.
        width: f32,
        /// Along Y.
        height: f32,
        /// Its color map (none: its emission evenly), its first texel at
        /// +X +Y, as UsdLux lays it; an engine reads the Texture prim the
        /// host decoded it to from its prim's
        /// [`textures`](Declared::textures) (`texture`).
        texture: Option<ImageRef>,
    },
    /// A DiskLight: a disk in its XY plane, emitting down -Z.
    Disk {
        /// Its radius.
        radius: f32,
    },
    /// A SphereLight at its origin.
    Sphere {
        /// Its radius, above 0.
        radius: f32,
        /// `treatAsPoint`: a point of the sphere's power (its radiant
        /// intensity the radiance × πr², normalized the radiance / 4).
        point: bool,
    },
    /// A CylinderLight: a tube along its X axis, emitting from its side.
    Cylinder {
        /// Along X.
        length: f32,
        /// Its radius, above 0.
        radius: f32,
        /// `treatAsLine`: a line of the tube's power (its radiant intensity
        /// per length the radiance × 2r × the sine of the angle to its axis;
        /// normalized, over the tube's area).
        line: bool,
    },
    /// A DistantLight.
    Distant {
        /// Its size across, in degrees (0: a delta direction).
        angle: f32,
    },
    /// A DomeLight.
    Dome {
        /// Its image (none: its color, evenly): a file or the host's Image
        /// node; an engine reads the Texture prim the host decoded it to
        /// from its prim's [`textures`](Declared::textures) (`texture`).
        image: Option<ImageRef>,
        /// How its image is laid out.
        format: DomeFormat,
        /// Where its top points.
        pole: Pole,
        /// An imageless dome's hemisphere gradient: below the horizon it
        /// shines this color, linear ACEScg, times its intensity and
        /// exposure; above it, its emission. None: one color all round.
        ground: Option<[f32; 3]>,
    },
    /// A PortalLight: a `width` × `height` opening in its XY plane through
    /// which a dome lights, toward its -Z. A dome a portal serves lights
    /// only through its portals; a portal emits nothing of its own.
    Portal {
        /// Along X.
        width: f32,
        /// Along Y.
        height: f32,
        /// The dome light it serves (none: none); an engine reads its prim
        /// from the portal's [`members`](Declared::members) (`dome`).
        dome: Option<ObjectRef>,
    },
    /// A mesh that emits (MeshLightAPI; UsdLux's deprecated GeometryLight):
    /// its triangles shine the light's emission from both sides (normalized:
    /// over their area). Where the light is placed is unused.
    Geometry {
        /// The polygons object that emits (none: nothing does); an engine
        /// reads its prims from the light's [`members`](Declared::members)
        /// (`geometry`).
        geometry: Option<ObjectRef>,
        /// What its mesh shows and what its light takes from the mesh's
        /// material.
        sync: MaterialSync,
    },
}

/// ShapingAPI: how a light's emission narrows, each factor read from the
/// direction light leaves in against the light's -Z, on any shape.
#[derive(Clone, Debug, PartialEq)]
pub struct Shaping {
    /// The cone's half-angle, degrees (0 to 180; 90, the fallback, a
    /// hemisphere): no light leaves farther off the axis.
    pub cone_angle: f32,
    /// The share of the cone (0 to 1) its light fades over (smoothstep) to
    /// its edge; 0 a hard edge.
    pub cone_softness: f32,
    /// `|cos|^focus` pulls the light toward the axis (0: none).
    pub focus: f32,
    /// The color its light fades to off the axis as it focuses (black: it
    /// fades out), linear.
    pub focus_tint: [f32; 3],
    /// An IES profile: its candela scale the light by direction; an engine
    /// reads the Texture prim the host decoded it to (`iesFile`,
    /// [`ies`](super::ies)'s layout).
    pub ies: Option<ImageRef>,
    /// Stretches the profile's vertical angles (0: as it is; above 0 about
    /// its bottom, below 0 about its top).
    pub ies_angle_scale: f32,
    /// The profile divided by its power, so it shapes and doesn't brighten.
    pub ies_normalize: bool,
}

impl Default for Shaping {
    /// UsdLux's fallbacks: a 90° cone, hard, no focus, no profile.
    fn default() -> Shaping {
        Shaping {
            cone_angle: 90.0,
            cone_softness: 0.0,
            focus: 0.0,
            focus_tint: [0.0; 3],
            ies: None,
            ies_angle_scale: 0.0,
            ies_normalize: false,
        }
    }
}

/// ShadowAPI: non-physical controls of the shadows a light's light samples
/// see.
#[derive(Clone, Debug, PartialEq)]
pub struct Shadow {
    /// Whether anything shadows its light.
    pub enable: bool,
    /// What its shadowed light is multiplied by, linear (black: none
    /// passes).
    pub color: [f32; 3],
    /// How far from the shaded point an occluder shadows (−1: any
    /// distance).
    pub distance: f32,
    /// Within that distance, how far before it the shadow fades out (−1:
    /// none).
    pub falloff: f32,
    /// The fade's gamma.
    pub falloff_gamma: f32,
}

impl Default for Shadow {
    /// UsdLux's fallbacks: black shadows at any distance.
    fn default() -> Shadow {
        Shadow {
            enable: true,
            color: [0.0; 3],
            distance: -1.0,
            falloff: -1.0,
            falloff_gamma: 1.0,
        }
    }
}

impl Shadow {
    /// Whether it changes nothing physical: every shadow black, at any
    /// distance.
    pub fn physical(&self) -> bool {
        self.enable && self.color == [0.0; 3] && self.distance < 0.0
    }
}

/// A standard light, decoded.
#[derive(Clone, Debug, PartialEq)]
pub struct Light {
    /// Its shape.
    pub shape: LightShape,
    /// Where the spectrum comes from.
    pub emission: Emission,
    /// Its color, linear (ACEScg in a prim an engine receives,
    /// [`crate::RENDER_SPACE`]): the spectrum's in [`Emission::Color`].
    pub color: [f32; 3],
    /// `inputs:intensity`.
    pub intensity: f32,
    /// `inputs:exposure`: the intensity is scaled by 2 to this power.
    pub exposure: f32,
    /// `inputs:normalize`: its power does not change with its size (a dome
    /// has none).
    pub normalize: bool,
    /// The blackbody's kelvin in [`Emission::Temperature`].
    pub color_temperature: f32,
    /// `inputs:diffuse`: what its effect on diffuse lobes is multiplied by.
    pub diffuse: f32,
    /// `inputs:specular`: what its effect on the other lobes is multiplied
    /// by.
    pub specular: f32,
    /// Its shaping (ShapingAPI applied), on a rect, disk, sphere or
    /// cylinder.
    pub shaping: Option<Shaping>,
    /// Its shadows.
    pub shadow: Shadow,
    /// Whether the camera sees the light itself (it lights and reflects
    /// either way).
    pub camera: bool,
    /// Light to world across the shutter.
    pub xform: Sampled<Xform>,
}

impl Default for Light {
    /// A 1 × 1 rectangle of intensity 1 (UsdLux's fallbacks), unseen.
    fn default() -> Light {
        Light {
            shape: LightShape::Rect {
                width: 1.0,
                height: 1.0,
                texture: None,
            },
            emission: Emission::Color,
            color: [1.0; 3],
            intensity: 1.0,
            exposure: 0.0,
            normalize: false,
            color_temperature: 6500.0,
            diffuse: 1.0,
            specular: 1.0,
            shaping: None,
            shadow: Shadow::default(),
            camera: false,
            xform: Sampled::still(Xform::IDENTITY),
        }
    }
}

/// The image value `name` of `prim`.
fn image(prim: &Declared, name: &str) -> Option<ImageRef> {
    prim.value(name).and_then(Value::image).cloned()
}

/// The object value `name` of `prim`.
fn object(prim: &Declared, name: &str) -> Option<ObjectRef> {
    prim.value(name).and_then(Value::object)
}

impl Light {
    /// `prim` as a standard light (none when it is of another type), what
    /// its type has no row for as [`Light::default`]'s.
    pub fn decode(prim: &Declared) -> Option<Light> {
        let shape = match prim.type_id {
            RECT_LIGHT => LightShape::Rect {
                width: prim.float("width"),
                height: prim.float("height"),
                texture: image(prim, "texture"),
            },
            DISK_LIGHT => LightShape::Disk {
                radius: prim.float("radius"),
            },
            SPHERE_LIGHT => LightShape::Sphere {
                radius: prim.float("radius"),
                point: prim.bool("treatAsPoint"),
            },
            CYLINDER_LIGHT => LightShape::Cylinder {
                length: prim.float("length"),
                radius: prim.float("radius"),
                line: prim.bool("treatAsLine"),
            },
            DISTANT_LIGHT => LightShape::Distant {
                angle: prim.float("angle"),
            },
            DOME_LIGHT => LightShape::Dome {
                image: image(prim, "texture"),
                format: DomeFormat::of_key(prim.choice("textureFormat")).unwrap_or_default(),
                pole: Pole::of_key(prim.choice("poleAxis")).unwrap_or_default(),
                ground: prim.bool("ground").then(|| prim.color("groundColor")),
            },
            PORTAL_LIGHT => LightShape::Portal {
                width: prim.float("width"),
                height: prim.float("height"),
                dome: object(prim, "dome"),
            },
            GEOMETRY_LIGHT => LightShape::Geometry {
                geometry: object(prim, "geometry"),
                sync: MaterialSync::of_key(prim.choice("materialSyncMode")).unwrap_or_default(),
            },
            _ => return None,
        };
        let held = Light::default();
        let has = |name: &str| prim.value(name).is_some();
        let float = |name: &str, fallback: f32| match has(name) {
            true => prim.float(name),
            false => fallback,
        };
        let flag = |name: &str, fallback: bool| match has(name) {
            true => prim.bool(name),
            false => fallback,
        };
        let color = |name: &str, fallback: [f32; 3]| match has(name) {
            true => prim.color(name),
            false => fallback,
        };
        let shaping = prim.bool("shaping").then(|| Shaping {
            cone_angle: prim.float("coneAngle"),
            cone_softness: prim.float("coneSoftness"),
            focus: prim.float("focus"),
            focus_tint: prim.color("focusTint"),
            ies: image(prim, "iesFile"),
            ies_angle_scale: prim.float("iesAngleScale"),
            ies_normalize: prim.bool("iesNormalize"),
        });
        let shadow = &held.shadow;
        Some(Light {
            shape,
            emission: Emission::of_key(prim.choice("emission")).unwrap_or_default(),
            color: color("color", held.color),
            intensity: float("intensity", held.intensity),
            exposure: float("exposure", held.exposure),
            normalize: flag("normalize", held.normalize),
            color_temperature: float("colorTemperature", held.color_temperature),
            diffuse: float("diffuse", held.diffuse),
            specular: float("specular", held.specular),
            shaping,
            shadow: Shadow {
                enable: flag("shadowEnable", shadow.enable),
                color: color("shadowColor", shadow.color),
                distance: float("shadowDistance", shadow.distance),
                falloff: float("shadowFalloff", shadow.falloff),
                falloff_gamma: float("shadowFalloffGamma", shadow.falloff_gamma),
            },
            camera: flag("cameraVisibility", held.camera),
            xform: prim.xform.clone(),
        })
    }

    /// Its radiance as an RGB engine takes it, linear ACEScg: the
    /// emission's color × intensity × 2^exposure (white for an illuminant
    /// [`ILLUMINANTS`](super::ILLUMINANTS) doesn't hold).
    pub fn radiance(&self) -> [f32; 3] {
        let color = self.color.map(f64::from);
        let kelvin = f64::from(self.color_temperature);
        let rgb = self.emission.color(color, kelvin).unwrap_or([1.0; 3]);
        let k = f64::from(self.intensity) * f64::from(self.exposure).exp2();
        rgb.map(|c| (c * k) as f32)
    }

    /// Whether its shape takes shaping: a rect, disk, sphere or cylinder.
    pub fn shapes(&self) -> bool {
        matches!(
            self.shape,
            LightShape::Rect { .. }
                | LightShape::Disk { .. }
                | LightShape::Sphere { .. }
                | LightShape::Cylinder { .. }
        )
    }

    /// Its type: the standard type its shape is.
    pub fn prim_type(&self) -> PrimType {
        match self.shape {
            LightShape::Rect { .. } => rect_light(),
            LightShape::Disk { .. } => disk_light(),
            LightShape::Sphere { .. } => sphere_light(),
            LightShape::Cylinder { .. } => cylinder_light(),
            LightShape::Distant { .. } => distant_light(),
            LightShape::Dome { .. } => dome_light(),
            LightShape::Portal { .. } => portal_light(),
            LightShape::Geometry { .. } => geometry_light(),
        }
    }

    /// It as a prim: its type's defaults, with its own values (shaping
    /// only where its type takes it).
    pub fn declared(&self) -> Declared {
        let ty = self.prim_type();
        let mut values = ty.schema().values();
        let mut set = |name: &str, v: Value| {
            if values.contains_key(name) {
                values.insert(name.to_owned(), v);
            }
        };
        let float = |v: f32| Value::Number(f64::from(v));
        let color = |c: [f32; 3]| Value::Color(c.map(f64::from));
        match &self.shape {
            LightShape::Rect {
                width,
                height,
                texture,
            } => {
                set("width", float(*width));
                set("height", float(*height));
                set("texture", Value::Image(texture.clone()));
            }
            LightShape::Disk { radius } => set("radius", float(*radius)),
            LightShape::Sphere { radius, point } => {
                set("radius", float(*radius));
                set("treatAsPoint", Value::Bool(*point));
            }
            LightShape::Cylinder {
                length,
                radius,
                line,
            } => {
                set("length", float(*length));
                set("radius", float(*radius));
                set("treatAsLine", Value::Bool(*line));
            }
            LightShape::Distant { angle } => set("angle", float(*angle)),
            LightShape::Dome {
                image,
                format,
                pole,
                ground,
            } => {
                set("texture", Value::Image(image.clone()));
                set("textureFormat", choice(format.key()));
                set("poleAxis", choice(pole.key()));
                set("ground", Value::Bool(ground.is_some()));
                if let Some(g) = ground {
                    set("groundColor", color(*g));
                }
            }
            LightShape::Portal {
                width,
                height,
                dome,
            } => {
                set("width", float(*width));
                set("height", float(*height));
                set("dome", Value::Object(*dome));
            }
            LightShape::Geometry { geometry, sync } => {
                set("geometry", Value::Object(*geometry));
                set("materialSyncMode", choice(sync.key()));
            }
        }
        set("emission", choice(self.emission.key()));
        set("color", color(self.color));
        set("intensity", float(self.intensity));
        set("exposure", float(self.exposure));
        set("normalize", Value::Bool(self.normalize));
        set("colorTemperature", float(self.color_temperature));
        set("diffuse", float(self.diffuse));
        set("specular", float(self.specular));
        set("shaping", Value::Bool(self.shaping.is_some()));
        if let Some(s) = &self.shaping {
            set("coneAngle", float(s.cone_angle));
            set("coneSoftness", float(s.cone_softness));
            set("focus", float(s.focus));
            set("focusTint", color(s.focus_tint));
            set("iesFile", Value::Image(s.ies.clone()));
            set("iesAngleScale", float(s.ies_angle_scale));
            set("iesNormalize", Value::Bool(s.ies_normalize));
        }
        let shadow = &self.shadow;
        set("shadowEnable", Value::Bool(shadow.enable));
        set("shadowColor", color(shadow.color));
        set("shadowDistance", float(shadow.distance));
        set("shadowFalloff", float(shadow.falloff));
        set("shadowFalloffGamma", float(shadow.falloff_gamma));
        set("cameraVisibility", Value::Bool(self.camera));
        Declared {
            xform: self.xform.clone(),
            ..Declared::new(ty.id, values)
        }
    }
}

/// A light a menu offers beside one of each type: a type with values off
/// its defaults.
#[derive(Clone, Debug, PartialEq)]
pub struct Preset {
    /// The last step of its creation's id (`spot_light`).
    pub name: &'static str,
    /// What a menu calls it.
    pub label: &'static str,
    /// Words other applications use for it.
    pub aliases: &'static [&'static str],
    /// The light it makes.
    pub light: Light,
}

/// The presets, in the order a menu lists them: the Spot Light, a 0.1
/// sphere in a 30° cone, softness 0.2, normalized, of intensity 100 (as
/// Houdini's light tool shapes one).
pub fn presets() -> Vec<Preset> {
    vec![Preset {
        name: "spot_light",
        label: "Spot Light",
        aliases: &["spotlight", "spot", "cone", "fresnel"],
        light: Light {
            shape: LightShape::Sphere {
                radius: 0.1,
                point: false,
            },
            intensity: 100.0,
            normalize: true,
            shaping: Some(Shaping {
                cone_angle: 30.0,
                cone_softness: 0.2,
                ..Shaping::default()
            }),
            ..Light::default()
        },
    }]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_light_keeps_its_motion_through_its_prim() {
        let moved = |x: f64| Xform::translation(kinogaki_geometry::Vec3::new(x, 0.0, 0.0));
        let motion = Sampled::new([-0.25, 0.25], [moved(0.0), moved(1.0)]).unwrap();
        let light = Light {
            xform: motion.clone(),
            ..Light::default()
        };
        let prim = light.declared();
        assert_eq!(prim.xform, motion);
        assert_eq!(Light::decode(&prim).unwrap(), light);
    }

    #[test]
    fn a_portal_and_a_geometry_light_keep_what_they_name_through_their_prims() {
        let portal = Light {
            shape: LightShape::Portal {
                width: 2.0,
                height: 0.5,
                dome: Some(ObjectRef(7)),
            },
            ..Light::default()
        };
        let prim = portal.declared();
        assert_eq!(prim.type_id, super::super::PORTAL_LIGHT);
        // A portal has its size and its dome alone.
        let mut names: Vec<&str> = prim.values.keys().map(String::as_str).collect();
        names.sort_unstable();
        assert_eq!(names, ["dome", "height", "width"]);
        assert_eq!(Light::decode(&prim), Some(portal));
        let geometry = Light {
            shape: LightShape::Geometry {
                geometry: Some(ObjectRef(3)),
                sync: MaterialSync::NoMaterialResponse,
            },
            intensity: 5.0,
            normalize: true,
            ..Light::default()
        };
        let prim = geometry.declared();
        assert!(prim.value("cameraVisibility").is_none() && prim.value("shadowEnable").is_none());
        assert_eq!(Light::decode(&prim), Some(geometry));
        // MeshLightAPI's fallback when a prim says nothing else.
        let fresh = Declared::new(
            super::super::GEOMETRY_LIGHT,
            geometry_light().schema().values(),
        );
        assert!(matches!(
            Light::decode(&fresh).unwrap().shape,
            LightShape::Geometry {
                geometry: None,
                sync: MaterialSync::GlowTintsLight
            }
        ));
    }

    #[test]
    fn keys_are_usds_tokens() {
        for (format, key) in DomeFormat::ALL {
            assert_eq!((format.key(), DomeFormat::of_key(key)), (key, Some(format)));
        }
        assert_eq!(Pole::of_key("Z"), Some(Pole::Z));
        assert_eq!(Pole::default().key(), "scene");
        for (sync, key) in MaterialSync::ALL {
            assert_eq!((sync.key(), MaterialSync::of_key(key)), (key, Some(sync)));
        }
    }
}
