//! What files that know only a preview surface (UsdPreviewSurface, OBJ's
//! MTL) carry of a material: [`PreviewSurface`], and a file's materials as
//! an import lists them ([`ImportedMaterial`]). Each engine reads one of
//! its materials as a preview surface for export and makes a material of
//! its own from one on import ([`crate::EngineDescription`]'s `preview` and
//! `from_preview`); hosts read and write the files.

/// A preview surface's parameters (none: the format's default).
#[derive(Clone, Debug, Default, PartialEq)]
pub struct PreviewSurface {
    /// The diffuse color (linear).
    pub diffuse: Option<[f64; 3]>,
    /// The specular roughness.
    pub roughness: Option<f64>,
    /// How metallic it is.
    pub metallic: Option<f64>,
    /// Its opacity.
    pub opacity: Option<f64>,
    /// The emitted color, in nits.
    pub emissive: Option<[f64; 3]>,
    /// The diffuse color's image file, as the file names it.
    pub texture: Option<String>,
    /// What a preview surface cannot say, as Kinogaki's own attributes
    /// (`kinogaki:*`), in name order.
    pub kinogaki: Vec<(String, Extra)>,
}

/// A material a file brings: the key its polygons' `material` values
/// name, the name it takes in the scene's library, and its preview
/// surface.
#[derive(Clone, Debug, PartialEq)]
pub struct ImportedMaterial {
    /// What the file's polygons name it by.
    pub key: String,
    /// The name it takes in the library.
    pub name: String,
    /// Its parameters.
    pub preview: PreviewSurface,
}

/// A Kinogaki attribute's value.
#[derive(Clone, Debug, PartialEq)]
pub enum Extra {
    /// A number.
    Number(f64),
    /// Three numbers.
    Triple([f64; 3]),
    /// A name.
    Token(String),
}
