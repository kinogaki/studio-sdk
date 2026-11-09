//! The format as a conformance consumer (every standard geometry fixture
//! written and read back keeps its points), and its materials both ways.
use super::{BASE_COLOR, Points};
use kinogaki_render_api::MaterialNetwork;
use kinogaki_studio_api::color::{ColorService, LIN_REC709};
use kinogaki_studio_api::geometry::image::ImageFile;
use kinogaki_studio_api::geometry::{Domain, Units, names};
use kinogaki_studio_api::{
    Data, Error, FileFormat, FileItem, Imported, Input, Purpose, Reading, Value, standard,
};
use kinogaki_studio_api_testing::format::{Scratch, document, round_trip};
use kinogaki_studio_api_testing::{Consumer, Fixtures, Unconverted, run};

/// Writes each value and reads it back.
struct RoundTrip;

impl Consumer for RoundTrip {
    fn name(&self) -> &str {
        "XYZ round trip"
    }
    fn check(&self, _fixture: &str, data: &Data) -> Result<(), String> {
        let Some(geometry) = standard::geometry().get(data) else {
            return Ok(());
        };
        let scratch = Scratch::new("points-conformance");
        let items = [FileItem::top("value", data.clone())];
        let (read, _) = round_trip(
            &Points,
            (&items, None),
            &scratch.join("value.xyz"),
            Units::default(),
        )
        .map_err(|e| e.to_string())?;
        let back = geometry_of(&read).ok_or("no geometry read")?;
        match back.positions().as_slice() == geometry.positions().as_slice() {
            true => Ok(()),
            false => Err("the points moved".into()),
        }
    }
}

/// The geometry a read made.
fn geometry_of(read: &Imported) -> Option<&kinogaki_studio_api::geometry::Geometry> {
    (read.items.iter().filter_map(|i| i.data.as_ref())).find_map(|d| standard::geometry().get(d))
}

/// `text`, the file `name` in `scratch`, read with `options` into `units`
/// and `colors`' working space.
fn read_text(
    scratch: &Scratch,
    name: &str,
    text: &str,
    options: &kinogaki_studio_api::FormatOptions,
    colors: &dyn ColorService,
) -> Result<Imported, Error> {
    let path = scratch.join(name);
    std::fs::write(&path, text).unwrap();
    let reading = Reading::new(Purpose::Objects, Units::default(), colors);
    Points.read(&Input::file(&path), options, &reading)
}

#[test]
fn every_fixtures_points_survive_a_round_trip() {
    let report = run(&[&Fixtures::default()], &[&RoundTrip]);
    assert!(report.failures().is_empty(), "{report}");
}

#[test]
fn a_file_reads_in_the_projects_units_scaled_by_its_option() {
    let scratch = Scratch::new("points-units");
    let path = scratch.join("two.xyz");
    std::fs::write(&path, "# one point\n1 2 3\n\n0.5 0 -1  # another\n").unwrap();
    let mut options = Points.options();
    options.insert("scale".into(), 2.0.into());
    let reading = Reading::new(Purpose::Value, Units::MILLIMETERS, &Unconverted);
    let read = Points
        .read(&Input::file(&path), &options, &reading)
        .unwrap();
    let points = geometry_of(&read).unwrap();
    assert_eq!(
        points.positions().as_slice(),
        &[[2000.0, 4000.0, 6000.0], [1000.0, 0.0, -2000.0]]
    );
}

#[test]
fn a_file_opens_in_its_metres_as_import_reads_it() {
    let scratch = Scratch::new("points-document");
    std::fs::write(scratch.join("clay.png"), b"a picture").unwrap();
    let path = scratch.join("clay.xyz");
    std::fs::write(&path, CLAY).unwrap();
    let opened = document(&Points, &Input::file(&path), &Points.options()).unwrap();
    assert_eq!(opened.meters_per_unit(), 1.0);
}

#[test]
fn a_bad_line_is_refused_naming_it() {
    let scratch = Scratch::new("points-bad");
    let options = Points.options();
    for (text, why) in [
        ("1 2 3\n4 5\n", "line 2: not a point"),
        (
            "1 2 3\n4 5 6\nface - 1 2\n",
            "line 3: a face has three points",
        ),
        ("1 2 3\n4 5 6\n7 8 9\nface - 1 2 4\n", "line 4: no point 4"),
        ("material Red 1 0 x\n", "line 1: not numbers"),
    ] {
        let error = read_text(&scratch, "bad.xyz", text, &options, &Unconverted).unwrap_err();
        assert!(error.message().contains(why), "{text:?}: {error}");
    }
    // In memory, as an embedded file reads.
    let held = Input::memory("held.xyz", b"1 2 3\n");
    let reading = Reading::new(Purpose::Value, Units::default(), &Unconverted);
    let read = Points.read(&held, &options, &reading).unwrap();
    assert_eq!(read.items[0].name, "held");
}

/// A color service of one conversion: linear Rec.709 is the working space
/// at half its values.
#[derive(Debug)]
struct Halving;

impl ColorService for Halving {
    fn working(&self) -> &str {
        "half"
    }
    fn convert(&self, from: &str, to: &str, rgb: &mut [[f64; 3]]) -> Result<(), Error> {
        let factor = match (from, to) {
            (LIN_REC709, "half") => 0.5,
            ("half", LIN_REC709) => 2.0,
            _ => return Err(Error::invalid(format_args!("no {from} to {to}"))),
        };
        rgb.iter_mut().flatten().for_each(|v| *v *= factor);
        Ok(())
    }
    fn revision(&self) -> u64 {
        0
    }
}

