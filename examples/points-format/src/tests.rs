//! The format as a conformance consumer: every standard geometry fixture
//! written and read back keeps its points.
use super::Points;
use kinogaki_studio_api::geometry::Units;
use kinogaki_studio_api::{
    Data, FileFormat, FileItem, FormatOptions, Input, Purpose, Reading, Writing, standard,
};
use kinogaki_studio_api_testing::{Consumer, Fixtures, Unconverted, run};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

/// A file of its own in the system's temporary folder, gone when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new() -> Scratch {
        static NEXT: AtomicU32 = AtomicU32::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let name = format!("example-points-{}-{n}.xyz", std::process::id());
        Scratch(std::env::temp_dir().join(name))
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

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
        let file = Scratch::new();
        let colors = Unconverted;
        let units = Units::default();
        let writer = Points.writer().ok_or("no writer")?;
        let item = FileItem::top("value", data.clone());
        let writing = Writing::new(units, &colors);
        (writer.write(&file.0, &[item], &writing, &FormatOptions::new()))
            .map_err(|e| e.to_string())?;
        let reading = Reading::new(Purpose::Objects, units, &colors);
        let read = (Points.read(&Input::file(&file.0), &Points.options(), &reading))
            .map_err(|e| e.to_string())?;
        let back = (read.items.first().and_then(|i| i.data.as_ref()))
            .and_then(|d| standard::geometry().get(d))
            .ok_or("no geometry read")?;
        match back.positions().as_slice() == geometry.positions().as_slice() {
            true => Ok(()),
            false => Err("the points moved".into()),
        }
    }
}

#[test]
fn every_fixtures_points_survive_a_round_trip() {
    let report = run(&[&Fixtures::default()], &[&RoundTrip]);
    assert!(report.failures().is_empty(), "{report}");
}

#[test]
fn a_file_reads_in_the_projects_units_scaled_by_its_option() {
    let file = Scratch::new();
    std::fs::write(&file.0, "# one point\n1 2 3\n\n0.5 0 -1  # another\n").unwrap();
    let colors = Unconverted;
    let millimetres = Units::MILLIMETERS;
    let mut options = Points.options();
    options.insert("scale".into(), 2.0.into());
    let reading = Reading::new(Purpose::Value, millimetres, &colors);
    let read = Points
        .read(&Input::file(&file.0), &options, &reading)
        .unwrap();
    let points = standard::geometry()
        .get(read.items[0].data.as_ref().unwrap())
        .unwrap();
    assert_eq!(
        points.positions().as_slice(),
        &[[2000.0, 4000.0, 6000.0], [1000.0, 0.0, -2000.0]]
    );
}

#[test]
fn a_line_of_two_numbers_is_refused_naming_it() {
    let file = Scratch::new();
    std::fs::write(&file.0, "1 2 3\n4 5\n").unwrap();
    let colors = Unconverted;
    let reading = Reading::new(Purpose::Value, Units::default(), &colors);
    let error = Points
        .read(&Input::file(&file.0), &Points.options(), &reading)
        .unwrap_err();
    assert!(error.to_string().contains("line 2"), "{error}");
    // In memory, as an embedded file reads.
    let held = Input::memory("held.xyz", b"1 2 3\n");
    let read = Points.read(&held, &Points.options(), &reading).unwrap();
    assert_eq!(read.items[0].name, "held");
}
