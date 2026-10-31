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
pub async fn race<T>(
    left: impl core::future::Future<Output = T>,
    right: impl core::future::Future<Output = T>,
) -> T {
    let mut left = core::pin::pin!(left);
    let mut right = core::pin::pin!(right);
    let mut left_first = true;
    core::future::poll_fn(|cx| {
        let first = if left_first {
            left.as_mut().poll(cx)
        } else {
            right.as_mut().poll(cx)
        };
        left_first = !left_first;
        match first {
            core::task::Poll::Ready(value) => core::task::Poll::Ready(value),
            core::task::Poll::Pending if left_first => left.as_mut().poll(cx),
            core::task::Poll::Pending => right.as_mut().poll(cx),
        }
    })
    .await
}

/// Implementation details used by generated code, not an application API.
#[doc(hidden)]
pub mod __private {
    pub use crate::executor::Runtime;

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
