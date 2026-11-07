//! A document's materials, each as a flat network ([`Material`]): what a
//! file format reads of a MaterialX document ([`Document::materials`]) and
//! writes one of ([`Document::of_materials`]).
//!
//! **Reading.** Each material node at the document's top level
//! (`surfacematerial`, `volumematerial`) with every node it reads:
//! - node graphs flattened: a wire into a graph's output reads the node
//!   behind it, a graph's interface input what is bound to it, and every
//!   node of a graph a material reads from is the material's, read or not;
//! - the document's own node definitions implemented by graphs flattened
//!   the same way (the library's definitions stay nodes);
//! - a value's color space (its `colorspace`, else its nearest enclosing
//!   element's) kept with each color and file name, and file names prefixed
//!   by their `fileprefix`;
//! - each input's unit (its `unit`, else its nearest enclosing element's)
//!   kept by the node ([`MaterialNode::units`]);
//! - its name the material node's, its label its `uiname`.
//!
//! What a flat network can't hold (looks and their assignments, a node no
//! definition resolves, a value no input of its definition takes) is left
//! out and said.
//!
//! **Writing.** Each material as one node graph holding its nodes beside
//! its material node, which reads the graph's outputs: names made valid and
//! unique (a material's own as its `uiname` when that changes it), each value
//! of a color or a file name with its color space where it isn't the
//! document's, one of no color space said so (`colorspace=""`), each
//! input of a unit with it and its unit type (`unit="millimeter"
//! unittype="distance"`), and a node's `nodedef` where its category, type
//! and inputs wouldn't resolve to it.
//!
//! [`Document::materials`]: crate::Document::materials
//! [`Document::of_materials`]: crate::Document::of_materials
mod read;
mod write;

use crate::library::NodeDef;
use crate::types::Value;
use std::collections::BTreeMap;
use std::sync::Arc;

/// A material: its material node and the nodes it holds (see the module
/// docs).
#[derive(Clone, Debug, PartialEq)]
pub struct Material {
    /// Its name: its material node's.
    pub name: String,
    /// What a UI calls it (`uiname`), when that is said.
    pub label: Option<String>,
    /// Its nodes, in the document's order.
    pub nodes: Vec<MaterialNode>,
    /// Its material node's index in `nodes`.
    pub material: usize,
    /// Whether both sides of a surface show it: Kinogaki's
    /// `kinogaki:doublesided` on the material node, MaterialX having no
    /// sides; yes where nothing says.
    pub double_sided: bool,
}

/// One node of a [`Material`].
#[derive(Clone, Debug, PartialEq)]
pub struct MaterialNode {
    /// Its name, unique in its material.
    pub name: String,
    /// Its definition.
    pub def: Arc<NodeDef>,
    /// Its inputs the document binds, by name, in order; the others hold
    /// their definition's defaults.
    pub inputs: Vec<(String, Bound)>,
    /// The unit each input's value is in where the document says (`unit`
    /// on the input, else on its node, its graph or the document), by
    /// input name: a distance's (`millimeter`) or an angle's.
    pub units: BTreeMap<String, String>,
}

/// What an input of a [`MaterialNode`] is bound to.
#[derive(Clone, Debug, PartialEq)]
pub enum Bound {
    /// A value of the input's type. A color's or a file name's color space
    /// is what the document says it is in (`lin_rec709`, `srgb_texture`);
    /// none where nothing is said.
    Value {
        /// The value; a file name with its `fileprefix`.
        value: Value,
        /// Its color space, for a color or a file name.
        colorspace: Option<String>,
    },
    /// Output `output` of the material's node `node` (by index).
    Output {
        /// The node, by its index in [`Material::nodes`].
        node: usize,
        /// Its output, by name.
        output: String,
    },
}

/// `name` as a valid MaterialX name: letters, digits and `_`, a digit not
/// first.
pub(crate) fn valid_name(name: &str) -> String {
    let mut valid: String = (name.chars())
        .map(|c| match c.is_ascii_alphanumeric() || c == '_' {
            true => c,
            false => '_',
        })
        .collect();
    if valid.is_empty() || valid.starts_with(|c: char| c.is_ascii_digit()) {
        valid.insert(0, '_');
    }
    valid
}

/// `name`, or `name2`, `name3`… : the first `taken` doesn't hold, taken.
pub(crate) fn unique(name: String, taken: &mut std::collections::HashSet<String>) -> String {
    let mut chosen = name.clone();
    let mut n = 1;
    while taken.contains(&chosen) {
        n += 1;
        chosen = format!("{name}{n}");
    }
    taken.insert(chosen.clone());
    chosen
}
