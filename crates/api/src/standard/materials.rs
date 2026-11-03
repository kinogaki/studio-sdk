//! [`PreviewMaterialsCodec`]: how a file's imported materials save.
use super::preview_materials;
use crate::codec::DataCodec;
use crate::data::Data;
use crate::element::{ElementReader, ElementWriter, Field};
use crate::error::Error;
use kinogaki_render_api::{Extra, ImportedMaterial, PreviewSurface};

/// Saves a file's materials as imported: `count`, then one `material`
/// child `m<i>` each holding its `key` and `name`, and its preview
/// surface's `diffuse`, `roughness`, `metallic`, `opacity`, `emissive`
/// (each empty when the file doesn't say) and `texture` (no text or one),
/// and its Kinogaki attributes as `extra_names`, `extra_kinds` (0 a
/// number, 1 three, 2 a name), their numbers in order (`extra_numbers`)
/// and their names (`extra_tokens`). A round trip is exact.
///
/// ```
/// use kinogaki_studio_api::DataCodec;
/// use kinogaki_studio_api::element::MemoryElement;
/// use kinogaki_studio_api::render::{ImportedMaterial, PreviewSurface};
/// use kinogaki_studio_api::standard::{self, PreviewMaterialsCodec};
///
/// let red = ImportedMaterial {
///     key: "red".into(),
///     name: "Red".into(),
///     preview: PreviewSurface { diffuse: Some([0.8, 0.1, 0.1]), ..PreviewSurface::default() },
/// };
/// let data = standard::preview_materials().wrap(vec![red.clone()]);
/// let mut saved = MemoryElement::new("result");
/// saved.write(|w| PreviewMaterialsCodec.write(&data, None, w)).unwrap();
/// let read = saved.read(|r| PreviewMaterialsCodec.read(r, None)).unwrap();
/// assert_eq!(standard::preview_materials().get(&read), Some(&vec![red]));
/// ```
#[derive(Clone, Copy, Debug, Default)]
pub struct PreviewMaterialsCodec;

impl DataCodec for PreviewMaterialsCodec {
    fn write(
        &self,
        data: &Data,
        _base: Option<&Data>,
        out: &mut dyn ElementWriter,
    ) -> Result<(), Error> {
        let materials = preview_materials()
            .get(data)
            .ok_or_else(|| Error::invalid("not imported materials"))?;
        out.set("count", Field::Int(materials.len() as i64))?;
        for (at, m) in materials.iter().enumerate() {
            write_material(m, &mut *out.child(&format!("m{at}"), "material")?)?;
        }
        Ok(())
    }

    fn read(&self, element: &dyn ElementReader, _base: Option<&Data>) -> Result<Data, Error> {
        let count = usize::try_from(element.int("count")?).map_err(|_| {
            Error::invalid(format_args!("{}: a negative count", element.location()))
        })?;
        let materials = (0..count)
            .map(|at| read_material(&*element.required_child(&format!("m{at}"))?))
            .collect::<Result<Vec<_>, Error>>()?;
        Ok(preview_materials().wrap(materials))
    }
}

fn write_material(m: &ImportedMaterial, out: &mut dyn ElementWriter) -> Result<(), Error> {
    let p = &m.preview;
    let reals = |v: Option<&[f64]>| Field::F64s(v.map_or_else(Vec::new, <[f64]>::to_vec));
    out.set("key", Field::Text(m.key.clone()))?;
    out.set("name", Field::Text(m.name.clone()))?;
    out.set("diffuse", reals(p.diffuse.as_ref().map(|v| &v[..])))?;
    out.set(
        "roughness",
        reals(p.roughness.as_ref().map(std::slice::from_ref)),
    )?;
    out.set(
        "metallic",
        reals(p.metallic.as_ref().map(std::slice::from_ref)),
    )?;
    out.set(
        "opacity",
        reals(p.opacity.as_ref().map(std::slice::from_ref)),
    )?;
    out.set("emissive", reals(p.emissive.as_ref().map(|v| &v[..])))?;
    out.set("texture", Field::Texts(p.texture.iter().cloned().collect()))?;
    let (mut kinds, mut numbers, mut tokens) = (Vec::new(), Vec::new(), Vec::new());
    for (_, extra) in &p.kinogaki {
        match extra {
            Extra::Number(n) => {
                kinds.push(0);
                numbers.push(*n);
            }
            Extra::Triple(t) => {
                kinds.push(1);
                numbers.extend(t);
            }
            Extra::Token(t) => {
                kinds.push(2);
                tokens.push(t.clone());
            }
        }
    }
    let names = p.kinogaki.iter().map(|(n, _)| n.clone()).collect();
    out.set("extraNames", Field::Texts(names))?;
    out.set("extraKinds", Field::U8s(kinds))?;
    out.set("extraNumbers", Field::F64s(numbers))?;
    out.set("extraTokens", Field::Texts(tokens))
}

