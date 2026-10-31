use core::convert::Infallible;
use core::future::Future;
use core::pin::{Pin, pin};
use core::task::{Context, Poll, Waker};
#[macro_use]
#[path = "support/storage.rs"]
mod storage;
#[path = "support/driver.rs"]
mod driver;
use driver::TestDriver;
use std::cell::{Cell, RefCell};
use std::rc::Rc;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::task::Wake;

#[derive(Default)]
struct WakeCount(AtomicUsize);

impl Wake for WakeCount {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
}

fn waker() -> (Arc<WakeCount>, Waker) {
    let count = Arc::new(WakeCount::default());
    (count.clone(), Waker::from(count))
}

fn poll<F: Future>(future: Pin<&mut F>, waker: &Waker) -> Poll<F::Output> {
    future.poll(&mut Context::from_waker(waker))
}

struct YieldOnce(bool);

impl Future for YieldOnce {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
        if self.0 {
            Poll::Ready(())
        } else {
            self.0 = true;
            cx.waker().wake_by_ref();
            Poll::Pending
        }
    }
}

#[test]
fn scoped_non_send_futures_can_hold_borrows_across_suspension() {
    let value = Cell::new(0);
    let local = Rc::new(Cell::new(0));
    let first = async {
        let bytes = [41_u8];
        let borrowed = &bytes[0];
        YieldOnce(false).await;
        value.set(u32::from(*borrowed));
        Ok::<_, Infallible>(())
    };
    let second = async {
        YieldOnce(false).await;
        local.set(local.get() + 1);
        Ok::<_, Infallible>(())
    };
    let mut executor = pin!(runtime!(Infallible; first, second));
    let (_, wake) = waker();

    assert_eq!(poll(executor.as_mut(), &wake), Poll::Pending);
    assert_eq!(poll(executor.as_mut(), &wake), Poll::Ready(Ok(())));
    assert_eq!(value.get(), 41);
    assert_eq!(local.get(), 1);
}

struct CountPolls<'a> {
    polls: &'a Cell<usize>,
    finish_at: usize,
}

impl Future for CountPolls<'_> {
    type Output = Result<(), Infallible>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let count = self.polls.get() + 1;
        self.polls.set(count);
        assert!(count <= self.finish_at, "completed task was polled again");
        if count == self.finish_at {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}

#[test]
fn every_live_task_progresses_and_completed_tasks_are_not_repolled() {
    let counts = [Cell::new(0), Cell::new(0), Cell::new(0)];
    let first = CountPolls {
        polls: &counts[0],
        finish_at: 1,
    };
    let second = CountPolls {
        polls: &counts[1],
        finish_at: 3,
    };
    let third = CountPolls {
        polls: &counts[2],
        finish_at: 2,
    };
    let mut executor = pin!(runtime!(Infallible; first, second, third));
    let (_, wake) = waker();

    assert_eq!(poll(executor.as_mut(), &wake), Poll::Pending);
    assert_eq!(counts.each_ref().map(Cell::get), [1, 1, 1]);
    assert_eq!(poll(executor.as_mut(), &wake), Poll::Pending);
    assert_eq!(counts.each_ref().map(Cell::get), [1, 2, 2]);
    assert_eq!(poll(executor.as_mut(), &wake), Poll::Ready(Ok(())));
    assert_eq!(poll(executor.as_mut(), &wake), Poll::Ready(Ok(())));
    assert_eq!(counts.each_ref().map(Cell::get), [1, 3, 2]);
}

struct RecordPoll<'a> {
    id: u8,
    order: &'a RefCell<Vec<u8>>,
}

impl Future for RecordPoll<'_> {
    type Output = Result<(), Infallible>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        self.order.borrow_mut().push(self.id);
        Poll::Pending
    }
}

#[test]
fn the_first_task_rotates_between_sweeps() {
    let order = RefCell::new(Vec::new());
    let first = RecordPoll {
        id: 0,
        order: &order,
    };
    let second = RecordPoll {
        id: 1,
        order: &order,
    };
    let third = RecordPoll {
        id: 2,
        order: &order,
    };
    let mut executor = pin!(runtime!(Infallible; first, second, third));
    let (_, wake) = waker();

    for _ in 0..3 {
        assert_eq!(poll(executor.as_mut(), &wake), Poll::Pending);
    }
    assert_eq!(*order.borrow(), [0, 1, 2, 1, 2, 0, 2, 0, 1]);
}

struct CaptureWaker<'a>(&'a RefCell<Option<Waker>>);

