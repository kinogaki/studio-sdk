//! What an engine's GPU tests share: this machine's device, offscreen, the
//! tests' turns on it, and the engine's wakes counted with a way to wait on
//! them.
use kinogaki_render_api::display::wgpu;
use kinogaki_render_api::{Pending, Wake};
use std::future::Future;
use std::sync::{Arc, Condvar, Mutex};
use std::task::{Context, Poll, Waker};
use std::time::{Duration, Instant};

mod turns;
pub use turns::{alone, share};

/// `future`'s output, polled to completion (wgpu's requests resolve without
/// a runtime).
pub fn ready<F: Future>(future: F) -> F::Output {
    let mut future = std::pin::pin!(future);
    let mut cx = Context::from_waker(Waker::noop());
    loop {
        if let Poll::Ready(v) = future.as_mut().poll(&mut cx) {
            return v;
        }
        std::thread::yield_now();
    }
}

/// This machine's GPU, offscreen: its high-performance adapter's device,
/// with the test's turn on it taken ([`share`]; nothing more after
/// [`alone`]).
///
/// # Panics
/// With no adapter or device.
pub fn device() -> (wgpu::Device, wgpu::Queue) {
    share();
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor::new_without_display_handle());
    let options = wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    };
    let adapter = ready(instance.request_adapter(&options)).expect("a GPU adapter");
    ready(adapter.request_device(&wgpu::DeviceDescriptor::default())).expect("a device")
}

/// An engine's wakes counted, and a way to wait for them.
#[derive(Clone, Default)]
pub struct Waiter {
    state: Arc<(Mutex<u64>, Condvar)>,
}

impl Waiter {
    /// The engine's wake.
    pub fn wake(&self) -> Wake {
        let state = self.state.clone();
        Arc::new(move || {
            *state.0.lock().unwrap_or_else(|e| e.into_inner()) += 1;
            state.1.notify_all();
        })
    }

    /// Wakes so far.
    pub fn count(&self) -> u64 {
        *self.state.0.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Waits (at most `limit`) until `done` holds, asking after each wake.
    ///
    /// # Panics
    /// When `limit` passes first.
    pub fn until(&self, limit: Duration, mut done: impl FnMut() -> bool) {
        let start = Instant::now();
        let mut guard = self.state.0.lock().unwrap_or_else(|e| e.into_inner());
        while !done() {
            let left = limit.saturating_sub(start.elapsed());
            assert!(!left.is_zero(), "timed out waiting for the engine");
            let waited = self.state.1.wait_timeout(guard, left);
            guard = waited.unwrap_or_else(|e| e.into_inner()).0;
        }
    }

    /// The value of `pending` once it arrives (within `limit`).
    ///
    /// # Panics
    /// When it fails, or `limit` passes first.
    pub fn take<T>(&self, pending: &Pending<T>, limit: Duration) -> T {
        let mut value = None;
        self.until(limit, || {
            value = pending.take();
            value.is_some()
        });
        value.expect("a result").expect("a delivered result")
    }
}
