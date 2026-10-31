use core::{
    future::Future,
    marker::PhantomData,
    pin::Pin,
    task::{Context, Poll, Waker},
};

/// Inferred inline storage produced by [`Runtime::localside`].
///
/// This sealed bound lets the runtime retain different concrete future types
/// without allocation. Callers do not implement or name it.
#[doc(hidden)]
pub trait Localsides<E>: private::List<E> {}

impl<E, T: private::List<E>> Localsides<E> for T {}

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
/// Register each future with [`localside`](Self::localside), then call [`run`](Self::run)
/// or await the runtime. Both routes use the same scheduler. The futures stay
/// in caller-owned inline memory, with pinning handled inside `run` or by Rust's
/// ordinary async machinery when awaited.
///
/// Each poll visits every live task at most once, rotating the first slot.
/// Tasks receive the current driver's Waker directly; retained Wakers never
/// refer to the runtime's scoped storage.
#[must_use = "a runtime must be driven to execute its localsides"]
pub struct Runtime<E, T = ()> {
    tasks: T,
    first: Option<usize>,
    error: PhantomData<fn() -> E>,
}

impl<E> Runtime<E> {
    /// Start an empty runtime with the enclosing localside's error type.
    pub const fn new() -> Self {
        Self {
            tasks: (),
            first: Some(0),
            error: PhantomData,
        }
    }
}

impl<E> Default for Runtime<E> {
    fn default() -> Self {
        Self::new()
    }
}

impl<E, T: Localsides<E>> Runtime<E, T> {
    /// Register a localside future by value without polling or allocating.
    ///
    /// Generated code supplies each future and infers its inline storage.
    /// Successful unbound values are discarded; errors use the enclosing
    /// localside's `From` conversion after the completed future is dropped.
    pub fn localside<F, V, Error>(self, future: F) -> Runtime<E, impl Localsides<E>>
    where
        F: Future<Output = Result<V, Error>>,
        E: From<Error>,
    {
        Runtime {
            tasks: self.tasks.insert(private::Leaf {
                future: Some(future),
            }),
            first: Some(self.first.unwrap_or(0)),
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

    // A binary carry forest keeps inferred storage depth logarithmic. A linear
    // type list would hit Rust's layout recursion limit with 256 localsides.
    // There is no fixed chunk size or task-count limit.
    pub trait List<E>: Rows<E> + Sized {
        type Insert<G: Rows<E>>: List<E>;
        fn insert<G: Rows<E>>(self, rows: G) -> Self::Insert<G>;
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

    impl<E> List<E> for () {
        type Insert<G: Rows<E>> = Full<G, ()>;
        fn insert<G: Rows<E>>(self, rows: G) -> Self::Insert<G> {
            Full(Pair {
                left: (),
                right: rows,
            })
        }
    }

    pub struct Leaf<F> {
        pub(super) future: Option<F>,
    }

    impl<F> Leaf<F> {
        fn project(self: Pin<&mut Self>) -> Pin<&mut Option<F>> {
            // SAFETY: the private future field is structurally pinned. Leaf
            // has no custom Drop or Unpin and never moves a stored future.
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

    pub struct Pair<L, R> {
        left: L,
        right: R,
    }

    impl<L, R> Pair<L, R> {
        fn project(self: Pin<&mut Self>) -> (Pin<&mut L>, Pin<&mut R>) {
            // SAFETY: both private fields are structurally pinned and have
            // disjoint borrows. Pair has no custom Drop or Unpin, and no
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

    // Tail contains older, larger chunks; head contains the newest chunk.
    // Pair stores them in registration order, without a runtime locator table.
    pub struct Full<H, T>(Pair<T, H>);
    pub struct Vacant<T>(T);

    impl<H, T> Full<H, T> {
        fn project(self: Pin<&mut Self>) -> Pin<&mut Pair<T, H>> {
            // SAFETY: field 0 is structurally pinned, private, and never moved
            // through pinned access. Full has no custom Drop or Unpin.
            unsafe { self.map_unchecked_mut(|full| &mut full.0) }
        }
    }

    impl<T> Vacant<T> {
        fn project(self: Pin<&mut Self>) -> Pin<&mut T> {
            // SAFETY: field 0 is structurally pinned, private, and never moved
            // through pinned access. Vacant has no custom Drop or Unpin.
            unsafe { self.map_unchecked_mut(|vacant| &mut vacant.0) }
        }
    }

    impl<E, H: Rows<E>, T: List<E>> Rows<E> for Full<H, T> {
        const LEN: usize = H::LEN + T::LEN;
        fn poll_range(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            start: usize,
            end: usize,
        ) -> Result<usize, E> {
            self.project().poll_range(cx, start, end)
        }
        fn cancel(self: Pin<&mut Self>) {
            self.project().cancel();
        }
    }

    impl<E, H: Rows<E>, T: List<E>> List<E> for Full<H, T> {
        type Insert<G: Rows<E>> = Vacant<T::Insert<Pair<H, G>>>;
        fn insert<G: Rows<E>>(self, rows: G) -> Self::Insert<G> {
            Vacant(self.0.left.insert(Pair {
                left: self.0.right,
                right: rows,
            }))
        }
    }

    impl<E, T: List<E>> Rows<E> for Vacant<T> {
        const LEN: usize = T::LEN;
        fn poll_range(
            self: Pin<&mut Self>,
            cx: &mut Context<'_>,
            start: usize,
            end: usize,
        ) -> Result<usize, E> {
            self.project().poll_range(cx, start, end)
        }
        fn cancel(self: Pin<&mut Self>) {
            self.project().cancel();
        }
    }

    impl<E, T: List<E>> List<E> for Vacant<T> {
        type Insert<G: Rows<E>> = Full<G, T>;
        fn insert<G: Rows<E>>(self, rows: G) -> Self::Insert<G> {
            Full(Pair {
                left: self.0,
                right: rows,
            })
        }
    }
}
