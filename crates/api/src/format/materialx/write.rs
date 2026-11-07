//! Materials as a MaterialX document (the module docs say how).
use super::{SPACE, colorspace};
use crate::color::ColorService;
use crate::format::Writing;
use crate::format::files::Beside;
use crate::image_files::{IMAGE_NODE, image_source};
use crate::network::NetworkSeed;
use crate::standard::ImportedMaterial;
use crate::{Diagnostic, Error};
use kinogaki_render_api::material::mtlx::{SIGNATURE, is_length, of_value};
use kinogaki_render_api::materialx::{Bound, Document, Material, MaterialNode, Type, library};
use kinogaki_value::Value;
use std::borrow::Cow;
use std::collections::BTreeMap;

/// `materials` as one document, its colors in [`SPACE`], the embedded
/// files its images hold written by `files` beside it: each material's
/// network as authored when it is MaterialX's, else its standard form,
/// else left out (see the module docs). With what was written with loss or
/// left out, as warnings.
///
/// # Errors
/// An Image node that can't be read as a source, an embedded file that
/// can't be written, or a cancelled write.
pub fn write(
    materials: &[&ImportedMaterial],
    files: &mut Beside,
    writing: &Writing<'_>,
) -> Result<(Document, Vec<Diagnostic>), Error> {
    // Lengths in the project's unit where MaterialX names it, else in
    // metres.
    let metres = writing.units.meters_per_unit();
    let unit = match library().distance_unit(metres) {
        Some(unit) => (unit, 1.0),
        None => ("meter", metres),
    };
    let mut write = Write {
        files,
        colors: writing.colors,
        unit,
        said: Vec::new(),
    };
    let mut written = Vec::with_capacity(materials.len());
    for m in materials {
        writing.check()?;
        written.extend(write.material(m)?);
    }
    let document = Document::of_materials(&written, SPACE);
    let warnings = write.said.into_iter().map(Diagnostic::warning).collect();
    Ok((document, warnings))
}

/// What a write holds while it writes.
struct Write<'a> {
    files: &'a mut Beside,
    colors: &'a dyn ColorService,
    /// The distance unit lengths are written in, and what takes them there
    /// from the project's.
    unit: (&'static str, f64),
    said: Vec<String>,
}

impl Write<'_> {
    /// `m` as a document's material: its network as authored when it is
    /// MaterialX's, else its standard form; none when it has neither.
    fn material(&mut self, m: &ImportedMaterial) -> Result<Option<Material>, Error> {
        let materialx = |seed: &NetworkSeed| {
            (seed.nodes.iter())
                .all(|n| n.node_type == IMAGE_NODE || n.parameters.contains_key(SIGNATURE))
        };
        let seed = match (&m.standard, materialx(&m.inside)) {
            (_, true) => &m.inside,
            (Some(standard), false) => {
                self.said
                    .push(format!("{}: written as its OpenPBR Surface form", m.name));
                standard
            }
            (None, false) => {
                self.said.push(format!(
                    "{}: nothing standard approximates it, left out",
                    m.name
                ));
                return Ok(None);
            }
        };
        let (unit, by) = self.unit;
        let mut seed = Cow::Borrowed(seed);
        if by != 1.0 && seed.to_mut().scale_lengths(by) > 0 {
            self.said.push(format!(
                "{}: lengths driven by textures keep the project's unit",
                m.name
            ));
        }
        let mut nodes = Vec::new();
        let mut at = vec![None; seed.nodes.len()];
        for (n, node) in seed.nodes.iter().enumerate() {
            let signature = node.parameters.get(SIGNATURE).and_then(Value::choice);
            let Some(def) = signature.and_then(|s| library().def(s)) else {
                continue;
            };
            let mut inputs = Vec::new();
            for port in &def.inputs {
                let Some(held) = node.parameters.get(&port.name) else {
                    continue;
                };
                let Some(mut v) = of_value(held) else {
                    continue;
                };
                if port.value.as_ref() == Some(&v) {
                    continue;
                }
                let colorspace = port.ty.is_color().then(|| {
                    self.document(&mut v);
                    SPACE.to_owned()
                });
                inputs.push((
                    port.name.clone(),
                    Bound::Value {
                        value: v,
                        colorspace,
                    },
                ));
            }
            let name = node.name.clone().unwrap_or_else(|| def.node.clone());
            at[n] = Some(nodes.len());
            nodes.push(MaterialNode {
                name,
                def: def.clone(),
                inputs,
                units: BTreeMap::new(),
            });
        }
        for wire in &seed.wires {
            let Some(to) = at.get(wire.to).copied().flatten() else {
                continue;
            };
            let bound = match at.get(wire.from).copied().flatten() {
                Some(from) => Bound::Output {
                    node: from,
                    output: wire.output.clone(),
                },
                None => {
                    let read = &seed.nodes[wire.from];
                    if read.node_type != IMAGE_NODE {
                        continue;
                    }
                    let Some(source) = image_source(&read.parameters)? else {
                        self.said.push(format!(
                            "{}: an Image node reading a result, left out",
                            m.name
                        ));
                        continue;
                    };
                    let file = self.files.name(source.file())?;
                    Bound::Value {
                        value: kinogaki_render_api::materialx::Value::Text(file),
                        colorspace: colorspace(source.color()).map(str::to_owned),
                    }
                }
            };
            nodes[to].inputs.push((wire.input.clone(), bound));
        }
        // Every length says its unit, held or driven.
        for node in &mut nodes {
            let lengths = (node.inputs.iter()).filter(|(input, _)| is_length(&node.def, input));
            node.units = lengths
                .map(|(input, _)| (input.clone(), unit.to_owned()))
                .collect();
        }
        // The shown node, read by a Surface Material where it is a surface.
        let shown = seed
            .nodes
            .iter()
            .position(|n| n.display)
            .and_then(|n| at[n]);
        let Some(shown) = shown else {
            self.said
                .push(format!("{}: it shows nothing, left out", m.name));
            return Ok(None);
        };
        let material = match nodes[shown].def.ty() == Type::of("material") {
            true => shown,
            false => {
                let def = library()
                    .def("ND_surfacematerial")
                    .expect("MaterialX's library defines it");
                let surface = Bound::Output {
                    node: shown,
                    output: nodes[shown].def.outputs[0].name.clone(),
                };
                nodes.push(MaterialNode {
                    name: m.name.clone(),
                    def: def.clone(),
                    inputs: vec![("surfaceshader".into(), surface)],
                    units: BTreeMap::new(),
                });
                nodes.len() - 1
            }
        };
        Ok(Some(Material {
            name: m.name.clone(),
            label: None,
            nodes,
            material,
            double_sided: m.double_sided,
        }))
    }

    /// `v`, a color in the working space, in the document's.
    fn document(&mut self, v: &mut kinogaki_render_api::materialx::Value) {
        let kinogaki_render_api::materialx::Value::Numbers(n) = v else {
            return;
        };
        if n.len() < 3 {
            return;
        }
        let mut three = [[n[0], n[1], n[2]]];
        match self.colors.working_to(SPACE, &mut three) {
            Ok(()) => n[..3].copy_from_slice(&three[0]),
            Err(e) => {
                let said = format!("colors written as they are: {e}");
                if !self.said.contains(&said) {
                    self.said.push(said);
                }
            }
        }
    }
}
