//! A local executor for independently written Hibana localsides.
//!
//! Register tasks once at startup. Each poll drives one fair sweep of live
//! tasks. Hibana alone owns protocol order, branches, delegation, and joins;
//! this crate owns only polling and task lifetime.
//!
//! Futures live inline in caller-owned [`Task`] storage. They can borrow local
//! data and need not be `Send` or `'static`. There is no allocator, task pool,
//! global scheduler, or platform dependency.
//!
//! ```
//! use core::{convert::Infallible, pin::pin};
//! use hibana_runtime::{Executor, Task};
//!
//! let mut left_value = 0;
//! let mut right_value = 0;
//! let mut left = pin!(Task::new(async {
//!     left_value = 7;
//!     Ok::<(), Infallible>(())
//! }));
//! let mut right = pin!(Task::new(async {
//!     right_value = 8;
//!     Ok::<(), Infallible>(())
//! }));
//! let execution = Executor::new([
//!     left.as_mut().register(),
//!     right.as_mut().register(),
//! ]);
//! // A host or board supplies a Waker and a wake-preserving wait operation.
//! // These tasks finish in their first poll, so no wait is needed here.
//! execution.run(core::task::Waker::noop(), || unreachable!()).unwrap();
//! ```
//!
//! Dropping the executor, or the first task error, drops all remaining task
//! futures in place before control returns. Task storage can then be dropped
//! normally. Cancellation does not roll back any accepted Hibana operation.
#![no_std]

use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};

/// Inline storage for one localside future, including its cancellation boundary.
///
/// Construct the task, pin it with [`core::pin::pin!`], then [`register`](Self::register)
/// it at startup. A registration exclusively borrows its task storage.
pub struct Task<F> {
    future: Option<F>,
}

impl<F> Task<F> {
    /// Store a future without allocating or polling it.
    pub const fn new(future: F) -> Self {
        Self {
            future: Some(future),
        }
    }

    fn future(self: Pin<&mut Self>) -> Pin<&mut Option<F>> {
        // SAFETY: `future` is structurally pinned. It is private, no operation
        // moves it out, and Task has no Drop implementation that could move F.
        // Cancellation uses Pin::set, which drops F before replacing it.
        unsafe { self.map_unchecked_mut(|task| &mut task.future) }
    }
}

impl<F, E> Task<F>
where
    F: Future<Output = Result<(), E>>,
{
    /// Register this pinned task. Dropping the registration cancels the future.
    ///
    /// Finished or cancelled task storage stays empty; registering it again
    /// completes immediately. Create a new Task to execute another future.
    ///
    /// The same task cannot be registered twice concurrently:
    ///
    /// ```compile_fail
    /// use core::pin::pin;
    /// use hibana_runtime::Task;
    /// let mut task = pin!(Task::new(async { Ok::<(), ()>(()) }));
    /// let first = task.as_mut().register();
    /// let second = task.as_mut().register();
    /// drop((first, second));
    /// ```
    pub fn register(self: Pin<&mut Self>) -> Registration<'_, E> {
        Registration { task: self }
    }
}

trait Runnable<E> {
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), E>>;
    fn cancel(self: Pin<&mut Self>);
}

impl<F, E> Runnable<E> for Task<F>
where
    F: Future<Output = Result<(), E>>,
{
    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Result<(), E>> {
        let mut slot = self.future();
        let result = match slot.as_mut().as_pin_mut() {
            Some(future) => future.poll(cx),
            None => return Poll::Ready(Ok(())),
        };
        if result.is_ready() {
            slot.set(None);
        }
        result
    }

    fn cancel(self: Pin<&mut Self>) {
        self.future().set(None);
    }
}

/// An exclusive task registration, with its concrete future type erased.
///
/// Created by [`Task::register`]. It owns cancellation, while the future's bytes
/// stay in the caller's pinned Task. It never stores a pointer in a Waker.
#[must_use = "dropping a registration cancels its task"]
pub struct Registration<'a, E> {
    task: Pin<&'a mut dyn Runnable<E>>,
}

impl<E> Drop for Registration<'_, E> {
    fn drop(&mut self) {
        self.task.as_mut().cancel();
    }
}

/// A fixed startup set of independent localsides, driven on one thread.
///
/// The task count is inferred from the registration array. Each poll visits
/// every live task at most once, rotating the first task between sweeps. The
/// current outer Waker is forwarded directly, so each wake requests another
/// sweep without task-local atomics or references to executor storage.
///
/// A task error cancels every remaining future synchronously. This is execution
/// cancellation, not a claim that a distributed protocol has rolled back.
#[must_use = "an executor must be driven to execute its tasks"]
pub struct Executor<'a, E, const N: usize> {
    tasks: [Option<Registration<'a, E>>; N],
    remaining: usize,
    first: usize,
}

impl<'a, E, const N: usize> Executor<'a, E, N> {
    /// Register exactly this set of tasks; their bodies are not polled yet.
    pub fn new(tasks: [Registration<'a, E>; N]) -> Self {
        Self {
            tasks: tasks.map(Some),
            remaining: N,
            first: 0,
        }
    }

    /// Drive tasks using a platform-owned wake and wait pair.
    ///
    /// `wait` must preserve a wake that occurs before it is called: for example,
    /// a thread unpark token, or a board event latch checked atomically with
    /// sleep. It must return after a wake. A bare interrupt sleep instruction
    /// without such a handshake can lose wakeups.
    ///
    /// The Waker may be cloned and retained after this method returns. Its
    /// storage and thread/interrupt safety are the platform driver's contract;
    /// the executor does not create scoped RawWakers.
    pub fn run(mut self, waker: &Waker, mut wait: impl FnMut()) -> Result<(), E> {
        let mut cx = Context::from_waker(waker);
        loop {
            match Pin::new(&mut self).poll(&mut cx) {
                Poll::Ready(result) => return result,
                Poll::Pending => wait(),
            }
        }
    }
}

// Moving an executor moves only pinned references and scheduling metadata, never
// the futures in caller-owned Task storage. E is not stored by the executor.
impl<E, const N: usize> Unpin for Executor<'_, E, N> {}

impl<E, const N: usize> Future for Executor<'_, E, N> {
    type Output = Result<(), E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let executor = self.get_mut();
        if executor.remaining == 0 {
            return Poll::Ready(Ok(()));
        }

        let mut index = executor.first;
        for _ in 0..N {
            if let Some(registration) = &mut executor.tasks[index] {
                match registration.task.as_mut().poll(cx) {
                    Poll::Ready(Ok(())) => {
                        executor.tasks[index] = None;
                        executor.remaining -= 1;
                    }
                    Poll::Ready(Err(error)) => {
                        // Release all task futures before exposing the error,
                        // even when a caller retains this executor after poll.
                        for task in &mut executor.tasks {
                            *task = None;
                        }
                        executor.remaining = 0;
                        return Poll::Ready(Err(error));
                    }
                    Poll::Pending => {}
                }
            }
            index = if index == N - 1 { 0 } else { index + 1 };
        }
        executor.first = if executor.first == N - 1 {
            0
        } else {
            executor.first + 1
        };

        if executor.remaining == 0 {
            Poll::Ready(Ok(()))
        } else {
            Poll::Pending
        }
    }
}
