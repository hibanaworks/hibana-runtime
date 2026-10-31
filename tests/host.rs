#![cfg(feature = "host")]

use core::{cell::Cell, convert::Infallible, future::pending};
use hibana_runtime::host::{Io, deadline, sleep};
use std::{io, os::unix::net::UnixStream, time::Duration};

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

#[test]
fn pending_read_resumes_when_an_independent_writer_finishes_its_timer() {
    #[hibana_runtime::main]
    async fn run(mut reader: Io, mut writer: Io) -> io::Result<()> {
        async {
            let mut bytes = [0; 3];
            let mut read = 0;
            while read < bytes.len() {
                let count = reader.read(&mut bytes[read..]).await?;
                assert_ne!(count, 0);
                read += count;
            }
            assert_eq!(&bytes, b"one");
        };
        async {
            sleep(Duration::from_millis(5)).await?;
            writer.write_all(b"one").await?;
        };
        Ok(())
    }
    let (reader, writer) = UnixStream::pair().unwrap();
    run(
        Io::new(reader.into()).unwrap(),
        Io::new(writer.into()).unwrap(),
    )
    .unwrap();
}

#[test]
fn backpressure_keeps_large_writes_pending_while_the_reader_progresses() {
    #[hibana_runtime::main]
    async fn run(mut reader: Io, mut writer: Io, input: &[u8]) -> io::Result<()> {
        async {
            sleep(Duration::from_millis(5)).await?;
            let mut received = 0;
            let mut bytes = [0; 4096];
            loop {
                let count = reader.read(&mut bytes).await?;
                if count == 0 {
                    break;
                }
                assert_eq!(&bytes[..count], &input[received..received + count]);
                received += count;
            }
            assert_eq!(received, input.len());
        };
        async move {
            writer.write_all(input).await?;
            drop(writer);
        };
        Ok(())
    }
    let (reader, writer) = UnixStream::pair().unwrap();
    let input: Vec<_> = (0..1024 * 1024).map(|n| n as u8).collect();
    run(
        Io::new(reader.into()).unwrap(),
        Io::new(writer.into()).unwrap(),
        &input,
    )
    .unwrap();
}

#[test]
fn a_timeout_cancels_the_read_and_the_same_handle_can_read_again() {
    #[hibana_runtime::main]
    async fn run(mut reader: Io, mut writer: Io) -> io::Result<()> {
        let mut bytes = [0; 1];
        let error =
            hibana_runtime::race(reader.read(&mut bytes), deadline(Duration::from_millis(5)))
                .await
                .unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::TimedOut);
        async {
            assert_eq!(reader.read(&mut bytes).await?, 1);
            assert_eq!(bytes[0], 7);
        };
        async {
            writer.write_all(&[7]).await?;
        };
        Ok(())
    }
    let (reader, writer) = UnixStream::pair().unwrap();
    run(
        Io::new(reader.into()).unwrap(),
        Io::new(writer.into()).unwrap(),
    )
    .unwrap();
}

#[test]
fn closed_peer_returns_broken_pipe_and_duplicate_handles_keep_their_own_lifetimes() {
    const CHILD: &str = "HIBANA_HOST_BROKEN_PIPE_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let status = std::process::Command::new(std::env::current_exe().unwrap())
            .args([
                "--exact",
                "closed_peer_returns_broken_pipe_and_duplicate_handles_keep_their_own_lifetimes",
                "--nocapture",
            ])
            .env(CHILD, "1")
            .status()
            .unwrap();
        assert!(
            status.success(),
            "closed-peer I/O terminated the child: {status}"
        );
        return;
    }
    // SAFETY: only this isolated child changes its process-wide signal handler.
    // The I/O handle must suppress SIGPIPE rather than relying on Rust startup.
    unsafe { libc::signal(libc::SIGPIPE, libc::SIG_DFL) };
    #[hibana_runtime::main]
    async fn run(writer: Io) -> io::Result<()> {
        let mut duplicate = writer.try_clone()?;
        drop(writer);
        assert_eq!(
            duplicate.write_all(&[1]).await.unwrap_err().kind(),
            io::ErrorKind::BrokenPipe
        );
        Ok(())
    }
    let (reader, writer) = UnixStream::pair().unwrap();
    drop(reader);
    run(Io::new(writer.into()).unwrap()).unwrap();
    let (reader, writer) = std::io::pipe().unwrap();
    drop(reader);
    run(Io::new(writer.into()).unwrap()).unwrap();
}

#[test]
fn pipe_readiness_and_eof_use_the_same_io_owner() {
    #[hibana_runtime::main]
    async fn run(mut reader: Io, mut writer: Io) -> io::Result<()> {
        async {
            let mut bytes = [0; 1];
            assert_eq!(reader.read(&mut bytes).await?, 1);
            assert_eq!(bytes[0], 9);
            assert_eq!(reader.read(&mut bytes).await?, 0);
        };
        async move {
            sleep(Duration::from_millis(1)).await?;
            writer.write_all(&[9]).await?;
            drop(writer);
        };
        Ok(())
    }
    let (reader, writer) = std::io::pipe().unwrap();
    run(
        Io::new(reader.into()).unwrap(),
        Io::new(writer.into()).unwrap(),
    )
    .unwrap();
}

#[test]
fn a_timer_starts_at_construction_and_zero_duration_is_ready() {
    #[hibana_runtime::main]
    async fn run(timer: impl Future<Output = io::Result<()>>) -> io::Result<()> {
        // The right side would win immediately if the timer needed another
        // poll. An expired timer must already be ready on its first poll.
        hibana_runtime::race(timer, async { Err(io::Error::from(io::ErrorKind::Other)) }).await?;
        sleep(Duration::ZERO).await?;
        Ok(())
    }
    let timer = sleep(Duration::from_millis(1));
    std::thread::sleep(Duration::from_millis(5));
    run(timer).unwrap();
}