impl Future for CaptureWaker<'_> {
    type Output = Result<(), Infallible>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        *self.0.borrow_mut() = Some(cx.waker().clone());
        Poll::Pending
    }
}

#[test]
fn tasks_receive_the_current_parent_waker_without_a_stale_registration() {
    let captured = RefCell::new(None);
    let task = CaptureWaker(&captured);
    let mut executor = pin!(runtime!(Infallible; task));
    let (first_count, first) = waker();
    let (second_count, second) = waker();

    assert_eq!(poll(executor.as_mut(), &first), Poll::Pending);
    // will_wake permits false negatives, including when function pointers are
    // compared under Miri. Check the notification's actual destination.
    captured.borrow().as_ref().unwrap().wake_by_ref();
    assert_eq!(first_count.0.load(Ordering::SeqCst), 1);
    assert_eq!(second_count.0.load(Ordering::SeqCst), 0);
    assert_eq!(poll(executor.as_mut(), &second), Poll::Pending);
    captured.borrow().as_ref().unwrap().wake_by_ref();
    assert_eq!(first_count.0.load(Ordering::SeqCst), 1);
    assert_eq!(second_count.0.load(Ordering::SeqCst), 1);
}

#[test]
fn a_task_can_wake_the_driver_from_inside_poll() {
    let task = async {
        YieldOnce(false).await;
        Ok::<_, Infallible>(())
    };
    let mut executor = pin!(runtime!(Infallible; task));
    let (count, wake) = waker();

    assert_eq!(poll(executor.as_mut(), &wake), Poll::Pending);
    assert_eq!(count.0.load(Ordering::SeqCst), 1);
    assert_eq!(poll(executor.as_mut(), &wake), Poll::Ready(Ok(())));
}

#[test]
fn a_retained_waker_remains_usable_after_task_and_executor_cancellation() {
    let captured = RefCell::new(None);
    let (count, wake) = waker();
    {
        let task = CaptureWaker(&captured);
        let mut executor = pin!(runtime!(Infallible; task));
        assert_eq!(poll(executor.as_mut(), &wake), Poll::Pending);
    }

    captured.borrow().as_ref().unwrap().wake_by_ref();
    assert_eq!(count.0.load(Ordering::SeqCst), 1);
}

struct PinnedProbe<'a> {
    first_address: Cell<*const Self>,
    drops: &'a Cell<usize>,
    _pinned: core::marker::PhantomPinned,
}

impl Future for PinnedProbe<'_> {
    type Output = Result<(), Infallible>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.as_ref().get_ref();
        let address = core::ptr::from_ref(this);
        if this.first_address.get().is_null() {
            this.first_address.set(address);
        } else {
            assert_eq!(this.first_address.get(), address);
        }
        Poll::Pending
    }
}

