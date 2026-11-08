//! XYZ: an example file format. A file is text, one statement a line, `#`
//! starting a comment:
//!
//! ```text
//! material Clay 0.8 0.45 0.3 clay.png   # a material: its color, linear Rec.709,
//!                                       # and the picture beside the file its color reads
//! 0 0 0                                 # a point, in metres
//! 1 0 0
//! 1 1 0
//! face Clay 1 2 3                       # a polygon over points (from 1) showing a material
//! face - 3 2 1                          # one showing none
//! ```
//!
//! It reads as the standard values File ▸ Import adopts: a materials item
//! first (each material an OpenPBR Surface whose base color is the
//! material's color, or an image texture reading its picture through an
//! Image node), then one Geometry of the points and faces, each face's
//! `material` naming its material. Written, every item's points and
//! polygons go out where the item is, and every material its color and
//! picture. Its one option scales what it reads; Embed Textures keeps the
//! pictures in the project.
//!
//! A format converts at its boundary and nowhere else: lengths into the
//! project's units (`Reading::units`), colors from the file's linear Rec.709
//! into the project's working space (`Reading::colors`). The files a file
//! names are found through `format::files::Named`, which charges them to the
//! read's budget.
use kinogaki_render_api::MaterialNetwork;
use kinogaki_studio_api::color::{self, ColorService};
use kinogaki_studio_api::format::files::{Beside, Named, replace};
use kinogaki_studio_api::format::options;
use kinogaki_studio_api::geometry::image::{ImageColor, ImageFile, ImageSource};
use kinogaki_studio_api::geometry::{
    Attribute, Attributes, Column, Domain, Geometry, Primitives, Text, Units, Xform, names,
};
use kinogaki_studio_api::image_files::image_node;
use kinogaki_studio_api::network::{NetworkSeed, NewNode};
use kinogaki_studio_api::params::{Schema, Value};
use kinogaki_studio_api::standard::{self, ImportedMaterial, Materials};
use kinogaki_studio_api::{
    DataTypeId, Diagnostic, Error, FileFormat, FileItem, FormatId, FormatOptions, FormatWriter,
    Imported, Input, Package, PackageId, Purpose, Reading, Writing, export_package,
};
use std::fmt::Write as _;
use std::io::Write as _;
use std::path::Path;
use std::sync::Arc;

#[cfg(test)]
mod tests;

/// The scale option's name.
const SCALE: &str = "scale";

/// Millimetres in a metre, the file's unit.
const METRE: f64 = 1000.0;

/// What it reads and writes: materials, and the polygons showing them.
const TYPES: &[DataTypeId] = &[standard::materials().id(), standard::geometry().id()];

/// MaterialX's OpenPBR Surface, each material's shading model, and its
/// color input.
const SURFACE: &str = "ND_open_pbr_surface_surfaceshader";
const BASE_COLOR: &str = "base_color";

/// MaterialX's image texture of a color, reading a material's picture.
const PICTURE: &str = "ND_image_color3";

/// The format.
pub struct Points;

/// Three words as three finite numbers.
fn three(words: [&str; 3]) -> Option<[f64; 3]> {
    let [x, y, z] = words.map(|w| w.parse::<f64>().ok().filter(|v| v.is_finite()));
    Some([x?, y?, z?])
}

/// The project units a file's metres are worth one of.
fn per_metre(units: Units) -> f64 {
    METRE / units.millimeters_per_unit()
}

impl FileFormat for Points {
    fn id(&self) -> FormatId {
        FormatId::declared("example.points.xyz")
    }
    fn label(&self) -> &str {
        "XYZ Points"
    }
    fn extensions(&self) -> &[&str] {
        &["xyz"]
    }
    fn produces(&self) -> &[DataTypeId] {
        TYPES
    }
    fn options(&self) -> FormatOptions {
        FormatOptions::from([
            (SCALE.to_owned(), Value::Number(1.0)),
            (options::EMBED.to_owned(), Value::Bool(false)),
        ])
    }
    fn schema(&self, chosen: &FormatOptions, _purpose: Purpose) -> Schema {
        let scale = options::option(chosen, SCALE, Value::number).unwrap_or(1.0);
        let embed = options::option(chosen, options::EMBED, Value::bool).unwrap_or(false);
        options::embed(options::scale(Schema::default(), scale), embed)
    }
    fn read(
        &self,
        input: &Input<'_>,
        chosen: &FormatOptions,
        reading: &Reading<'_>,
    ) -> Result<Imported, Error> {
        let bytes = input.bytes()?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| Error::invalid(format_args!("{input}: not text")))?;
        let scale = options::option(chosen, SCALE, Value::number)?;
        let mut file = File::read(text, scale * per_metre(reading.units), reading)?;
        let named = Named::new(input, chosen, reading);
        let materials = file.materials(&named, reading.colors)?;
        let name = Path::new(input.name()).file_stem();
        let name = name.map_or_else(String::new, |s| s.to_string_lossy().into_owned());
        let geometry = standard::geometry().wrap(file.geometry()?);
        let mut items = Vec::new();
        if !materials.materials.is_empty() {
            items.push(FileItem::top(
                "Materials",
                standard::materials().wrap(materials),
            ));
        }
        items.push(FileItem::top(name, geometry));
        let mut warnings = named.warnings();
        warnings.append(&mut file.said);
        Ok(Imported { items, warnings })
    }
    fn writer(&self) -> Option<&dyn FormatWriter> {
        Some(self)
    }
}

