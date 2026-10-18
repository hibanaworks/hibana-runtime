//! Linked Cortex-M0+ executor probe. This is not flashable Pico firmware.
#![cfg_attr(target_os = "none", no_std)]
#![cfg_attr(target_os = "none", no_main)]

#[cfg(target_os = "none")]
mod embedded {
    use core::{
        convert::Infallible,
        future::Future,
        mem::size_of,
        pin::{Pin, pin},
        ptr,
        task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
    };
    use hibana_runtime::{Executor, Task};

    struct Work(u32);

    impl Future for Work {
        type Output = Result<(), Infallible>;

        fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
            let remaining = core::hint::black_box(self.0);
            if remaining == 0 {
                Poll::Ready(Ok(()))
            } else {
                self.0 = remaining - 1;
                cx.waker().wake_by_ref();
                Poll::Pending
            }
        }
    }

    #[used]
    #[unsafe(no_mangle)]
    static TASK_STORAGE_BYTES: [u8; size_of::<Task<Work>>()] = [0; size_of::<Task<Work>>()];
    #[used]
    #[unsafe(no_mangle)]
    static EXECUTOR_STORAGE_BYTES: [u8; size_of::<Executor<'static, Infallible, 2>>()] =
        [0; size_of::<Executor<'static, Infallible, 2>>()];

    fn raw_waker() -> RawWaker {
        RawWaker::new(ptr::null(), &VTABLE)
    }
    fn clone(_: *const ()) -> RawWaker {
        raw_waker()
    }
    fn wake(_: *const ()) {
        // SAFETY: SEV only sets the architecture event latch. No data pointer
        // or scoped state is accessed, on any thread or interrupt.
        unsafe { core::arch::asm!("sev", options(nostack)) };
    }
    fn drop_waker(_: *const ()) {}
    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, wake, wake, drop_waker);

    fn wait() {
        // SAFETY: WFE consumes an event already latched by SEV, or waits for
        // the next one. A wake before this instruction is therefore retained.
        unsafe { core::arch::asm!("wfe", options(nostack)) };
    }

    #[unsafe(no_mangle)]
    pub extern "C" fn _start() -> ! {
        let mut left = pin!(Task::new(Work(core::hint::black_box(3))));
        let mut right = pin!(Task::new(Work(core::hint::black_box(5))));
        // SAFETY: callbacks ignore the null data pointer and access only the
        // architectural event latch. Every clone is valid indefinitely.
        let waker = unsafe { Waker::from_raw(raw_waker()) };
        Executor::new([left.as_mut().register(), right.as_mut().register()])
            .run(&waker, wait)
            .unwrap();
        loop {
            wait();
        }
    }

    #[panic_handler]
    fn panic(_: &core::panic::PanicInfo<'_>) -> ! {
        loop {
            wait();
        }
    }
}

#[cfg(not(target_os = "none"))]
fn main() {
    println!("Build this link probe for thumbv6m-none-eabi; see README.md.");
}
