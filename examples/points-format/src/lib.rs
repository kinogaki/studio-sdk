//! XYZ points: an example file format. A file is one point a line, three
//! numbers in metres apart by spaces (`#` starts a comment); it reads as
//! one value of the standard geometry holding those points and no
//! primitives, and the standard geometry writes as its points. Its one
//! option scales what it reads.
//!
//! A format converts what the file holds into the project: lengths into
//! its units (`Reading::units`), colors (none here) into its working space.
use kinogaki_studio_api::geometry::{Attributes, Geometry, Primitives, Units};
use kinogaki_studio_api::params::{Number, Parameter, Schema, Value};
use kinogaki_studio_api::{
    DataTypeId, Diagnostic, Error, FileFormat, FileItem, FormatId, FormatOptions, FormatWriter,
    Imported, Input, Package, PackageId, Purpose, Reading, Writing, export_package, standard,
};
use std::fmt::Write as _;
use std::path::Path;
use std::sync::LazyLock;

/// The scale option's name.
const SCALE: &str = "scale";

/// Millimetres in a metre, the file's unit.
const METRE: f64 = 1000.0;

/// What it reads and writes: the standard geometry.
static GEOMETRY: LazyLock<DataTypeId> = LazyLock::new(|| standard::geometry().id());

/// The format.
pub struct Points;

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
        std::slice::from_ref(&*GEOMETRY)
    }
    fn options(&self) -> FormatOptions {
        FormatOptions::from([(SCALE.to_owned(), Value::Number(1.0))])
    }
    fn schema(&self, options: &FormatOptions, _purpose: Purpose) -> Schema {
        let scale = options.get(SCALE).and_then(Value::number).unwrap_or(1.0);
        let number = Number::plain().range(1e-6, 1e6);
        Schema::default().parameter(Parameter::number(SCALE, "Scale", number, scale))
    }
    fn read(
        &self,
        input: &Input<'_>,
        options: &FormatOptions,
        reading: &Reading<'_>,
    ) -> Result<Imported, Error> {
        let bytes = input.bytes()?;
        let text = std::str::from_utf8(&bytes)
            .map_err(|_| Error::invalid(format_args!("{input}: not text")))?;
        let scale = options.get(SCALE).and_then(Value::number).unwrap_or(1.0);
        let factor = scale * per_metre(reading.units);
        let mut points = Vec::new();
        for (n, line) in text.lines().enumerate() {
            reading.check()?;
            let line = line.split('#').next().unwrap_or_default().trim();
            if line.is_empty() {
                continue;
            }
            let numbers: Vec<f64> = (line.split_whitespace())
                .map(str::parse)
                .collect::<Result<_, _>>()
                .map_err(|_| Error::invalid(format_args!("line {}: not numbers", n + 1)))?;
            let [x, y, z] = numbers[..] else {
                return Err(Error::invalid(format_args!(
                    "line {}: not three numbers",
                    n + 1
                )));
            };
            points.push([x * factor, y * factor, z * factor]);
        }
        let primitives = Primitives::empty().with_points(points.len())?;
        let geometry = Geometry::new(points.into(), primitives, Attributes::new())?;
        let name = Path::new(input.name()).file_stem();
        let name = name.map(|s| s.to_string_lossy().into_owned());
        let item = FileItem::top(
            name.unwrap_or_default(),
            standard::geometry().wrap(geometry),
        );
        Ok(Imported {
            items: vec![item],
            warnings: Vec::new(),
        })
    }
    fn writer(&self) -> Option<&dyn FormatWriter> {
        Some(self)
    }
}

impl FormatWriter for Points {
    fn accepts(&self) -> &[DataTypeId] {
        self.produces()
    }
    /// Every item's points, placed where its item is: what a file of
    /// points can hold of the geometry. What it leaves out is said once.
    fn write(
        &self,
        path: &Path,
        items: &[FileItem],
        writing: &Writing<'_>,
        _options: &FormatOptions,
    ) -> Result<Vec<Diagnostic>, Error> {
        let factor = 1.0 / per_metre(writing.units);
        let mut text = String::new();
        let mut left_out = false;
        let mut placed: Vec<_> = Vec::with_capacity(items.len());
        for item in items {
            writing.check()?;
            let parent = item.parent.and_then(|p| placed.get(p).copied());
            let at = parent.map_or(item.placement, |p: kinogaki_studio_api::geometry::Xform| {
                p.compose(&item.placement)
            });
            placed.push(at);
            let Some(geometry) = item.data.as_ref().and_then(|d| standard::geometry().get(d))
            else {
                continue;
            };
            left_out |= !geometry.primitives().is_empty();
            for p in geometry.positions().iter() {
                let [x, y, z] = at.apply_point((*p).into()).to_array().map(|v| v * factor);
                let _ = writeln!(text, "{x} {y} {z}");
            }
        }
        std::fs::write(path, text)
            .map_err(|e| Error::refused(format_args!("{}: {e}", path.display())))?;
        let note =
            || Diagnostic::warning("Only the points are written; their primitives are left out");
        Ok(left_out.then(note).into_iter().collect())
    }
}

/// The package: what Studio registers when it loads the plugin.
pub fn package() -> Package {
    Package::new(PackageId::declared("example.points"), "XYZ Points").with_format(Points)
}

// The entry point Studio looks up in the plugin's library.
export_package!(package);

#[cfg(test)]
mod tests;