impl Drop for PinnedProbe<'_> {
    fn drop(&mut self) {
        assert_eq!(self.first_address.get(), core::ptr::from_ref(self));
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn cancellation_drops_a_non_unpin_future_at_its_polled_address() {
    let drops = Cell::new(0);
    let task = PinnedProbe {
        first_address: Cell::new(core::ptr::null()),
        drops: &drops,
        _pinned: core::marker::PhantomPinned,
    };
    let (_, wake) = waker();
    {
        let mut executor = pin!(runtime!(Infallible; task));
        assert_eq!(poll(executor.as_mut(), &wake), Poll::Pending);
        assert_eq!(poll(executor.as_mut(), &wake), Poll::Pending);
    }
    assert_eq!(drops.get(), 1);
}

#[test]
fn either_race_winner_cancels_a_pinned_sibling_before_returning() {
    let (_, wake) = waker();
    for pinned_left in [true, false] {
        let drops = Cell::new(0);
        let pending = PinnedProbe {
            first_address: Cell::new(core::ptr::null()),
            drops: &drops,
            _pinned: core::marker::PhantomPinned,
        };
        let winner = async {
            YieldOnce(false).await;
            Ok::<(), Infallible>(())
        };
        if pinned_left {
            let mut race = pin!(hibana_runtime::race(pending, winner));
            assert_eq!(poll(race.as_mut(), &wake), Poll::Pending);
            assert_eq!(poll(race.as_mut(), &wake), Poll::Ready(Ok(())));
            assert_eq!(drops.get(), 1);
        } else {
            let mut race = pin!(hibana_runtime::race(winner, pending));
            assert_eq!(poll(race.as_mut(), &wake), Poll::Pending);
            assert_eq!(poll(race.as_mut(), &wake), Poll::Ready(Ok(())));
            assert_eq!(drops.get(), 1);
        }
        assert_eq!(drops.get(), 1);
    }
}

#[test]
fn dropping_a_pending_race_preserves_its_pinned_input_address() {
    let drops = Cell::new(0);
    let pending = PinnedProbe {
        first_address: Cell::new(core::ptr::null()),
        drops: &drops,
        _pinned: core::marker::PhantomPinned,
    };
    let (_, wake) = waker();
    {
        let mut race = pin!(hibana_runtime::race(
            pending,
            core::future::pending::<Result<(), Infallible>>()
        ));
        assert_eq!(poll(race.as_mut(), &wake), Poll::Pending);
        assert_eq!(poll(race.as_mut(), &wake), Poll::Pending);
        assert_eq!(drops.get(), 0);
    }
    assert_eq!(drops.get(), 1);
}

#[test]
fn races_rotate_poll_order_after_pending_and_stop_at_the_winner() {
    let (_, wake) = waker();
    for finish_at in [1, 2] {
        let counts = [Cell::new(0), Cell::new(0)];
        let left = CountPolls {
            polls: &counts[0],
            finish_at,
        };
        let right = CountPolls {
            polls: &counts[1],
            finish_at,
        };
        let mut race = pin!(hibana_runtime::race(left, right));
        if finish_at == 2 {
            assert_eq!(poll(race.as_mut(), &wake), Poll::Pending);
            assert_eq!(counts.each_ref().map(Cell::get), [1, 1]);
        }
        assert_eq!(poll(race.as_mut(), &wake), Poll::Ready(Ok(())));
        assert_eq!(
            counts.each_ref().map(Cell::get),
            if finish_at == 1 { [1, 0] } else { [1, 2] }
        );
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Failure;

struct Observed<'a> {
    polls: &'a Cell<usize>,
    drops: &'a Cell<usize>,
    result: Option<Result<(), Failure>>,
}

impl Future for Observed<'_> {
    type Output = Result<(), Failure>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        self.polls.set(self.polls.get() + 1);
        self.result.map_or(Poll::Pending, Poll::Ready)
    }
}

impl Drop for Observed<'_> {
    fn drop(&mut self) {
        self.drops.set(self.drops.get() + 1);
    }
}

#[test]
fn success_releases_the_future_before_the_other_tasks_finish() {
    let polls = [Cell::new(0), Cell::new(0)];
    let drops = [Cell::new(0), Cell::new(0)];
    let success = Observed {
        polls: &polls[0],
        drops: &drops[0],
        result: Some(Ok(())),
    };
    let pending = Observed {
        polls: &polls[1],
        drops: &drops[1],
        result: None,
    };
    {
        let mut executor = pin!(runtime!(Failure; success, pending));
        let (_, wake) = waker();
        assert_eq!(poll(executor.as_mut(), &wake), Poll::Pending);
        assert_eq!(drops.each_ref().map(Cell::get), [1, 0]);
    }
    assert_eq!(drops.each_ref().map(Cell::get), [1, 1]);
}

#[test]
fn failure_cancels_all_pending_futures_before_returning() {
    let polls = [Cell::new(0), Cell::new(0), Cell::new(0)];
    let drops = [Cell::new(0), Cell::new(0), Cell::new(0)];
    let first = Observed {
        polls: &polls[0],
        drops: &drops[0],
        result: None,
    };
    let failed = Observed {
        polls: &polls[1],
        drops: &drops[1],
        result: Some(Err(Failure)),
    };
    let last = Observed {
        polls: &polls[2],
        drops: &drops[2],
        result: None,
    };
    {
        let mut executor = pin!(runtime!(Failure; first, failed, last));
        let (_, wake) = waker();
        assert_eq!(poll(executor.as_mut(), &wake), Poll::Ready(Err(Failure)));
        // The completed executor is still alive: cancellation cannot be deferred
        // until its owner drops it.
        assert_eq!(drops.each_ref().map(Cell::get), [1, 1, 1]);
        assert_eq!(polls.each_ref().map(Cell::get), [1, 1, 0]);
    }
    assert_eq!(drops.each_ref().map(Cell::get), [1, 1, 1]);
}

#[test]
fn dropping_an_executor_cancels_even_unpolled_tasks_exactly_once() {
    let polls = Cell::new(0);
    let drops = Cell::new(0);
    {
        let task = Observed {
            polls: &polls,
            drops: &drops,
            result: None,
        };
        {
            let _executor = runtime!(Failure; task);
        }
        assert_eq!(polls.get(), 0);
        assert_eq!(drops.get(), 1);
    }
    assert_eq!(drops.get(), 1);
}

