#![cfg(all(feature = "host", target_os = "macos"))]

use hibana_runtime::host::Io;
use std::{
    io::{self, Read, Write},
    net::{SocketAddr, TcpListener},
};

#[hibana_runtime::main]
#[test]
async fn tcp_connections_use_the_owned_io_handle_for_ipv4_and_ipv6() -> io::Result<()> {
    for address in ["127.0.0.1:0", "[::1]:0"] {
        let listener = TcpListener::bind(address)?;
        let address = listener.local_addr()?;
        let peer = std::thread::spawn(move || -> io::Result<()> {
            let (mut stream, _) = listener.accept()?;
            let mut bytes = [0; 5];
            stream.read_exact(&mut bytes)?;
            assert_eq!(&bytes, b"hello");
            stream.write_all(b"reply")
        });
        let mut stream = Io::connect(address).await?;
        stream.write_all(b"hello").await?;
        let mut bytes = [0; 5];
        let mut read = 0;
        while read != bytes.len() {
            let count = stream.read(&mut bytes[read..]).await?;
            assert_ne!(count, 0, "reply must not be truncated");
            read += count;
        }
        assert_eq!(&bytes, b"reply");
        peer.join().unwrap()?;
    }
    Ok(())
}

#[hibana_runtime::main]
#[test]
async fn failed_tcp_connections_report_the_socket_error() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:0")?;
    let address: SocketAddr = listener.local_addr()?;
    drop(listener);
    for _ in 0..32 {
        let error = match Io::connect(address).await {
            Ok(_) => panic!("closed listener unexpectedly accepted a connection"),
            Err(error) => error,
        };
        assert_eq!(error.kind(), io::ErrorKind::ConnectionRefused);
    }
    Ok(())
}
