//! Different execution places build their own non-Send futures and kernel state.
use core::{cell::Cell, convert::Infallible, task::Waker};
use hibana_runtime::Driver;
use std::{
    rc::Rc,
    sync::{
        Arc,
        atomic::{AtomicU8, Ordering},
    },
    thread,
    time::{Duration, Instant},
};

struct Ready;
impl Driver for Ready {
    fn waker(&self) -> Waker {
        Waker::noop().clone()
    }
    fn wait(&mut self) {
        panic!("the computation completes in its first poll");
    }
}

#[test]
fn independent_images_poll_non_send_futures_on_two_threads_at_the_same_time() {
    #[hibana_runtime::main(Ready)]
    async fn image(local: Rc<Cell<u32>>, arrived: &AtomicU8, bit: u8) -> Result<(), Infallible> {
        async {
            // This handshake is test instrumentation. One single polling thread
            // cannot pass it: both images must enter their own Future::poll.
            arrived.fetch_or(bit, Ordering::AcqRel);
            let deadline = Instant::now() + Duration::from_secs(5);
            while arrived.load(Ordering::Acquire) != 3 {
                assert!(Instant::now() < deadline, "images were serialized");
                thread::yield_now();
            }
            local.set(local.get() + 1);
        };
        Ok(())
    }
    let arrived = Arc::new(AtomicU8::new(0));
    thread::scope(|scope| {
        let handles: Vec<_> = [1, 2]
            .into_iter()
            .map(|bit| {
                let arrived = arrived.clone();
                scope.spawn(move || {
                    let local = Rc::new(Cell::new(0));
                    image(local.clone(), &arrived, bit).unwrap();
                    assert_eq!(local.get(), 1);
                    thread::current().id()
                })
            })
            .collect();
        let mut ids = handles.into_iter().map(|handle| handle.join().unwrap());
        assert_ne!(ids.next().unwrap(), ids.next().unwrap());
    });
}
