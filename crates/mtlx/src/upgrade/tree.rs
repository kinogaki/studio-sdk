//! A document as an arena of elements, for upgrading: stable ids, parents,
//! and the graph questions MaterialX's upgrade asks (a port's node, a node's
//! downstream ports, a free child name), answered as its own `Element`
//! classes answer them.
use crate::xml::{Child, Element};
use std::borrow::Cow;

/// An element's id in its [`Tree`].
pub(super) type Id = usize;

enum Kind {
    Element {
        category: String,
        attributes: Vec<(String, String)>,
    },
    Comment(String),
    Newline,
}

struct Item {
    kind: Kind,
    children: Vec<Id>,
    parent: Option<Id>,
}

/// The arena (see the module docs).
pub(super) struct Tree {
    items: Vec<Item>,
}

/// Categories that are not nodes; every other element is one, as MaterialX
/// makes a node of any category it has no class for.
const NOT_NODES: &[&str] = &[
    "materialx",
    "input",
    "output",
    "token",
    "parameter",
    "nodedef",
    "nodegraph",
    "implementation",
    "typedef",
    "member",
    "unitdef",
    "unit",
    "unittypedef",
    "geompropdef",
    "geominfo",
    "geomprop",
    "geomattr",
    "look",
    "lookgroup",
    "materialassign",
    "visibility",
    "collection",
    "property",
    "propertyassign",
    "propertyset",
    "propertysetassign",
    "variant",
    "variantset",
    "variantassign",
    "backdrop",
    "attributedef",
    "targetdef",
    "xi:include",
    "material",
    "shaderref",
    "bindinput",
    "bindparam",
    "bindtoken",
    "override",
    "materialinherit",
];

impl Tree {
    pub fn from(root: Element) -> Tree {
        let mut tree = Tree { items: Vec::new() };
        tree.adopt(root, None);
        tree
    }

    fn adopt(&mut self, e: Element, parent: Option<Id>) -> Id {
        let id = self.items.len();
        self.items.push(Item {
            kind: Kind::Element {
                category: e.category.into_owned(),
                attributes: (e.attributes.into_iter())
                    .map(|(k, v)| (k.into_owned(), v.into_owned()))
                    .collect(),
            },
            children: Vec::new(),
            parent,
        });
        for child in e.children {
            let c = match child {
                Child::Element(c) => self.adopt(c, Some(id)),
                Child::Comment(text) => self.leaf(Kind::Comment(text.into_owned()), id),
                Child::Newline => self.leaf(Kind::Newline, id),
            };
            self.items[id].children.push(c);
        }
        id
    }

    fn leaf(&mut self, kind: Kind, parent: Id) -> Id {
        self.items.push(Item {
            kind,
            children: Vec::new(),
            parent: Some(parent),
        });
        self.items.len() - 1
    }

    /// The tree as an element again, from the root (id 0).
    pub fn into_element(mut self) -> Element {
        self.take(0)
    }

    fn take(&mut self, id: Id) -> Element {
        let children = std::mem::take(&mut self.items[id].children);
        let kind = std::mem::replace(&mut self.items[id].kind, Kind::Newline);
        let Kind::Element {
            category,
            attributes,
        } = kind
        else {
            unreachable!("only elements hold children");
        };
        let children = (children.into_iter())
            .map(|c| match &self.items[c].kind {
                Kind::Element { .. } => Child::Element(self.take(c)),
                Kind::Comment(text) => Child::Comment(Cow::Owned(text.clone())),
                Kind::Newline => Child::Newline,
            })
            .collect();
        Element {
            category: Cow::Owned(category),
            attributes: (attributes.into_iter())
                .map(|(k, v)| (Cow::Owned(k), Cow::Owned(v)))
                .collect(),
            children,
        }
    }

    /// Every element under the root, the root first, depth first.
    pub fn walk(&self) -> Vec<Id> {
        let mut out = Vec::new();
        let mut open = vec![0];
        while let Some(id) = open.pop() {
            out.push(id);
            open.extend(self.elements(id).rev());
        }
        out
    }

    /// Every node, in the order [`walk`](Self::walk) visits them.
    pub fn nodes(&self) -> Vec<Id> {
        self.walk()
            .into_iter()
            .filter(|&n| self.is_node(n))
            .collect()
    }

