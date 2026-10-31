use core::cell::Cell;
use core::convert::Infallible;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
use hibana_runtime::__private::Runtime;
#[path = "support/driver.rs"]
mod driver;
use driver::TestDriver;
use std::alloc::{GlobalAlloc, Layout, System};

thread_local! {
    static MEASURING: Cell<bool> = const { Cell::new(false) };
    static ALLOCATIONS: Cell<usize> = const { Cell::new(0) };
}

struct CountingAllocator;

fn record_allocation() {
    let _ = MEASURING.try_with(|measuring| {
        if measuring.get() {
            let _ = ALLOCATIONS.try_with(|count| count.set(count.get() + 1));
        }
    });
}

// Count only this test thread's measured section, so allocations made by the
// test harness and other threads cannot hide or create a runtime allocation.
// SAFETY: Every operation preserves the caller's allocation contract by
// forwarding the same pointer, layout, and size to the System allocator.
unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        // SAFETY: The caller guarantees the GlobalAlloc layout contract.
        unsafe { System.alloc(layout) }
    }

    unsafe fn alloc_zeroed(&self, layout: Layout) -> *mut u8 {
        record_allocation();
        // SAFETY: The caller guarantees the GlobalAlloc layout contract.
        unsafe { System.alloc_zeroed(layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        record_allocation();
        // SAFETY: Forward the caller's live allocation and its valid layout.
        unsafe { System.realloc(ptr, layout, size) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        // SAFETY: The caller supplies a live System allocation and its layout.
        unsafe { System.dealloc(ptr, layout) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn allocations_during(f: impl FnOnce()) -> usize {
    ALLOCATIONS.with(|count| count.set(0));
    MEASURING.with(|measuring| measuring.set(true));
    struct StopMeasuring;
    impl Drop for StopMeasuring {
        fn drop(&mut self) {
            MEASURING.with(|measuring| measuring.set(false));
        }
    }
    let guard = StopMeasuring;
    f();
    drop(guard);
    ALLOCATIONS.with(Cell::get)
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

struct PendingDrop<'a>(&'a Cell<usize>);

impl Future for PendingDrop<'_> {
    type Output = Result<(), ()>;

    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        Poll::Pending
    }
}

impl Drop for PendingDrop<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() + 1);
    }
}

#[test]
fn construction_registration_run_and_cancellation_allocate_nothing() {
    let completed = Cell::new(0);
    let waits = Cell::new(0);
    let cancelled = Cell::new(0);
    let allocations = allocations_during(|| {
        let first = async {
            YieldOnce(false).await;
            completed.set(completed.get() + 1);
            Ok::<_, Infallible>(())
        };
        let second = async {
            YieldOnce(false).await;
            completed.set(completed.get() + 1);
            Ok::<_, Infallible>(())
        };
        let executor = Runtime::new().localside(first).localside(second);
        let result = executor.run(TestDriver(Waker::noop(), || waits.set(waits.get() + 1)));
        assert_eq!(result, Ok(()));

        let result = Runtime::new()
            .localside(PendingDrop(&cancelled))
            .localside(async { Err(()) })
            .run(TestDriver(Waker::noop(), || unreachable!()));
        assert_eq!(result, Err(()));
    });

    assert_eq!(allocations, 0);
    assert_eq!(completed.get(), 2);
    assert_eq!(waits.get(), 1);
    assert_eq!(cancelled.get(), 1);
}

#[test]
fn automatic_scopes_entry_and_error_cancellation_allocate_nothing() {
    async fn effect(completed: &Cell<u32>) -> Result<(), ()> {
        YieldOnce(false).await;
        completed.set(completed.get() + 1);
        Ok(())
    }
    #[hibana_runtime::main(TestDriver(Waker::noop(), || {}))]
    async fn program(completed: &Cell<u32>, cancelled: &Cell<usize>) -> Result<(), ()> {
        async {
            #[localside]
            effect(completed);
            #[localside]
            effect(completed);
            Ok(())
        }
        .await?;
        #[localside]
        PendingDrop(cancelled);
        Err(())
    }
    let completed = Cell::new(0);
    let cancelled = Cell::new(0);
    assert_eq!(
        allocations_during(|| assert_eq!(program(&completed, &cancelled), Err(()))),
        0
    );
    assert_eq!(completed.get(), 2);
    assert_eq!(cancelled.get(), 1);
}

#[test]
fn racing_releases_the_pending_future_before_the_completed_race_is_dropped() {
    let cancelled = Cell::new(0);
    assert_eq!(
        allocations_during(|| {
            let result = hibana_runtime::race(PendingDrop(&cancelled), async {
                hibana_runtime::yield_now().await;
                Ok::<(), ()>(())
            });
            let mut result = core::pin::pin!(result);
            let mut context = Context::from_waker(Waker::noop());
            assert_eq!(result.as_mut().poll(&mut context), Poll::Pending);
            assert_eq!(cancelled.get(), 0);
            assert_eq!(result.as_mut().poll(&mut context), Poll::Ready(Ok(())));
            assert_eq!(cancelled.get(), 1);
        }),
        0
    );
}

#[test]
fn racing_stores_each_input_buffer_once() {
    fn buffered<const N: usize>() -> impl Future<Output = usize> {
        let bytes = [7u8; N];
        async move {
            hibana_runtime::yield_now().await;
            core::hint::black_box(bytes).len()
        }
    }
    let left = buffered::<4096>();
    let right = buffered::<2048>();
    let inputs = size_of_val(&left) + size_of_val(&right);
    let alignment = align_of_val(&left).max(align_of_val(&right));
    let race = hibana_runtime::race(left, right);
    // Two optional input slots and the polling order need only discriminants
    // and alignment padding beyond the two buffers themselves.
    let budget = inputs + 2 * alignment + size_of::<usize>();
    assert!(
        size_of_val(&race) <= budget,
        "race uses {} bytes for {inputs} bytes of input; budget {budget}",
        size_of_val(&race)
    );
}

#[test]
fn concurrent_result_bindings_allocate_nothing() {
    #[hibana_runtime::main(TestDriver(Waker::noop(), || {}))]
    async fn program() -> Result<(u8, u16), ()> {
        #[localside]
        let first = async {
            hibana_runtime::yield_now().await;
            Ok::<_, ()>(7u8)
        };
        #[localside]
        let second = async {
            hibana_runtime::yield_now().await;
            Ok::<_, ()>(8u16)
        };
        Ok((first, second))
    }
    assert_eq!(allocations_during(|| assert_eq!(program(), Ok((7, 8)))), 0);
}

#[cfg(feature = "host")]
#[test]
fn host_entry_waking_and_cancellation_allocate_nothing() {
    #[hibana_runtime::main]
    async fn program(completed: &Cell<u32>, cancelled: &Cell<usize>) -> Result<(), ()> {
        async {
            async {
                hibana_runtime::yield_now().await;
                completed.set(completed.get() + 1);
            };
            async {
                hibana_runtime::yield_now().await;
                completed.set(completed.get() + 1);
            };
            Ok(())
        }
        .await?;
        #[localside]
        PendingDrop(cancelled);
        hibana_runtime::yield_now().await;
        Err(())
    }

    let completed = Cell::new(0);
    let cancelled = Cell::new(0);
    assert_eq!(
        allocations_during(|| assert_eq!(program(&completed, &cancelled), Err(()))),
        0
    );
    assert_eq!(completed.get(), 2);
    assert_eq!(cancelled.get(), 1);
}
