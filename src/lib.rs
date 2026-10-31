//! Localsides declare work; their async bodies own its execution.
//!
//! In an attributed async function, `#[localside] worker(...);` declares a
//! named localside directly. Standalone `async { ... };` statements declare
//! inline localsides. They run concurrently with the rest of their async body and
//! finish before that future returns. Ordinary blocks never wait. An explicit
//! `.await` is the completion boundary for a nested async body.
//! Hibana remains the only owner of protocol progress and endpoint authority.
//!
//! ```
//! use core::{cell::Cell, convert::Infallible, task::Waker};
//! # struct Events;
//! # impl hibana_runtime::Driver for Events {
//! #     fn waker(&self) -> Waker { Waker::noop().clone() }
//! #     fn wait(&mut self) {}
//! # }
//! async fn write(value: &Cell<u32>, next: u32) -> Result<(), Infallible> {
//!     value.set(next);
//!     Ok(())
//! }
//! #[hibana_runtime::main(Events)]
//! async fn example() -> Result<(), Infallible> {
//!     let responses = Cell::new(0);
//!     let audits = Cell::new(0);
//!     async {
//!         #[localside]
//!         write(&responses, 7);
//!         #[localside]
//!         write(&audits, 8);
//!         Ok(())
//!     }.await?;
//!     assert_eq!((responses.get(), audits.get()), (7, 8));
//!     Ok(())
//! }
//! example().unwrap();
//! ```
//!
//! The same exclusive Endpoint cannot be borrowed by two declarations:
//!
//! ```compile_fail,E0499
//! const CLIENT: u8 = 0;
//! #[hibana_runtime::localside]
//! async fn conflicting(mut endpoint: hibana::Endpoint<CLIENT>) -> Result<(), hibana::EndpointError> {
//!     async { endpoint.send::<hibana::g::Msg<1, u32>>(&7).await };
//!     async { endpoint.send::<hibana::g::Msg<2, u32>>(&8).await };
//!     Ok(())
//! }
//! ```
//!
//! Moving an owned future into two declarations is also rejected:
//!
//! ```compile_fail,E0382
//! #[hibana_runtime::localside]
//! async fn duplicated() -> Result<(), ()> {
//!     let future = async { Ok::<(), ()>(()) };
//!     async { future.await };
//!     async { future.await };
//!     Ok(())
//! }
//! ```
#![no_std]

#[cfg(feature = "host")]
extern crate std;

mod executor;
#[cfg(feature = "host")]
mod host;

pub use executor::Driver;
pub use hibana_runtime_macros::{localside, main};

/// Give other localsides a turn before continuing a computation.
///
/// This returns `Pending` once and requests another poll. It neither advances
/// an endpoint nor cancels an operation.
pub async fn yield_now() {
    let mut yielded = false;
    core::future::poll_fn(|cx| {
        if yielded {
            core::task::Poll::Ready(())
        } else {
            yielded = true;
            cx.waker().wake_by_ref();
            core::task::Poll::Pending
        }
    })
    .await;
}

/// Complete when either future completes, dropping the other in place.
///
/// Poll order alternates after a pending poll. Cancellation follows each
/// future's own contract; accepted communication is never rolled back.
/// Each input is stored once in caller-owned memory. Both are dropped before
/// the result is returned.
pub fn race<T>(
    left: impl core::future::Future<Output = T>,
    right: impl core::future::Future<Output = T>,
) -> impl core::future::Future<Output = T> {
    Race {
        left: Some(left),
        right: Some(right),
        left_first: true,
    }
}

// Keep the inputs in their final slots instead of moving them into a second
// pair of pinned locals inside an async wrapper.
struct Race<L, R> {
    left: Option<L>,
    right: Option<R>,
    left_first: bool,
}

struct StoreResult<'a, F, V> {
    future: F,
    output: &'a mut Option<V>,
}

impl<F, V, E> core::future::Future for StoreResult<'_, F, V>
where
    F: core::future::Future<Output = Result<V, E>>,
{
    type Output = Result<(), E>;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        // SAFETY: future is a private structurally pinned field. The separate
        // output reference is unpinned, and neither field is moved here. There
        // is no custom Drop or Unpin implementation.
        let (future, output) = unsafe {
            let store = self.get_unchecked_mut();
            (
                core::pin::Pin::new_unchecked(&mut store.future),
                &mut store.output,
            )
        };
        future.poll(cx).map(|result| {
            result.map(|value| {
                **output = Some(value);
            })
        })
    }
}

impl<L: core::future::Future, R: core::future::Future<Output = L::Output>> core::future::Future
    for Race<L, R>
{
    type Output = L::Output;

    fn poll(
        self: core::pin::Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Self::Output> {
        // SAFETY: the private input fields are structurally pinned and disjoint.
        // There is no custom Drop or Unpin implementation. Clearing a slot
        // drops its future in place; neither polling nor cancellation moves it.
        let (mut left, mut right, left_first) = unsafe {
            let race = self.get_unchecked_mut();
            (
                core::pin::Pin::new_unchecked(&mut race.left),
                core::pin::Pin::new_unchecked(&mut race.right),
                &mut race.left_first,
            )
        };
        let mut left_future = left
            .as_mut()
            .as_pin_mut()
            .expect("a completed race cannot be polled again");
        let mut right_future = right
            .as_mut()
            .as_pin_mut()
            .expect("a completed race cannot be polled again");
        let first = if *left_first {
            left_future.as_mut().poll(cx)
        } else {
            right_future.as_mut().poll(cx)
        };
        *left_first = !*left_first;
        let result = match first {
            core::task::Poll::Ready(value) => core::task::Poll::Ready(value),
            core::task::Poll::Pending if *left_first => left_future.as_mut().poll(cx),
            core::task::Poll::Pending => right_future.as_mut().poll(cx),
        };
        if result.is_ready() {
            left.set(None);
            right.set(None);
        }
        result
    }
}

/// Implementation details used by generated code, not an application API.
#[doc(hidden)]
pub mod __private {
    pub use crate::executor::Runtime;

    /// Save a completed child value without moving its future into an awaiter.
    pub fn store_result<F, V, E>(
        future: F,
        output: &mut Option<V>,
    ) -> impl core::future::Future<Output = Result<(), E>>
    where
        F: core::future::Future<Output = Result<V, E>>,
    {
        super::StoreResult { future, output }
    }

    /// Consume an async closure so its returned future owns its captures.
    #[inline]
    pub fn call_once<T>(body: impl FnOnce() -> T) -> T {
        body()
    }

    /// Host readiness integration for the entry-point attribute.
    #[cfg(feature = "host")]
    pub fn host_block_on<F: core::future::Future>(future: F) -> F::Output {
        crate::host::block_on(future)
    }

    /// Infer the error type even when the function uses a Result alias.
    pub trait Outcome {
        type Error;
    }

    impl<T, E> Outcome for Result<T, E> {
        type Error = E;
    }

    pub fn block_on<F: core::future::Future>(
        future: F,
        mut driver: impl crate::Driver,
    ) -> F::Output {
        let waker = driver.waker();
        let mut future = core::pin::pin!(future);
        let mut cx = core::task::Context::from_waker(&waker);
        loop {
            match future.as_mut().poll(&mut cx) {
                core::task::Poll::Ready(output) => return output,
                core::task::Poll::Pending => driver.wait(),
            }
        }
    }
}
