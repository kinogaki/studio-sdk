//! GPU tests' turns on the one GPU they share.
//!
//! Test harnesses run tests in parallel, and several test processes (two
//! checkouts' checks, a library's and Studio's) can run at once. Most GPU
//! tests only need the GPU to work; a test that times GPU work against a
//! budget (a pass against the display interval, a restart against a pass in
//! flight) measures whatever else is queued on the GPU instead. Beside the
//! Render-view tests' path tracing, a 112 × 63 navigating pass took 50–200
//! ms of GPU time where alone it takes 1.5; beside another process's
//! renders, a restart waited 31 ms behind a 60 ms pass it cancels in 9.
//!
//! So a GPU test takes a turn first ([`device`](super::device) takes a
//! shared one). [`share`] holds a shared turn until the test's thread ends
//! (after its locals, devices and engines among them, are dropped). [`alone`] waits for every turn to end, holds the GPU by
//! itself, and tests asking for a turn meanwhile wait for it; in this
//! process, tests waiting to be alone go before new shared turns. Between
//! processes a turn is an advisory lock (shared or exclusive) on one file,
//! `~/.kinogaki/gpu-turns.lock`, so every process's tests take turns,
//! whichever checkout or session runs them.
use std::cell::RefCell;
use std::fs::File;
use std::path::Path;
use std::sync::{Condvar, Mutex, MutexGuard};

/// This process's turns.
struct Turns {
    /// Tests holding a shared turn.
    sharing: usize,
    /// A test holds the GPU alone.
    alone: bool,
    /// Tests waiting to hold it alone: no new shared turns meanwhile.
    waiting: usize,
}

static TURNS: Mutex<Turns> = Mutex::new(Turns {
    sharing: 0,
    alone: false,
    waiting: 0,
});
static CHANGED: Condvar = Condvar::new();

fn turns() -> MutexGuard<'static, Turns> {
    // Nothing panics while holding it; a panicking test only ends its turn.
    TURNS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

fn wait(turns: MutexGuard<'static, Turns>) -> MutexGuard<'static, Turns> {
    CHANGED
        .wait(turns)
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// The lock file every process's turns share.
fn lock_file() -> File {
    let home = std::env::var_os("HOME").expect("HOME: the GPU turns' lock is in ~/.kinogaki");
    let folder = Path::new(&home).join(".kinogaki");
    std::fs::create_dir_all(&folder)
        .unwrap_or_else(|e| panic!("the GPU turns' folder {}: {e}", folder.display()));
    let path = folder.join("gpu-turns.lock");
    File::options()
        .create(true)
        .truncate(false)
        .write(true)
        .open(&path)
        .unwrap_or_else(|e| panic!("the GPU turns' lock file {}: {e}", path.display()))
}

/// A test thread's turn, given back when the thread ends.
struct Turn {
    alone: bool,
    /// Its lock on the file (released when closed).
    _file: File,
}
impl Drop for Turn {
    fn drop(&mut self) {
        let mut turns = turns();
        if self.alone {
            turns.alone = false;
        } else {
            turns.sharing -= 1;
        }
        CHANGED.notify_all();
    }
}

thread_local! {
    static HELD: RefCell<Option<Turn>> = const { RefCell::new(None) };
}

/// Hold a shared turn on the GPU for the rest of this test (its thread);
/// nothing when it holds one already, or the GPU alone.
pub fn share() {
    HELD.with(|held| {
        let mut held = held.borrow_mut();
        if held.is_some() {
            return;
        }
        {
            let mut turns = turns();
            while turns.alone || turns.waiting > 0 {
                turns = wait(turns);
            }
            turns.sharing += 1;
        }
        let file = lock_file();
        file.lock_shared()
            .expect("a shared lock on the GPU turns' file");
        *held = Some(Turn {
            alone: false,
            _file: file,
        });
    });
}

/// Hold the GPU alone for the rest of this test (its thread): a test whose
/// assertions time GPU work calls it before making a device.
pub fn alone() {
    HELD.with(|held| {
        let mut held = held.borrow_mut();
        assert!(
            held.is_none(),
            "hold the GPU alone before taking a shared turn"
        );
        {
            let mut turns = turns();
            turns.waiting += 1;
            while turns.alone || turns.sharing > 0 {
                turns = wait(turns);
            }
            turns.waiting -= 1;
            turns.alone = true;
        }
        let file = lock_file();
        file.lock().expect("the GPU turns' file locked");
        *held = Some(Turn {
            alone: true,
            _file: file,
        });
    });
}
