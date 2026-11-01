//! The conformance suite of the standard data types: producers make
//! standard values, consumers check them, and [`run`] tries every pair.
//!
//! A kernel implements [`Producer`] in its own repository (its operations'
//! outputs over the fixtures), a render engine or a format implements
//! [`Consumer`] (it draws, writes or reads each value and checks what it
//! got), and each runs the suite with the SDK's own parts beside its own:
//! [`Fixtures`] (hand-built values covering every shape and attribute
//! kind), [`Codecs`] (a bit-exact save and read) and [`Invariants`] (each
//! value rebuilt through the validating constructors is equal, and its
//! edges are canonical).
//!
//! ```
//! use kinogaki_studio_api::conformance::{self, Codecs, Fixtures, Invariants};
//!
//! let report = conformance::run(&[&Fixtures::default()], &[&Codecs, &Invariants]);
//! assert!(report.failures().is_empty(), "{report}");
//! ```
mod brep;
mod checks;
mod colors;
mod fixtures;

pub use checks::{Codecs, Invariants};
pub use colors::Unconverted;
pub use fixtures::Fixtures;

use crate::data::Data;
use std::fmt;

/// Makes standard values to check: a kernel's operations over the
/// fixtures, a format's reads, the SDK's [`Fixtures`].
pub trait Producer {
    /// Its name in the report.
    fn name(&self) -> &str;
    /// Its values, each named for the report (`"quad grid"`).
    fn fixtures(&self) -> Vec<(String, Data)>;
}

/// Checks standard values: draws, renders, writes or reads them.
pub trait Consumer {
    /// Its name in the report.
    fn name(&self) -> &str;
    /// Checks `data`, the producer's fixture named `fixture`: `Err` says
    /// what is wrong. A value of a type it doesn't take passes.
    fn check(&self, fixture: &str, data: &Data) -> Result<(), String>;
}

/// One check of one fixture by one consumer.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Outcome {
    /// The producer.
    pub producer: String,
    /// The fixture.
    pub fixture: String,
    /// The consumer.
    pub consumer: String,
    /// `Err` with what the consumer found wrong.
    pub result: Result<(), String>,
}

/// Every check [`run`] made, in order: producers, their fixtures, then
/// consumers. Displays as a table.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Report {
    /// The outcomes.
    pub outcomes: Vec<Outcome>,
}

impl Report {
    /// The failed checks.
    pub fn failures(&self) -> Vec<&Outcome> {
        self.outcomes.iter().filter(|o| o.result.is_err()).collect()
    }
}

impl fmt::Display for Report {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for o in &self.outcomes {
            let result = match &o.result {
                Ok(()) => "ok".to_owned(),
                Err(why) => format!("FAILED: {why}"),
            };
            writeln!(
                f,
                "{} / {} / {}: {result}",
                o.producer, o.fixture, o.consumer
            )?;
        }
        Ok(())
    }
}

/// Checks every producer's every fixture with every consumer.
pub fn run(producers: &[&dyn Producer], consumers: &[&dyn Consumer]) -> Report {
    let mut outcomes = Vec::new();
    for producer in producers {
        for (fixture, data) in producer.fixtures() {
            for consumer in consumers {
                outcomes.push(Outcome {
                    producer: producer.name().to_owned(),
                    fixture: fixture.clone(),
                    consumer: consumer.name().to_owned(),
                    result: consumer.check(&fixture, &data),
                });
            }
        }
    }
    Report { outcomes }
}
