//! Driver for a closed in-process simulation with no external event producer.
use core::{
    ptr,
    sync::atomic::{AtomicBool, Ordering},
    task::{RawWaker, RawWakerVTable, Waker},
};
use hibana_runtime::Driver;

static READY: AtomicBool = AtomicBool::new(false);

pub struct WakeLatch;

impl WakeLatch {
    pub fn new() -> Self {
        READY.store(false, Ordering::Release);
        Self
    }
}

impl Driver for WakeLatch {
    fn waker(&self) -> Waker {
        // SAFETY: the callbacks use only the static atomic latch and ignore
        // the data pointer. Every clone is valid indefinitely on any thread.
        unsafe { Waker::from_raw(raw_waker()) }
    }

    fn wait(&mut self) {
        assert!(
            READY.swap(false, Ordering::AcqRel),
            "the closed choreography stalled without a wake"
        );
    }
}

fn raw_waker() -> RawWaker {
    RawWaker::new(ptr::null(), &VTABLE)
}
fn clone(_: *const ()) -> RawWaker {
    raw_waker()
}
fn wake(_: *const ()) {
    READY.store(true, Ordering::Release);
}
fn drop_waker(_: *const ()) {}
static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake, drop_waker);
