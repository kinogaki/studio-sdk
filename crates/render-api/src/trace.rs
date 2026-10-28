//! A latency trace for measuring an engine in its host: with
//! `KINOGAKI_RENDER_TRACE=FILE` set, [`mark`] writes one line a call
//! (`microseconds stage values`), the time on the wall clock so another
//! process's log (an input driver's) lines up with it. Engines mark what
//! they receive, submit and finish; hosts mark their frames and what they
//! show. Unset, a mark is one check of a value read once.
use std::io::Write;
use std::sync::{Mutex, OnceLock};

type Log = Option<Mutex<std::io::BufWriter<std::fs::File>>>;

static LOG: OnceLock<Log> = OnceLock::new();

fn log() -> &'static Log {
    LOG.get_or_init(|| {
        let path = std::env::var_os("KINOGAKI_RENDER_TRACE")?;
        let file = std::fs::File::create(path).ok()?;
        Some(Mutex::new(std::io::BufWriter::new(file)))
    })
}

/// Whether the trace is on (to skip gathering values for [`mark`]).
pub fn on() -> bool {
    log().is_some()
}

/// One line: the wall-clock time in microseconds, `stage`, `values`.
pub fn mark(stage: &str, values: std::fmt::Arguments<'_>) {
    let Some(log) = log() else {
        return;
    };
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_micros());
    let mut w = log.lock().unwrap_or_else(|e| e.into_inner());
    // A trace that cannot be written is no reason to stop.
    let _ = writeln!(w, "{at} {stage} {values}");
    let _ = w.flush();
}
