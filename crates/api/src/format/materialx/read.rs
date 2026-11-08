//! A document's materials as Studio's (the module docs say how).
use super::{SPACE, image_color};
use crate::color::ColorService;
use crate::format::files::Named;
use crate::format::{Input, Reading};
use crate::image_files::image_node;
use crate::network::{NetworkSeed, NewNode};
use crate::standard::ImportedMaterial;
use crate::{Diagnostic, Error};
use kinogaki_geometry::image::{ImageColor, ImageSource};
use kinogaki_render_api::material::NodeInput as Port;
use kinogaki_render_api::material::mtlx::{def, value};
use kinogaki_render_api::materialx::{Bound, Document, Included, Material, Type, library};
use kinogaki_value::Value;
use std::path::Path;

/// The materials of `text`, the document `input` holds, as Studio's: its
/// includes read through `named` (each document once; one out of reach
/// said by `named`, one that doesn't read said here), its graphs flattened,
/// each material converted (see the module docs). With what was left out
/// or approximated, as warnings; `named`'s own are the caller's to add.
///
/// # Errors
/// Text that isn't a MaterialX document, or a cancelled read.
pub fn read(
    text: &str,
    input: &Input<'_>,
    named: &Named,
    reading: &Reading<'_>,
) -> Result<(Vec<ImportedMaterial>, Vec<Diagnostic>), Error> {
    let mut document: Document =
        (text.parse()).map_err(|why: Error| Error::invalid(format!("{input}: {why}")))?;
    let mut unread = Vec::new();
    let identity = input.path().and_then(|p| p.canonicalize().ok());
    let identity = identity.map(|p| p.display().to_string());
    let mut said = document.include(identity.as_deref(), &mut |href, within| {
        include(named, href, within, &mut unread)
    });
    said.append(&mut unread);
    reading.check()?;
    let (materials, left) = document.materials();
    said.extend(left);
    let mut read = Read {
        named,
        colors: reading.colors,
        metres: reading.units.meters_per_unit(),
        said,
    };
    let mut converted = Vec::with_capacity(materials.len());
    for m in &materials {
        reading.check()?;
        converted.push(read.material(m));
    }
    let warnings = read.said.into_iter().map(Diagnostic::warning).collect();
    Ok((converted, warnings))
}

/// The document `href` names, relative to the document at `within` (none:
/// the one read): its canonical path, which is its identity, and its text;
/// none when it is out of reach (`named`'s warnings say so) or doesn't
/// read (said in `unread`).
fn include(
    named: &Named,
    href: &str,
    within: Option<&str>,
    unread: &mut Vec<String>,
) -> Option<Included> {
    let from = within.and_then(|w| Path::new(w).parent());
    let path = named.file(href, from)?;
    let text = (named.read(&path).map_err(|e| e.to_string()))
        .and_then(|bytes| String::from_utf8(bytes).map_err(|_| "not UTF-8 text".into()));
    match text {
        Ok(text) => Some(Included {
            identity: path.display().to_string(),
            text,
        }),
        Err(why) => {
            unread.push(format!("{href}: {why}; what it holds is left out"));
            None
        }
    }
}

/// What a read holds while it reads.
struct Read<'a> {
    /// The files the document names, which its images are read through.
    named: &'a Named,
    colors: &'a dyn ColorService,
    /// The project's metres a unit, which lengths a document says the unit
    /// of are converted into.
    metres: f64,
    /// What was left out or approximated, in words.
    said: Vec<String>,
}

