//! [`read_hostile`]: a format reading a hostile file as File ▸ Import
//! reads one, for a format's tests of the files that once broke it.
use crate::Unconverted;
use kinogaki_studio_api::format::{ReadBudget, ReadLimits};
use kinogaki_studio_api::{
    Error, FileFormat, FormatOptions, Imported, Input, Purpose, Reading, Units,
};
use std::time::{Duration, Instant};

/// The stack a read runs on: Rust's default for a spawned thread, which
/// File ▸ Import's worker has.
pub const IMPORT_STACK: usize = 2 << 20;

/// `format` reading `input` with `options` as File ▸ Import does, on a
/// thread of [`IMPORT_STACK`], within `limits`, in metres with Y up and
/// colors unconverted: what it read and how long it took. A reader
/// recursing past the stack aborts the test, as it would end Studio; one
/// asking more than `limits` allow fails with an error, which a test
/// asserts with the time it took.
///
/// ```
/// use kinogaki_studio_api::format::ReadLimits;
/// use kinogaki_studio_api::{Error, FileFormat, FormatOptions, Imported, Input, Reading};
/// use kinogaki_studio_api_testing::read_hostile;
/// # use kinogaki_studio_api::{DataTypeId, FormatId};
/// struct Nested;
/// impl FileFormat for Nested {
///     // A file of `(` nests as deep as it is long.
///     fn read(&self, input: &Input<'_>, _: &FormatOptions, r: &Reading<'_>) -> Result<Imported, Error> {
///         let depth = input.bytes()?.iter().take_while(|&&b| b == b'(').count();
///         r.budget.nest(depth)?;
///         Ok(Imported::default())
///     }
/// #   fn id(&self) -> FormatId { FormatId::declared("acme.format.nested") }
/// #   fn label(&self) -> &str { "Nested" }
/// #   fn extensions(&self) -> &[&str] { &["nest"] }
/// #   fn produces(&self) -> &[DataTypeId] { &[] }
/// }
/// let deep = vec![b'('; 100_000];
/// let input = Input::memory("deep.nest", &deep);
/// let (read, took) = read_hostile(&Nested, &input, &FormatOptions::new(), ReadLimits::default());
/// assert!(read.unwrap_err().message().contains("nested deeper than 256"));
/// assert!(took.as_secs() < 1);
/// ```
pub fn read_hostile(
    format: &dyn FileFormat,
    input: &Input<'_>,
    options: &FormatOptions,
    limits: ReadLimits,
) -> (Result<Imported, Error>, Duration) {
    std::thread::scope(|scope| {
        let read = std::thread::Builder::new()
            .name("hostile-read".into())
            .stack_size(IMPORT_STACK)
            .spawn_scoped(scope, || {
                let reading = Reading {
                    budget: ReadBudget::new(limits),
                    ..Reading::new(Purpose::Objects, Units::default(), &Unconverted)
                };
                let start = Instant::now();
                let read = format.read(input, options, &reading);
                (read, start.elapsed())
            })
            .expect("a thread for the read");
        read.join().expect("the read ends without a panic")
    })
}
