//! MaterialX's own documents (a subset of its test suite and examples, 1.39.5,
//! Apache-2.0; `tests/documents/read`) read and written back as MaterialX
//! writes them: as they were, or, where MaterialX writes them otherwise
//! (`tests/documents/written`, made by MaterialX 1.39.5's own
//! `readFromXmlFile` and `writeToXmlString`, comments and blank lines kept),
//! as it does. Among them are its 1.36, 1.37 and 1.38 syntax documents and
//! two 1.38 materials: those read upgraded to 1.39, as MaterialX upgrades
//! them.
use kinogaki_mtlx::Document;
use std::path::Path;

#[test]
fn documents_write_back_as_materialx_writes_them() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/documents");
    let mut read = 0;
    let mut entries: Vec<_> = std::fs::read_dir(root.join("read"))
        .expect("the documents")
        .map(|e| e.expect("an entry").path())
        .collect();
    entries.sort();
    for path in entries {
        let name = path.file_name().expect("a name");
        let text = std::fs::read_to_string(&path).expect("UTF-8");
        let written = root.join("written").join(name);
        let expected = std::fs::read_to_string(&written).unwrap_or_else(|_| text.clone());
        let document: Document = text.parse().unwrap_or_else(|why| panic!("{name:?}: {why}"));
        assert_eq!(document.to_string(), expected, "{name:?}");
        // What it writes reads back to the same document.
        let again: Document = expected.parse().expect("its own writing");
        assert_eq!(again, document, "{name:?}");
        read += 1;
    }
    assert_eq!(read, 25);
}

#[test]
fn what_isnt_materialx_is_refused() {
    let refused = |text: &str| text.parse::<Document>().unwrap_err();
    assert!(refused("<usd />").contains("isn't a MaterialX document"));
    assert!(refused("<materialx version=\"1.35\" />").contains("1.35 isn't read"));
    assert!(refused("<materialx version=\"1.39\">").contains("isn't closed"));
}

#[test]
fn includes_comments_and_unknown_attributes_are_kept() {
    let text = "<?xml version=\"1.0\"?>\n<materialx version=\"1.39\" kinogaki:doublesided=\"false\">\n  <xi:include href=\"other.mtlx\" />\n  <!-- kept -->\n\n  <surfacematerial name=\"m\" type=\"material\" kinogaki:doublesided=\"false\" />\n</materialx>\n";
    let document: Document = text.parse().unwrap();
    assert_eq!(document.to_string(), text);
}
