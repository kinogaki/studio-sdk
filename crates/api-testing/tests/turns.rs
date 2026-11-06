//! A test alone on the GPU starts only after the shared turns before it
//! end, and its turn ends with its thread.
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};

#[test]
fn alone_waits_for_the_shared_turns_and_ends_with_its_thread() {
    let released = Arc::new(AtomicBool::new(false));
    let (ready, shared) = mpsc::channel();
    let (go, release) = mpsc::channel::<()>();
    let sharer = {
        let released = released.clone();
        std::thread::spawn(move || {
            kinogaki_studio_api_testing::engine::share();
            // A second call holds the same turn.
            kinogaki_studio_api_testing::engine::share();
            ready.send(()).unwrap();
            release.recv().unwrap();
            released.store(true, Ordering::SeqCst);
        })
    };
    shared.recv().unwrap();
    let alone = {
        let released = released.clone();
        std::thread::spawn(move || {
            kinogaki_studio_api_testing::engine::alone();
            assert!(
                released.load(Ordering::SeqCst),
                "alone beside a shared turn"
            );
        })
    };
    go.send(()).unwrap();
    sharer.join().unwrap();
    alone.join().unwrap();
    // Its thread ended: shared turns go on.
    std::thread::spawn(kinogaki_studio_api_testing::engine::share)
        .join()
        .unwrap();
}
