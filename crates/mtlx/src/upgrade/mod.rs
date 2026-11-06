//! Documents of MaterialX 1.36 to 1.38 upgraded to 1.39, step by step, as
//! MaterialX's `Document::upgradeVersion` upgrades them with no library
//! loaded: renamed nodes and inputs, `color2` as `vector2`, material
//! elements as material nodes, BSDF layering as `layer` nodes, parameters as
//! inputs, `channels` and swizzles as `extract`, `convert` and `combine`
//! nodes, thin-film layers as BSDF inputs, object-space normal maps as
//! graphs. Kinogaki's own data is always current: this serves other
//! applications' files.
mod tree;
mod v137;
mod v139;

use crate::document::VERSION;
use crate::xml::Element;
use tree::Tree;

/// `root` (a `<materialx>` element) at MaterialX 1.39.
pub(crate) fn upgrade(root: &mut Element) -> Result<(), String> {
    let version = root.get("version").to_owned();
    // A document naming no version is read as it is, as MaterialX reads one.
    if version.is_empty() {
        return Ok(());
    }
    let minor = match version.split('.').collect::<Vec<_>>()[..] {
        ["1", minor, ..] => minor.parse::<u32>().ok(),
        _ => None,
    };
    let Some(mut minor) = minor.filter(|m| (36..=39).contains(m)) else {
        return Err(format!("MaterialX {version} isn't read: 1.36 to 1.39 are"));
    };
    if minor == 39 {
        return Ok(());
    }
    let mut tree = Tree::from(std::mem::take(root));
    while minor < 39 {
        match minor {
            36 => v137::from_136(&mut tree),
            37 => v137::from_137(&mut tree),
            _ => v139::from_138(&mut tree),
        }
        minor += 1;
    }
    *root = tree.into_element();
    root.set("version", VERSION);
    Ok(())
}