/// A file as read: its points, faces and materials, in the file's order.
#[derive(Default)]
struct File {
    points: Vec<[f64; 3]>,
    /// Each face's first corner in `corners`, and one past the last.
    offsets: Vec<u32>,
    corners: Vec<u32>,
    /// Each face's material, by its index in `names` (`Text::NONE`: none).
    shows: Vec<u32>,
    /// The names faces show.
    names: Vec<Arc<str>>,
    /// Each material's name, color and picture, in the file's order.
    looks: Vec<(Arc<str>, [f64; 3], Option<String>)>,
    said: Vec<Diagnostic>,
}

impl File {
    /// `text` read, its points times `factor`, checking `reading`'s
    /// cancellation as it goes.
    fn read(text: &str, factor: f64, reading: &Reading<'_>) -> Result<File, Error> {
        let mut file = File {
            offsets: vec![0],
            ..File::default()
        };
        for (n, line) in text.lines().enumerate() {
            reading.check()?;
            let at = |why: &str| Error::invalid(format_args!("line {}: {why}", n + 1));
            let line = line.split('#').next().unwrap_or_default();
            let words: Vec<&str> = line.split_whitespace().collect();
            match words[..] {
                [] => {}
                ["material", name, r, g, b, ref picture @ ..] => {
                    if file.looks.iter().any(|(named, ..)| **named == *name) {
                        return Err(at(&format!("material {name} again")));
                    }
                    let color = three([r, g, b]).ok_or_else(|| at("not numbers"))?;
                    let picture = match picture {
                        [] => None,
                        [file] => Some(file.to_string()),
                        _ => return Err(at("a material names one picture")),
                    };
                    file.looks.push((Arc::from(name), color, picture));
                }
                ["face", material, ref points @ ..] => {
                    file.face(material, points).map_err(|why| at(&why))?;
                }
                [x, y, z] => {
                    let p = three([x, y, z]).ok_or_else(|| at("not numbers"))?;
                    file.points.push(p.map(|v| v * factor));
                }
                _ => return Err(at("not a point, a face or a material")),
            }
        }
        Ok(file)
    }

    /// A face over `points` (numbered from 1) showing `material` (`-`:
    /// none).
    fn face(&mut self, material: &str, points: &[&str]) -> Result<(), String> {
        if points.len() < 3 {
            return Err("a face has three points or more".into());
        }
        for point in points {
            let number: u32 = point
                .parse()
                .map_err(|_| format!("{point} isn't a point's number"))?;
            let index = (number.checked_sub(1)).filter(|&i| (i as usize) < self.points.len());
            self.corners
                .push(index.ok_or_else(|| format!("no point {number} before this face"))?);
        }
        self.offsets.push(self.corners.len() as u32);
        let shows = match material {
            "-" => Text::NONE,
            name => match self.names.iter().position(|n| **n == *name) {
                Some(at) => at as u32,
                None => {
                    self.names.push(Arc::from(name));
                    (self.names.len() - 1) as u32
                }
            },
        };
        self.shows.push(shows);
        Ok(())
    }

    /// Its materials as Import adopts them: each an OpenPBR Surface, its
    /// color converted from linear Rec.709 into the working space, its
    /// picture found through `named` and read by an Image node.
    fn materials(&mut self, named: &Named, colors: &dyn ColorService) -> Result<Materials, Error> {
        let mut materials = Vec::new();
        for (name, color, picture) in &self.looks {
            let mut rgb = [*color];
            colors.to_working(color::LIN_REC709, &mut rgb)?;
            let surface = NewNode::materialx(SURFACE).expect("MaterialX's OpenPBR Surface");
            let surface = surface.with(BASE_COLOR, Value::Color(rgb[0])).displayed();
            let mut inside = NetworkSeed::default();
            let surface = inside.node(surface);
            if let Some(file) = picture.as_deref().and_then(|p| named.image(p, None)) {
                let texture = inside.node(NewNode::materialx(PICTURE).expect("MaterialX's image"));
                let source = ImageSource::new(file, ImageColor::Auto);
                let image = inside.node(image_node::reading(&source));
                inside.wire(image, image_node::IMAGE, texture, "file");
                inside.wire(texture, "out", surface, BASE_COLOR);
            }
            materials.push(ImportedMaterial {
                key: name.to_string(),
                name: name.to_string(),
                inside,
                standard: None,
                double_sided: true,
            });
        }
        let defined = |name: &Arc<str>| self.looks.iter().any(|(n, ..)| n == name);
        for name in self.names.iter().filter(|n| !defined(n)) {
            self.said.push(Diagnostic::warning(format!(
                "no material {name}; its faces show none"
            )));
        }
        Ok(Materials { materials })
    }