    /// `id`'s child elements, in order.
    pub fn elements(&self, id: Id) -> impl DoubleEndedIterator<Item = Id> + '_ {
        (self.items[id].children.iter().copied())
            .filter(|&c| matches!(self.items[c].kind, Kind::Element { .. }))
    }

    /// `id`'s child elements of `category`.
    pub fn of(&self, id: Id, category: &str) -> Vec<Id> {
        self.elements(id)
            .filter(|&c| self.category(c) == category)
            .collect()
    }

    pub fn parent(&self, id: Id) -> Option<Id> {
        self.items[id].parent
    }

    pub fn category(&self, id: Id) -> &str {
        match &self.items[id].kind {
            Kind::Element { category, .. } => category,
            _ => "",
        }
    }

    pub fn set_category(&mut self, id: Id, category: &str) {
        if let Kind::Element { category: c, .. } = &mut self.items[id].kind {
            *c = category.to_owned();
        }
    }

    fn attributes(&self, id: Id) -> &[(String, String)] {
        match &self.items[id].kind {
            Kind::Element { attributes, .. } => attributes,
            _ => &[],
        }
    }

    pub fn attr(&self, id: Id, name: &str) -> Option<&str> {
        (self.attributes(id).iter()).find_map(|(k, v)| (k == name).then_some(v.as_str()))
    }

    pub fn get(&self, id: Id, name: &str) -> &str {
        self.attr(id, name).unwrap_or("")
    }

    pub fn set(&mut self, id: Id, name: &str, value: &str) {
        if let Kind::Element { attributes, .. } = &mut self.items[id].kind {
            match attributes.iter_mut().find(|(k, _)| k == name) {
                Some((_, v)) => *v = value.to_owned(),
                None => attributes.push((name.to_owned(), value.to_owned())),
            }
        }
    }

    pub fn unset(&mut self, id: Id, name: &str) {
        if let Kind::Element { attributes, .. } = &mut self.items[id].kind {
            attributes.retain(|(k, _)| k != name);
        }
    }

    pub fn name(&self, id: Id) -> &str {
        self.get(id, "name")
    }

    /// `parent`'s child element named `name`.
    pub fn child(&self, parent: Id, name: &str) -> Option<Id> {
        self.elements(parent).find(|&c| self.name(c) == name)
    }

    pub fn is_node(&self, id: Id) -> bool {
        matches!(self.items[id].kind, Kind::Element { .. })
            && self.parent(id).is_some()
            && !NOT_NODES.contains(&self.category(id))
    }

    /// A new element of `category` under `parent`, at `index` among its
    /// children (none: last), with `attributes`.
    pub fn add(
        &mut self,
        parent: Id,
        index: Option<usize>,
        category: &str,
        attributes: &[(&str, &str)],
    ) -> Id {
        let id = self.items.len();
        self.items.push(Item {
            kind: Kind::Element {
                category: category.to_owned(),
                attributes: (attributes.iter())
                    .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                    .collect(),
            },
            children: Vec::new(),
            parent: Some(parent),
        });
        let children = &mut self.items[parent].children;
        match index {
            Some(at) => children.insert(at.min(children.len()), id),
            None => children.push(id),
        }
        id
    }

    /// Node `node`'s input `name` (added of `ty` when missing).
    pub fn input(&mut self, node: Id, name: &str, ty: &str) -> Id {
        match self
            .of(node, "input")
            .into_iter()
            .find(|&i| self.name(i) == name)
        {
            Some(i) => i,
            None => self.add(node, None, "input", &[("name", name), ("type", ty)]),
        }
    }

    /// Removes `id` from its parent.
    pub fn remove(&mut self, id: Id) {
        if let Some(parent) = self.items[id].parent {
            self.items[parent].children.retain(|&c| c != id);
        }
    }

    /// `id`'s place among its parent's children (comments and blank lines
    /// counted, as MaterialX counts them).
    pub fn index(&self, id: Id) -> Option<usize> {
        let parent = self.parent(id)?;
        self.items[parent].children.iter().position(|&c| c == id)
    }

    /// The graph `port` belongs to: its node's parent, or its own parent for a
    /// graph's output.
    pub fn graph_of(&self, port: Id) -> Option<Id> {
        let parent = self.parent(port)?;
        match self.is_node(parent) {
            true => self.parent(parent),
            false => Some(parent),
        }
    }

    /// The node `port`'s `nodename` connects it to.
    pub fn connected(&self, port: Id) -> Option<Id> {
        let name = self.attr(port, "nodename")?;
        let graph = self.graph_of(port)?;
        self.child(graph, name).filter(|&n| self.is_node(n))
    }

    /// The ports (inputs and outputs) connected to `node`, in tree order.
    pub fn downstream(&self, node: Id) -> Vec<Id> {
        (self.walk().into_iter())
            .filter(|&p| matches!(self.category(p), "input" | "output"))
            .filter(|&p| self.connected(p) == Some(node))
            .collect()
    }

    /// `base` made a name no child of `parent` has, as MaterialX's
    /// `createValidChildName` makes one.
    pub fn free_name(&self, parent: Id, base: &str) -> String {
        let mut name: String = (base.chars())
            .map(|c| {
                if c.is_ascii_alphanumeric() || c == '_' || c == ':' {
                    c
                } else {
                    '_'
                }
            })
            .collect();
        while self.child(parent, &name).is_some() {
            name = increment(&name);
        }
        name
    }

    /// `id`'s attributes, copied.
    pub fn copy_of(&self, id: Id) -> Vec<(String, String)> {
        self.attributes(id).to_vec()
    }

    /// Sets every attribute of `from` on `to`.
    pub fn copy_attributes(&mut self, from: Id, to: Id) {
        for (key, value) in self.copy_of(from) {
            self.set(to, &key, &value);
        }
    }

    /// Copies `from`'s input `name` (its type, value and connection) to
    /// `to`'s input `as_name`, as MaterialX's `copyInputWithBindings` does.
    pub fn copy_input(&mut self, from: Id, name: &str, to: Id, as_name: &str) {
        let Some(source) = self
            .of(from, "input")
            .into_iter()
            .find(|&i| self.name(i) == name)
        else {
            return;
        };
        let ty = self.get(source, "type").to_owned();
        let target = self.input(to, as_name, &ty);
        for (key, value) in self.copy_of(source).iter().filter(|(k, _)| k != "name") {
            self.set(target, key, value);
        }
    }
}

/// MaterialX's `incrementName`: a trailing number counted up, else `2` added.
fn increment(name: &str) -> String {
    let digits = name.len() - name.trim_end_matches(|c: char| c.is_ascii_digit()).len();
    match digits {
        0 => format!("{name}2"),
        _ => {
            let (stem, number) = name.split_at(name.len() - digits);
            let next = number.parse::<u64>().map_or(2, |n| n + 1);
            format!("{stem}{next}")
        }
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn names_count_up_as_materialx_counts_them() {
        assert_eq!(super::increment("swizzle"), "swizzle2");
        assert_eq!(super::increment("swizzle9"), "swizzle10");
    }
}