const CLAY: &str = "material Clay 0.8 0.4 0.2 clay.png\nmaterial Red 1 0 0\n0 0 0\n1 0 0\n1 1 0\n0 1 0\nface Clay 1 2 3\nface Red 1 3 4\nface - 4 3 2\n";

#[test]
fn materials_read_first_their_colors_in_the_working_space_and_pictures_as_image_nodes() {
    let scratch = Scratch::new("points-materials");
    std::fs::write(scratch.join("clay.png"), b"a picture").unwrap();
    let read = read_text(&scratch, "clay.xyz", CLAY, &Points.options(), &Halving).unwrap();
    let looks = standard::materials().get(read.items[0].data.as_ref().unwrap());
    let looks = looks.expect("the materials item first");
    let keys: Vec<&str> = looks.materials.iter().map(|m| m.key.as_str()).collect();
    assert_eq!(keys, ["Clay", "Red"]);
    // Red: its color, halved into the working space.
    let red = MaterialNetwork::try_from(&looks.materials[1].inside).expect("MaterialX's nodes");
    let surface = &red.nodes()[red.surface().unwrap() as usize];
    assert_eq!(surface.values()[BASE_COLOR], Value::Color([0.5, 0.0, 0.0]));
    // Clay: its picture, read through an Image node into an image texture.
    let clay = MaterialNetwork::try_from(&looks.materials[0].inside).unwrap();
    let surface = clay.surface().unwrap();
    let link = (clay.links().iter()).find(|l| l.to == surface && &*l.input == BASE_COLOR);
    let texture = &clay.nodes()[link.expect("a wired base color").from as usize];
    let file = texture.images()["file"].file();
    let ImageFile::Path(path) = file else {
        panic!("a path when Embed Textures is off: {file:?}");
    };
    assert!(path.ends_with("clay.png"), "{path}");
    // The faces name their materials; the last none.
    let geometry = geometry_of(&read).unwrap();
    let shows = geometry
        .attributes()
        .text(Domain::Primitive, names::MATERIAL)
        .unwrap();
    assert_eq!(
        [0, 1, 2].map(|f| shows.get(f)),
        [Some("Clay"), Some("Red"), None]
    );
    // Embedded, the picture's bytes are the project's.
    let mut embed = Points.options();
    embed.insert("embed".into(), true.into());
    let read = read_text(&scratch, "clay.xyz", CLAY, &embed, &Halving).unwrap();
    let looks = standard::materials()
        .get(read.items[0].data.as_ref().unwrap())
        .unwrap();
    let clay = MaterialNetwork::try_from(&looks.materials[0].inside).unwrap();
    let embedded = clay
        .nodes()
        .iter()
        .find_map(|n| n.images().get("file"))
        .unwrap();
    let ImageFile::Embedded(held) = embedded.file() else {
        panic!("embedded with Embed Textures on");
    };
    assert_eq!(held.bytes(), b"a picture");
}

#[test]
fn materials_and_faces_write_back_as_they_read() {
    let scratch = Scratch::new("points-back");
    std::fs::write(scratch.join("clay.png"), b"a picture").unwrap();
    let read = read_text(&scratch, "clay.xyz", CLAY, &Points.options(), &Unconverted).unwrap();
    let out = scratch.join("out");
    std::fs::create_dir(&out).unwrap();
    let (back, said) = round_trip(
        &Points,
        (&read.items, None),
        &out.join("clay.xyz"),
        Units::default(),
    )
    .unwrap();
    assert!(said.is_empty(), "{said:?}");
    let written = std::fs::read_to_string(out.join("clay.xyz")).unwrap();
    assert!(
        written.starts_with("material Clay 0.8 0.4 0.2 ../clay.png\nmaterial Red 1 0 0\n"),
        "{written}"
    );
    assert!(
        written.ends_with("face Clay 1 2 3\nface Red 1 3 4\nface - 4 3 2\n"),
        "{written}"
    );
    let (a, b) = (geometry_of(&read).unwrap(), geometry_of(&back).unwrap());
    assert_eq!(a.positions().as_slice(), b.positions().as_slice());
    assert_eq!(
        a.primitives().vertices().as_slice(),
        b.primitives().vertices().as_slice()
    );
    // An embedded picture is written beside the file, once.
    let mut embed = Points.options();
    embed.insert("embed".into(), true.into());
    let read = read_text(&scratch, "clay.xyz", CLAY, &embed, &Unconverted).unwrap();
    let packed = scratch.join("packed");
    std::fs::create_dir(&packed).unwrap();
    round_trip(
        &Points,
        (&read.items, None),
        &packed.join("clay.xyz"),
        Units::default(),
    )
    .unwrap();
    let written = std::fs::read_to_string(packed.join("clay.xyz")).unwrap();
    assert!(
        written.starts_with("material Clay 0.8 0.4 0.2 clay_textures/clay.png\n"),
        "{written}"
    );
    assert_eq!(
        std::fs::read(packed.join("clay_textures/clay.png")).unwrap(),
        b"a picture"
    );
}