#[test]
fn run_pins_owned_scoped_futures_and_releases_borrows_without_caller_pinning() {
    async fn write(value: &mut u8) -> Result<(), Infallible> {
        let bytes = [41_u8];
        let borrowed = &bytes[0];
        YieldOnce(false).await;
        *value = *borrowed;
        Ok(())
    }

    let mut value = 0;
    let runtime = runtime!(Infallible; write(&mut value));
    // Startup ownership can move through an ordinary function. Address
    // stability begins inside run, rather than at a caller-written pin site.
    let runtime = core::convert::identity(runtime);
    let token = Arc::new(WakeToken::default());
    let wake = Waker::from(token.clone());
    runtime
        .run(TestDriver(&wake, || {
            assert!(token.0.swap(false, Ordering::SeqCst))
        }))
        .unwrap();
    assert_eq!(value, 41);
}

#[test]
fn runtimes_can_be_awaited_without_caller_pinning() {
    let value = Cell::new(0);
    let token = Arc::new(WakeToken::default());
    let wake = Waker::from(token.clone());
    runtime!(Infallible; async {
        runtime!(Infallible; async {
                YieldOnce(false).await;
                value.set(42);
                Ok::<_, Infallible>(())
            })
            .await
    })
    .run(TestDriver(&wake, || {
        assert!(token.0.swap(false, Ordering::SeqCst))
    }))
    .unwrap();
    assert_eq!(value.get(), 42);
}

#[derive(Default)]
struct WakeToken(AtomicBool);

impl Wake for WakeToken {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }

    fn wake_by_ref(self: &Arc<Self>) {
        self.0.store(true, Ordering::SeqCst);
    }
}

#[test]
fn run_works_with_a_driver_that_retains_a_wake_before_wait() {
    let token = Arc::new(WakeToken::default());
    let wake = Waker::from(token.clone());
    let waits = Cell::new(0);
    let task = async {
        YieldOnce(false).await;
        Ok::<_, Infallible>(())
    };
    let executor = runtime!(Infallible; task);

    assert_eq!(
        executor.run(TestDriver(&wake, || {
            assert!(token.0.swap(false, Ordering::SeqCst), "wake token was lost");
            waits.set(waits.get() + 1);
        })),
        Ok(())
    );
    assert_eq!(waits.get(), 1);
}

#[test]
fn a_cross_thread_wake_drives_a_non_send_local_future() {
    struct Unpark(std::thread::Thread);

    impl Wake for Unpark {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }

        fn wake_by_ref(self: &Arc<Self>) {
            self.0.unpark();
        }
    }

    struct ExternalReady<'a> {
        ready: &'a AtomicBool,
        registration: Option<std::sync::mpsc::SyncSender<Waker>>,
    }

    impl Future for ExternalReady<'_> {
        type Output = ();

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<()> {
            if let Some(registration) = self.registration.take() {
                registration.send(cx.waker().clone()).unwrap();
                // Suspend even if the producer has already woken us. The
                // platform's unpark token must survive until the driver waits.
                return Poll::Pending;
            }
            if self.ready.load(Ordering::Acquire) {
                Poll::Ready(())
            } else {
                Poll::Pending
            }
        }
    }

    let ready = Arc::new(AtomicBool::new(false));
    let producer_ready = ready.clone();
    let (registration, registered) = std::sync::mpsc::sync_channel::<Waker>(1);
    let producer = std::thread::spawn(move || {
        let wake = registered.recv().unwrap();
        producer_ready.store(true, Ordering::Release);
        wake.wake();
    });
    let local = Rc::new(Cell::new(0));
    let task = async {
        ExternalReady {
            ready: &ready,
            registration: Some(registration),
        }
        .await;
        local.set(local.get() + 1);
        Ok::<_, Infallible>(())
    };
    let executor = runtime!(Infallible; task);
    let wake = Waker::from(Arc::new(Unpark(std::thread::current())));

    assert_eq!(executor.run(TestDriver(&wake, std::thread::park)), Ok(()));
    producer.join().unwrap();
    assert_eq!(local.get(), 1);
}

#[test]
fn empty_executor_completes_without_waiting() {
    let (_, wake) = waker();
    let mut executor = pin!(runtime!(Infallible; ));
    assert_eq!(poll(executor.as_mut(), &wake), Poll::Ready(Ok(())));
    assert_eq!(poll(executor.as_mut(), &wake), Poll::Ready(Ok(())));
    assert_eq!(
        runtime!(Infallible; ).run(TestDriver(&wake, || {
            panic!("an empty executor must not wait")
        })),
        Ok(())
    );
}
