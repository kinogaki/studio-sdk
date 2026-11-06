//! Translations between shading models: the definitions of nodes named
//! `<from>_to_<to>` in group `translation` (MaterialX's
//! `standard_surface_to_open_pbr_surface`, Kinogaki's
//! `gltf_pbr_to_open_pbr_surface`), chained by a breadth-first search, as
//! MaterialX's `ShaderTranslator` uses one.
//!
//! A translation node takes the source model's inputs by their names; each of
//! its outputs `<input>_out` feeds the target model's `<input>`.
use crate::library::{Library, NodeDef};
use std::collections::{HashMap, VecDeque};
use std::sync::Arc;

impl Library {
    /// The shortest chain of translations from shading model `from` (a node
    /// name: `standard_surface`) to a model `to` accepts, each step's
    /// definition in order; empty when `from` is accepted itself, none when no
    /// chain reaches one.
    pub fn translation(&self, from: &str, to: impl Fn(&str) -> bool) -> Option<Vec<Arc<NodeDef>>> {
        if to(from) {
            return Some(Vec::new());
        }
        let steps: Vec<(&str, &str, &Arc<NodeDef>)> = (self.defs())
            .filter(|d| d.group == "translation")
            .filter_map(|d| {
                let (a, b) = d.node.split_once("_to_")?;
                Some((a, b, d))
            })
            .collect();
        // Each model reached, with the step that reached it.
        let mut reached: HashMap<&str, (&str, &Arc<NodeDef>)> = HashMap::new();
        let mut open = VecDeque::from([from]);
        while let Some(model) = open.pop_front() {
            for &(a, b, def) in steps.iter().filter(|(a, ..)| *a == model) {
                if b == from || reached.contains_key(b) {
                    continue;
                }
                reached.insert(b, (a, def));
                if to(b) {
                    let mut chain = vec![def.clone()];
                    let mut at = a;
                    while at != from {
                        let (before, step) = reached[at];
                        chain.push(step.clone());
                        at = before;
                    }
                    chain.reverse();
                    return Some(chain);
                }
                open.push_back(b);
            }
        }
        None
    }
}

/// The model a translation definition translates to (`open_pbr_surface` of
/// `standard_surface_to_open_pbr_surface`).
pub fn target(step: &NodeDef) -> Option<&str> {
    step.node.split_once("_to_").map(|(_, to)| to)
}

#[cfg(test)]
mod tests {
    use crate::library;

    #[test]
    fn models_translate_by_the_shortest_chain() {
        let library = library();
        let names = |chain: Vec<std::sync::Arc<crate::NodeDef>>| -> Vec<String> {
            chain.into_iter().map(|d| d.node.clone()).collect()
        };
        let open = |m: &str| m == "open_pbr_surface";
        assert_eq!(
            library.translation("open_pbr_surface", open),
            Some(Vec::new())
        );
        let direct = library.translation("standard_surface", open).unwrap();
        assert_eq!(names(direct), ["standard_surface_to_open_pbr_surface"]);
        // OpenPBR Surface to glTF PBR goes through Standard Surface.
        let gltf = library
            .translation("open_pbr_surface", |m| m == "gltf_pbr")
            .unwrap();
        assert_eq!(
            names(gltf),
            [
                "open_pbr_surface_to_standard_surface",
                "standard_surface_to_gltf_pbr"
            ]
        );
        let usd = library.translation("UsdPreviewSurface", open).unwrap();
        assert_eq!(super::target(&usd[0]), Some("open_pbr_surface"));
        assert_eq!(
            library.translation("open_pbr_surface", |m| m == "nothing"),
            None
        );
    }
}