impl Read<'_> {
    /// `m` as a Material of Studio's.
    fn material(&mut self, m: &Material) -> ImportedMaterial {
        // A material node holding a surface alone shows the surface.
        let held = &m.nodes[m.material];
        let surface = match &held.inputs[..] {
            [(input, Bound::Output { node, .. })]
                if held.def.node == "surfacematerial" && input == "surfaceshader" =>
            {
                Some(*node)
            }
            _ => None,
        };
        let shown = surface.unwrap_or(m.material);
        let mut seed = NetworkSeed::default();
        let mut made = vec![None; m.nodes.len()];
        for (at, node) in m.nodes.iter().enumerate() {
            if surface.is_some() && at == m.material {
                continue;
            }
            let Some(new) = NewNode::materialx(&node.def.name) else {
                self.said.push(format!(
                    "{}: no node {} here, left out",
                    m.name, node.def.name
                ));
                continue;
            };
            let new = new.named(&node.name);
            made[at] = Some(seed.node(if at == shown { new.displayed() } else { new }));
        }
        let mut images: Vec<(ImageSource, usize)> = Vec::new();
        for (at, node) in m.nodes.iter().enumerate() {
            let Some(to) = made[at] else { continue };
            let ports = def(&node.def.name);
            for (input, bound) in &node.inputs {
                match bound {
                    Bound::Output { node, output } => {
                        if let Some(from) = made[*node] {
                            seed.wire(from, output, to, input);
                        }
                    }
                    Bound::Value {
                        value: v,
                        colorspace,
                    } => {
                        let ty = &node
                            .def
                            .input(input)
                            .expect("a bound input is its definition's")
                            .ty;
                        if *ty == Type::Filename {
                            let kinogaki_render_api::materialx::Value::Text(file) = v else {
                                continue;
                            };
                            if file.is_empty() {
                                continue;
                            }
                            let color = ports.as_ref().is_none_or(|d| d.gives_color());
                            let read = image_color(colorspace.as_deref(), color);
                            let Some(source) = self.source(file, read) else {
                                continue;
                            };
                            let image = match images.iter().find(|(s, _)| *s == source) {
                                Some((_, image)) => *image,
                                None => {
                                    let stem = Path::new(file).file_stem();
                                    let name = stem
                                        .map_or_else(|| "Image".into(), |s| s.to_string_lossy());
                                    let image = seed.node(image_node::reading(&source).named(name));
                                    images.push((source, image));
                                    image
                                }
                            };
                            seed.wire(image, image_node::IMAGE, to, input);
                            continue;
                        }
                        let port = ports
                            .as_ref()
                            .and_then(|d| d.find_input(input))
                            .and_then(Port::value);
                        let held = port.and_then(|p| value(&p.kind, v));
                        let Some(mut held) = held else {
                            let why = match port.is_some() {
                                true => "a value its input doesn't hold",
                                false => "a value where it takes a wire alone",
                            };
                            self.said.push(format!(
                                "{}: {}'s {input} holds {why}, left out",
                                m.name, node.name
                            ));
                            continue;
                        };
                        single(&mut held);
                        if ty.is_color() {
                            self.working(&mut held, colorspace.as_deref().unwrap_or(SPACE));
                        }
                        seed.nodes[to].parameters.insert(input.clone(), held);
                    }
                }
            }
        }
        self.lengths(m, &made, &mut seed);
        ImportedMaterial {
            key: m.name.clone(),
            name: m.label.clone().unwrap_or_else(|| m.name.clone()),
            inside: seed,
            standard: None,
            double_sided: m.double_sided,
        }
    }

    /// `seed`'s lengths (`made` the node each of `m`'s became) from the
    /// distance unit the document says each is in into the project's;
    /// those it says none of stay as read.
    fn lengths(&mut self, m: &Material, made: &[Option<usize>], seed: &mut NetworkSeed) {
        let mut units = vec![None; seed.nodes.len()];
        for (node, at) in m.nodes.iter().zip(made) {
            if let Some(at) = at {
                units[*at] = Some(&node.units);
            }
        }
        let unconverted = seed.scale_lengths_by(|at, input| {
            let unit = units[at].and_then(|units| units.get(input));
            match unit.and_then(|unit| library().unit(unit)) {
                Some(("distance", metres)) => metres / self.metres,
                _ => 1.0,
            }
        });
        if unconverted > 0 {
            self.said.push(format!(
                "{}: {unconverted} lengths driven by textures keep the document's unit",
                m.name
            ));
        }
    }

    /// What file `file` (relative to the document's folder) read as `color`
    /// reads; none for a file out of reach, which the read's warnings list.
    fn source(&mut self, file: &str, color: ImageColor) -> Option<ImageSource> {
        if file.contains("<UDIM>") || file.contains("<UVTILE>") {
            self.said
                .push(format!("{file}: a tile set, read as the one file it names"));
        }
        let read = self.named.image(file, None)?;
        Some(ImageSource::new(read, color))
    }

    /// `held`, a color in `space`, in the working space.
    fn working(&mut self, held: &mut Value, space: &str) {
        let rgb = match held {
            Value::Color(c) => &mut c[..],
            Value::Numbers(n) if n.len() >= 3 => &mut n[..3],
            _ => return,
        };
        let mut three = [[rgb[0], rgb[1], rgb[2]]];
        match self.colors.to_working(space, &mut three) {
            Ok(()) => rgb.copy_from_slice(&three[0]),
            Err(e) => {
                let said = format!("colors in {space} kept as they are: {e}");
                if !self.said.contains(&said) {
                    self.said.push(said);
                }
            }
        }
    }
}

/// `v`'s numbers at MaterialX's precision, 32 bits: what a document holds.
fn single(v: &mut Value) {
    let round = |x: &mut f64| *x = f64::from(*x as f32);
    match v {
        Value::Number(x) => round(x),
        Value::Color(c) => c.iter_mut().for_each(round),
        Value::Numbers(n) => n.iter_mut().for_each(round),
        _ => {}
    }
}
