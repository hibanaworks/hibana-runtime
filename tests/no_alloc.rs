use core::cell::Cell;
use core::convert::Infallible;
use core::future::Future;
use core::pin::Pin;
use core::task::{Context, Poll, Waker};
#[macro_use]
#[path = "support/storage.rs"]
mod storage;
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
        let executor = runtime!(Infallible; first, second);
        let result = executor.run(TestDriver(Waker::noop(), || waits.set(waits.get() + 1)));
        assert_eq!(result, Ok(()));

        let result = runtime!(_; PendingDrop(&cancelled), async { Err::<(), ()>(()) })
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
fn grouped_results_and_their_cancellation_allocate_nothing() {
    #[hibana_runtime::main(TestDriver(Waker::noop(), || {}))]
    async fn program(cancelled: &Cell<usize>) -> Result<(), ()> {
        #[localside]
        let values = (
            async {
                YieldOnce(false).await;
                Ok(7_u32)
            },
            async {
                YieldOnce(false).await;
                Ok(true)
            },
        );
        assert_eq!(values, (7, true));
        #[localside]
        let _values = (PendingDrop(cancelled), async {
            YieldOnce(false).await;
            Err::<(), _>(())
        });
        panic!("cancelled result group must not continue")
    }
    let cancelled = Cell::new(0);
    assert_eq!(
        allocations_during(|| assert_eq!(program(&cancelled), Err(()))),
        0
    );
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
fn localside_declarations_store_their_future_buffer_once() {
    struct Buffered([u8; 4096]);
    impl Future for Buffered {
        type Output = Result<u8, Infallible>;
        fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
            Poll::Ready(Ok(core::hint::black_box(&self.0)[0]))
        }
    }
    #[hibana_runtime::localside]
    async fn unbound() -> Result<(), Infallible> {
        #[localside]
        Buffered([7; 4096]);
        Ok(())
    }
    #[hibana_runtime::localside]
    async fn bound() -> Result<u8, Infallible> {
        #[localside]
        let value = Buffered([7; 4096]);
        Ok(value)
    }
    #[hibana_runtime::localside]
    async fn child_and_result() -> Result<u8, Infallible> {
        #[localside]
        Buffered([8; 4096]);
        #[localside]
        let value = Buffered([7; 4096]);
        Ok(value)
    }
    let budget = size_of::<Buffered>() + 16 * size_of::<usize>();
    let unbound = unbound();
    let bound = bound();
    assert!(
        size_of_val(&unbound) <= budget,
        "unbound localside uses {} bytes; budget {budget}",
        size_of_val(&unbound)
    );
    assert!(
        size_of_val(&bound) <= budget,
        "bound localside uses {} bytes; budget {budget}",
        size_of_val(&bound)
    );
    let mixed = child_and_result();
    assert!(
        size_of_val(&mixed) <= 2 * budget,
        "child and result use {} bytes; budget {}",
        size_of_val(&mixed),
        2 * budget,
    );
}

#[test]
fn an_explicit_result_group_allocates_nothing() {
    #[hibana_runtime::main(TestDriver(Waker::noop(), || {}))]
    async fn program() -> Result<(u8, u16), ()> {
        #[localside]
        let (first, second) = (
            async {
                hibana_runtime::yield_now().await;
                Ok::<_, ()>(7u8)
            },
            async {
                hibana_runtime::yield_now().await;
                Ok::<_, ()>(8u16)
            },
        );
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

#[cfg(feature = "host")]
#[test]
fn host_readiness_timers_and_timeout_cancellation_allocate_nothing() {
    use hibana_runtime::host::{Io, deadline, sleep};
    use std::{io, os::unix::net::UnixStream, time::Duration};

    #[hibana_runtime::main]
    async fn program(mut reader: Io, mut writer: Io) -> io::Result<()> {
        let mut bytes = [0; 1];
        let error =
            hibana_runtime::race(reader.read(&mut bytes), deadline(Duration::from_millis(1)))
                .await
                .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        async {
            assert_eq!(reader.read(&mut bytes).await?, 1);
            assert_eq!(bytes[0], 8);
        };
        async {
            sleep(Duration::from_millis(1)).await?;
            writer.write_all(&[8]).await?;
        };
        Ok(())
    }

    let (reader, writer) = UnixStream::pair().unwrap();
    let reader = Io::new(reader.into()).unwrap();
    let writer = Io::new(writer.into()).unwrap();
    assert_eq!(allocations_during(|| program(reader, writer).unwrap()), 0);
}

#[cfg(feature = "host")]
#[test]
fn waiting_for_an_owned_child_allocates_nothing() {
    #[hibana_runtime::main]
    async fn run(child: &mut hibana_runtime::host::Process) -> std::io::Result<()> {
        assert!(child.wait().await?.success());
        Ok(())
    }
    let mut child =
        hibana_runtime::host::Process::spawn(std::process::Command::new("/bin/sleep").arg("0.01"))
            .unwrap();
    assert_eq!(allocations_during(|| run(&mut child).unwrap()), 0);
}

#[cfg(feature = "host")]
#[test]
fn external_notification_construction_cloning_and_waiting_allocate_nothing() {
    #[hibana_runtime::main]
    async fn run() -> std::io::Result<()> {
        let (notify, mut input) = hibana_runtime::host::notification()?;
        let duplicate = notify.try_clone()?;
        async {
            let mut bytes = [0; 8];
            assert_eq!(input.read(&mut bytes).await?, 1);
            assert_eq!(input.read(&mut bytes).await?, 0);
        };
        async move {
            hibana_runtime::yield_now().await;
            duplicate.notify()?;
            drop(duplicate);
            drop(notify);
        };
        Ok(())
    }
    assert_eq!(allocations_during(|| run().unwrap()), 0);
}

#[test]
fn array_declarations_use_caller_storage_without_allocation() {
    #[hibana_runtime::main(TestDriver(Waker::noop(), || {}))]
    async fn image(outputs: [&Cell<u8>; 7]) -> Result<(), Infallible> {
        #[localside]
        for output in outputs {
            YieldOnce(false).await;
            output.set(1);
        }
        Ok(())
    }
    let outputs = core::array::from_fn(|_| Cell::new(0));
    assert_eq!(allocations_during(|| image(outputs.each_ref()).unwrap()), 0);
    assert_eq!(outputs.map(Cell::into_inner), [1; 7]);
}
