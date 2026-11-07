//! What Flat paints a triangle with ([`Paint`]): its material's OpenPBR
//! Surface's base color, a constant or an image (`ND_image_color3`) read at
//! the hit's texture coordinates, nearest texel, tiled. The MaterialX nodes it
//! implements are those ([`NODES`], with the Surface Material showing the
//! surface); a host adopts every other material into them, or cuts it to
//! the default, white.
use kinogaki_render_api::geometry::image::Image;
use kinogaki_render_api::material::standard;
use kinogaki_render_api::value::Value;
use kinogaki_render_api::{Prim, PrimId};
use std::collections::BTreeMap;

/// The MaterialX nodes Flat implements, by node or signature.
pub(crate) const NODES: [&str; 3] = [standard::OPEN_PBR, "surfacematerial", "ND_image_color3"];

/// A material as Flat paints it: a color, times an image where it has one.
pub(crate) struct Paint {
    color: [f32; 3],
    image: Option<Image>,
}

impl Paint {
    /// The paint of Material prim `material` among `prims`: white where it
    /// names none, or its network shows no OpenPBR Surface.
    pub fn of(prims: &BTreeMap<PrimId, Prim>, material: Option<PrimId>) -> Paint {
        let white = Paint {
            color: [1.0; 3],
            image: None,
        };
        let Some(Prim::Material(material)) = material.and_then(|m| prims.get(&m)) else {
            return white;
        };
        let network = &material.network;
        let Some(at) = network.surface() else {
            return white;
        };
        let surface = &network.nodes()[at as usize];
        if surface
            .def()
            .mtlx()
            .is_none_or(|d| d.node != standard::OPEN_PBR)
        {
            return white;
        }
        let wired = (network.links().iter()).find(|l| l.to == at && &*l.input == "base_color");
        let image = wired.and_then(|l| {
            let texture = network.nodes()[l.from as usize].textures().get("file")?;
            match prims.get(&texture.prim()) {
                Some(Prim::Texture(image)) => Some(image.clone()),
                _ => None,
            }
        });
        let held = surface.values().get("base_color").and_then(Value::color);
        Paint {
            color: match (&image, wired) {
                (Some(_), _) => [1.0; 3],
                (None, Some(_)) => [0.8; 3],
                (None, None) => held.map_or([0.8; 3], |c| c.map(|v| v as f32)),
            },
            image,
        }
    }

    /// Its color at texture coordinates `uv` (v up from the image's bottom
    /// row).
    pub fn at(&self, [u, v]: [f32; 2]) -> [f32; 3] {
        let Some(image) = &self.image else {
            return self.color;
        };
        let window = image.data();
        let [w, h] = window.size().map(|s| s.max(1) as f32);
        let x = ((u - u.floor()) * w).min(w - 1.0) as i32;
        let y = (h - 1.0 - ((v - v.floor()) * h).min(h - 1.0)) as i32;
        let [min_x, min_y] = window.min();
        let texel = image.read_pixel(0, min_x + x, min_y + y);
        [0, 1, 2].map(|k| self.color[k] * texel[k])
    }
}
