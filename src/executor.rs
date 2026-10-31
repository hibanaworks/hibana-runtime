use core::{
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll, Waker},
};

/// Inferred inline storage produced by the localside attribute.
///
/// This sealed bound lets the runtime retain different concrete future types
/// without allocation. Callers do not implement or name it.
#[doc(hidden)]
pub trait Localsides<E>: private::Rows<E> {}

impl<E, T: private::Rows<E>> Localsides<E> for T {}

pub use private::{Leaf, Pair};

/// Platform integration for waking and parking the runtime.
///
/// The entry-point attribute uses a configured driver. A host or board
/// integration implements this boundary once; localsides write Endpoint
/// operations and application effects. The driver contains no protocol order or endpoint authority.
pub trait Driver {
    /// Return the runtime's wake handle. Clones may outlive the run and may be
    /// invoked from another thread or interrupt, under the Waker contract.
    fn waker(&self) -> Waker;

    /// Wait until notified, preserving a wake that arrived before this call.
    /// A thread unpark token or a board event latch provides this handshake.
    /// Spurious returns are allowed; dropping wake notifications is not.
    fn wait(&mut self);
}

/// A startup set of localsides, stored by value and driven on one thread.
///
/// Generated code constructs the complete storage once, then calls [`run`](Self::run)
/// or awaits the runtime. Both routes use the same scheduler. The futures stay
/// in caller-owned inline memory, with pinning handled inside `run` or by Rust's
/// ordinary async machinery when awaited.
///
/// Each poll visits every live task at most once, rotating the first slot.
/// Tasks receive the current driver's Waker directly; retained Wakers never
/// refer to the runtime's scoped storage.
#[must_use = "a runtime must be driven to execute its localsides"]
pub struct Runtime<E, T> {
    tasks: T,
    first: Option<usize>,
    error: PhantomData<fn() -> E>,
}

impl<E, T: Localsides<E>> Runtime<E, T> {
    /// Retain the complete generated storage without rebuilding its prefixes.
    pub const fn new(tasks: T) -> Self {
        Self {
            tasks,
            first: Some(0),
            error: PhantomData,
        }
    }

    /// Drive the localsides with a configured platform driver.
    ///
    /// The runtime owns pinning, polling and cancellation. Platform wake/wait
    /// details belong to [`Driver`], so application startup needs no Waker,
    /// callback, task storage, or pinning operation.
    pub fn run(self, driver: impl Driver) -> Result<(), E> {
        crate::__private::block_on(self, driver)
    }

    fn project(self: Pin<&mut Self>) -> (Pin<&mut T>, &mut Option<usize>) {
        // SAFETY: tasks is structurally pinned and private. There is no custom
        // Drop or Unpin implementation that could move it after pinning. The
        // two returned field borrows are disjoint; first is not pinned.
        unsafe {
            let runtime = self.get_unchecked_mut();
            (Pin::new_unchecked(&mut runtime.tasks), &mut runtime.first)
        }
    }
}

impl<E, T: Localsides<E>> Future for Runtime<E, T> {
    type Output = Result<(), E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let (mut tasks, first) = self.project();
        let Some(start) = *first else {
            return Poll::Ready(Ok(()));
        };

        // Traverse two disjoint ranges for the rotating order. Each range
        // walks the inline storage once, rather than looking up each task
        // through a nested type chain (which would cost O(N squared)).
        let result = tasks
            .as_mut()
            .poll_range(cx, start, T::LEN)
            .and_then(|suffix| {
                tasks
                    .as_mut()
                    .poll_range(cx, 0, start)
                    .map(|prefix| suffix + prefix)
            });
        match result {
            Err(error) => {
                tasks.cancel();
                *first = None;
                Poll::Ready(Err(error))
            }
            Ok(0) => {
                *first = None;
                Poll::Ready(Ok(()))
            }
            Ok(_) => {
                // A pending task implies LEN > 0, including for an empty run.
                *first = Some(if start == T::LEN - 1 { 0 } else { start + 1 });
                Poll::Pending
            }
        }
    }
}

mod private {
    use super::*;

    pub trait Rows<E> {
        const LEN: usize;
        fn poll_range(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            start: usize,
            end: usize,
        ) -> Result<usize, E>;
        fn cancel(self: Pin<&mut Self>);
    }

