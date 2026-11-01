#![cfg(not(miri))]

use core::{
    cell::Cell,
    convert::Infallible,
    future::Future,
    pin::Pin,
    task::{Context, Poll, Waker},
};
use hibana_runtime::Driver;

const BUFFER_BYTES: usize = 4096;
const LOCALSIDES: usize = 16;

struct Ready;
impl Driver for Ready {
    fn waker(&self) -> Waker {
        Waker::noop().clone()
    }
    fn wait(&mut self) {
        panic!("these localsides finish in their first poll");
    }
}

struct Buffered<'a> {
    bytes: [u8; BUFFER_BYTES],
    completed: &'a Cell<usize>,
}
impl Future for Buffered<'_> {
    type Output = Result<(), Infallible>;
    fn poll(self: Pin<&mut Self>, _: &mut Context<'_>) -> Poll<Self::Output> {
        assert_eq!(core::hint::black_box(&self.bytes)[BUFFER_BYTES - 1], 7);
        self.completed.set(self.completed.get() + 1);
        Poll::Ready(Ok(()))
    }
}
fn buffered(completed: &Cell<usize>) -> Buffered<'_> {
    Buffered {
        bytes: [7; BUFFER_BYTES],
        completed,
    }
}

#[hibana_runtime::main(Ready)]
async fn image(completed: &Cell<usize>) -> Result<(), Infallible> {
    #[concurrent]
    buffered(completed);
    #[concurrent]
    buffered(completed);
    #[concurrent]
    buffered(completed);
    #[concurrent]
    buffered(completed);
    #[concurrent]
    buffered(completed);
    #[concurrent]
    buffered(completed);
    #[concurrent]
    buffered(completed);
    #[concurrent]
    buffered(completed);
    #[concurrent]
    buffered(completed);
    #[concurrent]
    buffered(completed);
    #[concurrent]
    buffered(completed);
    #[concurrent]
    buffered(completed);
    #[concurrent]
    buffered(completed);
    #[concurrent]
    buffered(completed);
    #[concurrent]
    buffered(completed);
    #[concurrent]
    buffered(completed);
    Ok(())
}

#[test]
fn declaring_localsides_does_not_rebuild_every_earlier_buffer() {
    const CHILD: &str = "HIBANA_STACK_PROBE";
    if std::env::var_os(CHILD).is_none() {
        // An overflow aborts a process, so isolate the capacity probe. The
        // budget is derived from this fixture's data, not a runtime capacity.
        let result = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "declaring_localsides_does_not_rebuild_every_earlier_buffer",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "stack probe failed: {}\n{}",
            result.status,
            String::from_utf8_lossy(&result.stderr)
        );
        return;
    }
    let input_bytes = LOCALSIDES * core::mem::size_of::<Buffered>();
    std::thread::Builder::new()
        .stack_size(4 * input_bytes + 64 * 1024)
        .spawn(|| {
            let completed = Cell::new(0);
            image(&completed).unwrap();
            assert_eq!(completed.get(), LOCALSIDES);
        })
        .unwrap()
        .join()
        .unwrap();
}