fn read_material(element: &dyn ElementReader) -> Result<ImportedMaterial, Error> {
    let at = element.location();
    let wrong = |name: &str| Error::invalid(format_args!("{at}: {name} has the wrong length"));
    let one = |name: &str| -> Result<Option<f64>, Error> {
        match element.f64s(name)? {
            [] => Ok(None),
            [v] => Ok(Some(*v)),
            _ => Err(wrong(name)),
        }
    };
    let three = |name: &str| -> Result<Option<[f64; 3]>, Error> {
        match element.f64s(name)? {
            [] => Ok(None),
            &[a, b, c] => Ok(Some([a, b, c])),
            _ => Err(wrong(name)),
        }
    };
    let texture = match element.texts("texture")? {
        [] => None,
        [t] => Some(t.clone()),
        _ => return Err(wrong("texture")),
    };
    let names = element.texts("extraNames")?;
    let kinds = super::attributes::u8s(element, "extraKinds")?;
    if kinds.len() != names.len() {
        return Err(wrong("extraKinds"));
    }
    let (mut numbers, mut tokens) = (
        element.f64s("extraNumbers")?.iter().copied(),
        element.texts("extraTokens")?.iter().cloned(),
    );
    let mut next = || numbers.next().ok_or_else(|| wrong("extraNumbers"));
    let mut kinogaki = Vec::with_capacity(names.len());
    for (name, kind) in names.iter().zip(kinds) {
        let extra = match kind {
            0 => Extra::Number(next()?),
            1 => Extra::Triple([next()?, next()?, next()?]),
            2 => Extra::Token(tokens.next().ok_or_else(|| wrong("extraTokens"))?),
            k => return Err(Error::invalid(format_args!("{at}: no attribute kind {k}"))),
        };
        kinogaki.push((name.clone(), extra));
    }
    if next().is_ok() {
        return Err(wrong("extraNumbers"));
    }
    if tokens.next().is_some() {
        return Err(wrong("extraTokens"));
    }
    Ok(ImportedMaterial {
        key: element.text("key")?.to_owned(),
        name: element.text("name")?.to_owned(),
        preview: PreviewSurface {
            diffuse: three("diffuse")?,
            roughness: one("roughness")?,
            metallic: one("metallic")?,
            opacity: one("opacity")?,
            emissive: three("emissive")?,
            texture,
            kinogaki,
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::element::MemoryElement;

    #[test]
    fn every_field_and_attribute_round_trips() {
        let full = ImportedMaterial {
            key: "steel".into(),
            name: "Steel 2".into(),
            preview: PreviewSurface {
                diffuse: Some([0.5, 0.5, 0.55]),
                roughness: Some(0.3),
                metallic: Some(1.0),
                opacity: Some(0.9),
                emissive: Some([0.0, 1.0, 2.0]),
                texture: Some("tex/steel.png".into()),
                kinogaki: vec![
                    ("kinogaki:anisotropy".into(), Extra::Number(0.25)),
                    ("kinogaki:sheen".into(), Extra::Triple([1.0, 0.5, 0.25])),
                    ("kinogaki:surface".into(), Extra::Token("brushed".into())),
                ],
            },
        };
        let bare = ImportedMaterial {
            key: "plain".into(),
            name: "Plain".into(),
            preview: PreviewSurface::default(),
        };
        let materials = vec![full, bare];
        let data = preview_materials().wrap(materials.clone());
        let mut saved = MemoryElement::new("result");
        saved
            .write(|w| PreviewMaterialsCodec.write(&data, None, w))
            .unwrap();
        let read = saved.read(|r| PreviewMaterialsCodec.read(r, None)).unwrap();
        assert_eq!(preview_materials().get(&read), Some(&materials));
        let none = preview_materials().wrap(Vec::new());
        let mut saved = MemoryElement::new("result");
        saved
            .write(|w| PreviewMaterialsCodec.write(&none, None, w))
            .unwrap();
        let read = saved.read(|r| PreviewMaterialsCodec.read(r, None)).unwrap();
        assert_eq!(preview_materials().get(&read), Some(&Vec::new()));
    }
}