    impl<E> Rows<E> for () {
        const LEN: usize = 0;
        fn poll_range(
            self: Pin<&mut Self>,
            _: &mut Context<'_>,
            _: usize,
            _: usize,
        ) -> Result<usize, E> {
            Ok(0)
        }
        fn cancel(self: Pin<&mut Self>) {}
    }

    /// Generated inline storage for one future.
    pub struct Leaf<F> {
        pub future: Option<F>,
    }

    impl<F> Leaf<F> {
        fn project(self: Pin<&mut Self>) -> Pin<&mut Option<F>> {
            // SAFETY: future is structurally pinned. Leaf has no custom Drop
            // or Unpin and pinned access never moves a stored future.
            unsafe { self.map_unchecked_mut(|leaf| &mut leaf.future) }
        }
    }

    impl<E, V, Error, F: Future<Output = Result<V, Error>>> Rows<E> for Leaf<F>
    where
        E: From<Error>,
    {
        const LEN: usize = 1;

        fn poll_range(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            start: usize,
            end: usize,
        ) -> Result<usize, E> {
            if start >= end {
                return Ok(0);
            }
            let mut slot = self.project();
            let Some(future) = slot.as_mut().as_pin_mut() else {
                return Ok(0);
            };
            match future.poll(cx) {
                Poll::Ready(result) => {
                    slot.set(None);
                    result.map(|_| 0).map_err(E::from)
                }
                Poll::Pending => Ok(1),
            }
        }

        fn cancel(self: Pin<&mut Self>) {
            self.project().set(None);
        }
    }

    impl<E, F, const N: usize> Rows<E> for [Leaf<F>; N]
    where
        Leaf<F>: Rows<E>,
    {
        const LEN: usize = N;

        fn poll_range(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            start: usize,
            end: usize,
        ) -> Result<usize, E> {
            if start >= end {
                return Ok(0);
            }
            // SAFETY: array elements are structurally pinned. Convert directly
            // to a raw element pointer; slice indexing would create references
            // to the entire array, including other futures' pinned borrows.
            let rows = unsafe { self.get_unchecked_mut() } as *mut [Leaf<F>; N];
            let rows = rows.cast::<Leaf<F>>();
            let mut pending = 0;
            for index in start.min(N)..end.min(N) {
                // SAFETY: index is within the array and only this element is
                // reborrowed. Its address remains stable and no element moves.
                pending +=
                    unsafe { Pin::new_unchecked(&mut *rows.add(index)) }.poll_range(cx, 0, 1)?;
            }
            Ok(pending)
        }

        fn cancel(self: Pin<&mut Self>) {
            // SAFETY: retain each element's pin and avoid references to the
            // whole array while its futures contain active self borrows.
            let rows = unsafe { self.get_unchecked_mut() } as *mut [Leaf<F>; N];
            let rows = rows.cast::<Leaf<F>>();
            for index in 0..N {
                // SAFETY: index is in bounds. Only this stable element is
                // reborrowed and Leaf::cancel drops its future in place.
                unsafe { Pin::new_unchecked(&mut *rows.add(index)) }.cancel();
            }
        }
    }

    /// Generated source-ordered storage for two subtrees.
    pub struct Pair<L, R> {
        pub left: L,
        pub right: R,
    }

    impl<L, R> Pair<L, R> {
        fn project(self: Pin<&mut Self>) -> (Pin<&mut L>, Pin<&mut R>) {
            // SAFETY: both fields are structurally pinned and have disjoint
            // borrows. Pair has no custom Drop or Unpin, and no
            // pinned method moves either field out.
            unsafe {
                let pair = self.get_unchecked_mut();
                (
                    Pin::new_unchecked(&mut pair.left),
                    Pin::new_unchecked(&mut pair.right),
                )
            }
        }
    }

    impl<E, L: Rows<E>, R: Rows<E>> Rows<E> for Pair<L, R> {
        const LEN: usize = L::LEN + R::LEN;

        fn poll_range(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            start: usize,
            end: usize,
        ) -> Result<usize, E> {
            if start >= end {
                return Ok(0);
            }
            let (left, right) = self.project();
            let mut pending = if start < L::LEN {
                left.poll_range(cx, start, end.min(L::LEN))?
            } else {
                0
            };
            if end > L::LEN {
                pending += right.poll_range(cx, start.saturating_sub(L::LEN), end - L::LEN)?;
            }
            Ok(pending)
        }

        fn cancel(self: Pin<&mut Self>) {
            let (left, right) = self.project();
            left.cancel();
            right.cancel();
        }
    }
}
