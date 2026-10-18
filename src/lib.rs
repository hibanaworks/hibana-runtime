//! Localsides declare work; their lexical scopes own its execution.
//!
//! In an attributed async function, `#[localside] worker(...);` declares a
//! named localside directly. Standalone `async { ... };` statements declare
//! inline localsides. They run concurrently with the rest of their block and
//! finish before that block exits. Ordinary awaited expressions stay sequential.
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
//!     let left = Cell::new(0);
//!     let right = Cell::new(0);
//!     {
//!         #[localside]
//!         write(&left, 7);
//!         #[localside]
//!         write(&right, 8);
//!     }
//!     assert_eq!((left.get(), right.get()), (7, 8));
//!     Ok(())
//! }
//! example().unwrap();
//! ```
//!
//! The same exclusive Endpoint cannot be borrowed by two declarations:
//!
//! ```compile_fail,E0499
//! #[hibana_runtime::localside]
//! async fn conflicting(mut endpoint: hibana::Endpoint<'_, 0>) -> Result<(), hibana::EndpointError> {
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

mod executor;

pub use executor::Driver;
pub use hibana_runtime_macros::{localside, main};

/// Implementation details used by generated code, not an application API.
#[doc(hidden)]
pub mod __private {
    pub use crate::executor::Runtime;

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
