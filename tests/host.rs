#![cfg(feature = "host")]

use core::{cell::Cell, convert::Infallible, future::pending};

#[test]
fn host_entry_drives_independent_borrowed_localsides() {
    #[hibana_runtime::main]
    async fn run(left: &mut u8, right: &mut u8) -> Result<u8, Infallible> {
        async {
            async {
                hibana_runtime::yield_now().await;
                *left = 7;
            };
            async {
                hibana_runtime::yield_now().await;
                *right = 8;
            };
            Ok(())
        }
        .await?;
        Ok(*left + *right)
    }

    let mut left = 0;
    let mut right = 0;
    assert_eq!(run(&mut left, &mut right), Ok(15));
    assert_eq!((left, right), (7, 8));
}

#[test]
fn host_entry_resumes_a_local_future_after_an_external_wake() {
    #[hibana_runtime::main]
    async fn run() -> Result<(), Infallible> {
        let (send, receive) = std::sync::mpsc::channel();
        let mut producer = Some(send);
        core::future::poll_fn(|cx| {
            if receive.try_recv().is_ok() {
                return core::task::Poll::Ready(());
            }
            if let Some(send) = producer.take() {
                let wake = cx.waker().clone();
                std::thread::spawn(move || {
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    send.send(()).unwrap();
                    wake.wake();
                });
            }
            core::task::Poll::Pending
        })
        .await;
        Ok(())
    }
    assert_eq!(run(), Ok(()));
}

#[test]
fn host_entry_propagates_child_failure_and_drops_pending_siblings_in_place() {
    struct OnDrop<'a>(&'a Cell<usize>);
    impl Drop for OnDrop<'_> {
        fn drop(&mut self) {
            self.0.set(self.0.get() + 1);
        }
    }

    #[hibana_runtime::main]
    async fn run(dropped: &Cell<usize>) -> Result<(), &'static str> {
        let probe = OnDrop(dropped);
        async move {
            let _probe = probe;
            pending::<()>().await;
        };
        async {
            hibana_runtime::yield_now().await;
            Err("child failed")
        };
        Ok(())
    }

    let dropped = Cell::new(0);
    assert_eq!(run(&dropped), Err("child failed"));
    assert_eq!(dropped.get(), 1);
}
