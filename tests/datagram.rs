#![cfg(all(feature = "host", target_os = "macos"))]

use hibana_runtime::host::{Datagram, deadline, sleep};
use std::{
    fs, io,
    os::unix::net::UnixDatagram,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

static NEXT: AtomicUsize = AtomicUsize::new(0);

struct Addresses(PathBuf);
impl Addresses {
    fn new() -> Self {
        let path = PathBuf::from(format!(
            "/tmp/hbr-dgram-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn role(&self, role: u8) -> PathBuf {
        self.0.join(role.to_string())
    }
}
impl Drop for Addresses {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).unwrap();
    }
}

#[hibana_runtime::main]
#[test]
async fn datagrams_retain_message_boundaries_and_the_actual_bound_sender() -> io::Result<()> {
    let addresses = Addresses::new();
    let source = addresses.role(0);
    let target = addresses.role(1);
    let mut sender = Datagram::new(UnixDatagram::bind(&source)?)?.try_clone()?;
    let mut receiver = Datagram::new(UnixDatagram::bind(&target)?)?;
    async {
        async {
            let mut bytes = [0; 8];
            for expected in [b"first".as_slice(), b"", b"end"] {
                let (count, peer) = receiver.recv_from(&mut bytes).await?;
                assert_eq!(&bytes[..count], expected);
                assert_eq!(peer.as_pathname(), Some(source.as_path()));
            }
        };
        async {
            sleep(Duration::from_millis(2)).await?;
            for bytes in [b"first".as_slice(), b"", b"end"] {
                assert_eq!(sender.send_to(bytes, &target).await?, bytes.len());
            }
        };
        Ok(())
    }
    .await?;
    Ok(())
}

#[hibana_runtime::main]
#[test]
async fn a_cancelled_receive_releases_its_buffer_and_keeps_the_socket_usable() -> io::Result<()> {
    let addresses = Addresses::new();
    let source = addresses.role(0);
    let target = addresses.role(1);
    let mut sender = Datagram::new(UnixDatagram::bind(&source)?)?;
    let mut receiver = Datagram::new(UnixDatagram::bind(&target)?)?;
    let mut bytes = [0; 8];
    let error = hibana_runtime::race(
        receiver.recv_from(&mut bytes),
        deadline(Duration::from_millis(2)),
    )
    .await
    .unwrap_err();
    assert_eq!(error.kind(), io::ErrorKind::TimedOut);
    let ((count, peer), count_sent) = async {
        async { receiver.recv_from(&mut bytes).await };
        async { sender.send_to(b"next", &target).await };
    }
    .await?;
    assert_eq!(
        (count, count_sent, &bytes[..count]),
        (4, 4, b"next".as_slice())
    );
    assert_eq!(peer.as_pathname(), Some(source.as_path()));
    Ok(())
}