    /// Its points and faces as the standard Geometry, each face's
    /// `material` naming its material.
    fn geometry(&self) -> Result<Geometry, Error> {
        let count = self.points.len();
        let offsets = Column::from(self.offsets.clone());
        let primitives = Primitives::of_polygons(count, offsets, self.corners.clone().into())?;
        let mut attributes = Attributes::new();
        if !self.names.is_empty() {
            let text = Text::new(self.shows.clone().into(), self.names.clone().into())?;
            attributes =
                attributes.with(Domain::Primitive, names::MATERIAL, Attribute::text(text))?;
        }
        Geometry::new(self.points.clone().into(), primitives, attributes)
    }
}

impl FormatWriter for Points {
    fn accepts(&self) -> &[DataTypeId] {
        TYPES
    }
    /// Every material its items' faces show, and every item's points and
    /// polygons where the item is: what a file of points and faces holds of
    /// them. What it leaves out is said once.
    fn write(
        &self,
        path: &Path,
        items: &[FileItem],
        writing: &Writing<'_>,
        _options: &FormatOptions,
    ) -> Result<Vec<Diagnostic>, Error> {
        let factor = 1.0 / per_metre(writing.units);
        let mut text = String::new();
        let mut said = Vec::new();
        let mut beside = Beside::new(path);
        let looks = (items.iter().filter_map(|i| i.data.as_ref()))
            .filter_map(|d| standard::materials().get(d))
            .flat_map(|m| &m.materials);
        for material in looks {
            writing.check()?;
            match look(material, writing.colors, &mut beside)? {
                Some(line) => text.push_str(&line),
                None => said.push(format!("{}: no OpenPBR form; left out", material.key)),
            }
        }
        let mut placed: Vec<Xform> = Vec::with_capacity(items.len());
        let mut first = 1;
        for item in items {
            writing.check()?;
            let parent = item.parent.and_then(|p| placed.get(p).copied());
            let at = parent.map_or(item.placement, |p| p.compose(&item.placement));
            placed.push(at);
            let Some(geometry) = item.data.as_ref().and_then(|d| standard::geometry().get(d))
            else {
                continue;
            };
            for p in geometry.positions().iter() {
                let [x, y, z] = at.apply_point((*p).into()).to_array().map(|v| v * factor);
                let _ = writeln!(text, "{x} {y} {z}");
            }
            let primitives = geometry.primitives();
            let shows = geometry
                .attributes()
                .text(Domain::Primitive, names::MATERIAL);
            for face in primitives.polygons().iter() {
                let material = shows.and_then(|t| t.get(face as usize)).unwrap_or("-");
                let points = primitives
                    .primitive(face as usize)
                    .iter()
                    .map(|p| p + first);
                let points: Vec<String> = points.map(|p| p.to_string()).collect();
                let _ = writeln!(text, "face {} {}", word(material), points.join(" "));
            }
            if primitives.len() > primitives.polygons().len() {
                said.push(format!(
                    "{}: only polygons are written; its curves and lines are left out",
                    item.name
                ));
            }
            first += geometry.positions().len() as u32;
        }
        replace(path, |file| file.write_all(text.as_bytes()))
            .map_err(|e: std::io::Error| Error::invalid(format_args!("{}: {e}", path.display())))?;
        Ok(said.into_iter().map(Diagnostic::warning).collect())
    }
}

/// `material`'s line: its OpenPBR form's base color, out of the working
/// space into linear Rec.709, or the picture its base color reads, named
/// as `beside` names it; `None` when it has no OpenPBR form.
fn look(
    material: &ImportedMaterial,
    colors: &dyn ColorService,
    beside: &mut Beside,
) -> Result<Option<String>, Error> {
    let Ok(network) = MaterialNetwork::try_from(&material.inside) else {
        return Ok(None);
    };
    let Some(network) = network.translated("open_pbr_surface") else {
        return Ok(None);
    };
    let Some(surface) = network.surface() else {
        return Ok(None);
    };
    let node = &network.nodes()[surface as usize];
    let mut rgb = [match node.values().get(BASE_COLOR) {
        Some(Value::Color(c)) => *c,
        _ => [0.8; 3],
    }];
    colors.working_to(color::LIN_REC709, &mut rgb)?;
    let [r, g, b] = rgb[0].map(color::settled);
    let reads = (network.links().iter()).find(|l| l.to == surface && &*l.input == BASE_COLOR);
    let picture = reads.and_then(|l| network.nodes()[l.from as usize].images().get("file"));
    let picture = match picture.map(ImageSource::file) {
        Some(file @ (ImageFile::Path(_) | ImageFile::Embedded(_))) => {
            format!(" {}", word(&beside.name(file)?))
        }
        None => String::new(),
    };
    Ok(Some(format!(
        "material {} {r} {g} {b}{picture}\n",
        word(&material.key)
    )))
}

/// `name` as one word of the file: its spaces as underscores.
fn word(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join("_")
}

/// The package: what Studio registers when it loads the plugin.
pub fn package() -> Package {
    Package::new(PackageId::declared("example.points"), "XYZ Points").with_format(Points)
}

// The entry point Studio looks up in the plugin's library.
export_package!(package);
