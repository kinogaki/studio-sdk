use super::*;
use crate::color::ColorService;
use crate::format::files::{Beside, Named};
use crate::format::{FormatOptions, Input, Purpose, Reading, Writing};
use crate::image_files::image_source;
use crate::standard::ImportedMaterial;
use crate::{Error, Units};
use kinogaki_geometry::image::ImageFile;

/// A color service converting nothing.
#[derive(Debug)]
struct Unconverted;
impl ColorService for Unconverted {
    fn working(&self) -> &str {
        kinogaki_render_api::RENDER_SPACE
    }
    fn convert(&self, _: &str, _: &str, _: &mut [[f64; 3]]) -> Result<(), Error> {
        Ok(())
    }
    fn revision(&self) -> u64 {
        0
    }
}

#[test]
fn a_colorspace_of_no_conversion_reads_as_data_and_writes_as_none() {
    for data in ["raw", "RAW", "none", "data", "Data"] {
        assert_eq!(image_color(Some(data), true), ImageColor::Data, "{data}");
    }
    assert_eq!(image_color(None, true), ImageColor::Auto);
    let texture = image_color(Some("srgb_texture"), true);
    assert_eq!(texture, ImageColor::space("srgb_texture").unwrap());
    // What is written reads back as itself.
    for color in [ImageColor::Auto, ImageColor::Data, texture] {
        assert_eq!(image_color(colorspace(&color), true), color);
    }
    // An image giving no color is data, whatever it says.
    for said in [None, Some("srgb_texture")] {
        assert_eq!(image_color(said, false), ImageColor::Data, "{said:?}");
    }
}

/// A document's float, vector3 and color3 images naming no `colorspace`: the
/// float's and the vector's read as data (MaterialX converts colors alone),
/// the color's as its file says.
#[test]
fn an_image_giving_no_color_reads_as_data() {
    let dir = std::env::temp_dir().join(format!("kinogaki-sdk-mtlx-data-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for file in ["rough.png", "normal.png", "color.png"] {
        std::fs::write(dir.join(file), [1, 2, 3]).unwrap();
    }
    let text = r#"<?xml version="1.0"?>
<materialx version="1.39">
  <image name="rough" type="float"><input name="file" type="filename" value="rough.png" /></image>
  <image name="normal" type="vector3"><input name="file" type="filename" value="normal.png" /></image>
  <image name="color" type="color3"><input name="file" type="filename" value="color.png" /></image>
  <normalmap name="bent" type="vector3"><input name="in" type="vector3" nodename="normal" /></normalmap>
  <standard_surface name="surface" type="surfaceshader">
    <input name="base_color" type="color3" nodename="color" />
    <input name="specular_roughness" type="float" nodename="rough" />
    <input name="normal" type="vector3" nodename="bent" />
  </standard_surface>
  <surfacematerial name="Look" type="material">
    <input name="surfaceshader" type="surfaceshader" nodename="surface" />
  </surfacematerial>
</materialx>
"#;
    let path = dir.join("look.mtlx");
    std::fs::write(&path, text).unwrap();
    let input = Input::file(&path);
    let reading = Reading::new(Purpose::Objects, Units::METERS, &Unconverted);
    let named = Named::new(&input, &FormatOptions::new(), &reading);
    let (materials, said) = read(text, &input, &named, &reading).unwrap();
    assert!(said.is_empty(), "{said:?}");
    let colors: Vec<(String, ImageColor)> = (materials[0].inside.nodes.iter())
        .filter(|n| n.node_type == crate::image_files::IMAGE_NODE)
        .map(|n| {
            let source = image_source(&n.parameters).unwrap().unwrap();
            (n.name.clone().unwrap_or_default(), source.color().clone())
        })
        .collect();
    let of = |name: &str| {
        colors
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, c)| c.clone())
    };
    assert_eq!(of("rough"), Some(ImageColor::Data), "{colors:?}");
    assert_eq!(of("normal"), Some(ImageColor::Data), "{colors:?}");
    assert_eq!(of("color"), Some(ImageColor::Auto), "{colors:?}");
    std::fs::remove_dir_all(&dir).unwrap();
}

/// A document naming `wood.png` beside it with `colorspace="none"` and a
/// color in `lin_rec709`: read, its image is data, and written and read
/// again, the same material.
#[test]
fn a_documents_material_reads_and_writes_back_as_itself() {
    let dir = std::env::temp_dir().join(format!("kinogaki-sdk-mtlx-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("wood.png"), [1, 2, 3]).unwrap();
    let text = r#"<?xml version="1.0"?>
<materialx version="1.39" colorspace="lin_rec709">
  <image name="bump" type="float">
    <input name="file" type="filename" value="wood.png" colorspace="none" />
  </image>
  <standard_surface name="surface" type="surfaceshader">
    <input name="base_color" type="color3" value="0.5, 0.25, 0.125" />
    <input name="specular_roughness" type="float" nodename="bump" />
  </standard_surface>
  <surfacematerial name="Wood" type="material">
    <input name="surfaceshader" type="surfaceshader" nodename="surface" />
  </surfacematerial>
</materialx>
"#;
    let path = dir.join("look.mtlx");
    std::fs::write(&path, text).unwrap();
    let input = Input::file(&path);
    let reading = Reading::new(Purpose::Objects, Units::METERS, &Unconverted);
    let named = Named::new(&input, &FormatOptions::new(), &reading);
    let (materials, said) = read(text, &input, &named, &reading).unwrap();
    assert!(said.is_empty() && named.warnings().is_empty(), "{said:?}");
    let [wood] = &materials[..] else {
        panic!("one material: {materials:?}");
    };
    let image = (wood.inside.nodes.iter())
        .find(|n| n.node_type == crate::image_files::IMAGE_NODE)
        .expect("an Image node");
    let source = image_source(&image.parameters).unwrap().unwrap();
    assert_eq!(*source.color(), ImageColor::Data);
    assert!(matches!(source.file(), ImageFile::Path(p) if p.ends_with("wood.png")));
    // Written beside it and read again: the same material.
    let mut files = Beside::new(&path);
    let writing = Writing::new(Units::METERS, &Unconverted);
    let (document, said) = write(&[wood], &mut files, &writing).unwrap();
    assert!(said.is_empty(), "{said:?}");
    let written = document.to_string();
    assert!(written.contains(r#"colorspace="none""#), "{written}");
    let (again, _) = read(&written, &input, &named, &reading).unwrap();
    let names = |m: &ImportedMaterial| -> Vec<String> {
        let mut names: Vec<String> = (m.inside.nodes.iter())
            .map(|n| n.node_type.to_string())
            .collect();
        names.sort();
        names
    };
    assert_eq!(names(&again[0]), names(wood));
    assert_eq!(again[0].inside.wires.len(), wood.inside.wires.len());
    let image = |m: &ImportedMaterial| {
        let node = (m.inside.nodes.iter()).find(|n| n.node_type == crate::image_files::IMAGE_NODE);
        image_source(&node.unwrap().parameters).unwrap()
    };
    assert_eq!(image(&again[0]), image(wood));
    std::fs::remove_dir_all(&dir).unwrap();
}
