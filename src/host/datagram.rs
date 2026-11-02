use super::{EVFILT_READ, EVFILT_WRITE, descriptor::wait};
use core::{future::poll_fn, task::Poll};
use std::{
    io,
    os::{
        fd::AsFd,
        unix::net::{SocketAddr, UnixDatagram},
    },
    path::Path,
};

/// One owned, nonblocking Unix datagram socket.
///
/// Each operation sends or receives one complete datagram. The socket retains
/// its bound address, including across descriptor duplication. A pending
/// operation owns its readiness registration and removes it when cancelled.
pub struct Datagram(UnixDatagram);

impl Datagram {
    /// Take ownership of a socket and enable nonblocking I/O.
    /// Existing descriptor duplicates share this file status flag.
    pub fn new(socket: UnixDatagram) -> io::Result<Self> {
        socket.set_nonblocking(true)?;
        Ok(Self(socket))
    }

    /// Duplicate the descriptor for an independently owned I/O direction.
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self(self.0.try_clone()?))
    }

    /// Send one datagram to a filesystem address.
    /// A cancelled pending send has published no partial datagram.
    pub async fn send_to(&mut self, bytes: &[u8], address: &Path) -> io::Result<usize> {
        let mut interest = None;
        poll_fn(|cx| {
            loop {
                match self.0.send_to(bytes, address) {
                    Ok(count) => return Poll::Ready(Ok(count)),
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        return wait(self.0.as_fd(), cx, &mut interest, EVFILT_WRITE);
                    }
                    Err(error) => return Poll::Ready(Err(error)),
                }
            }
        })
        .await
    }

    /// Receive one datagram and its actual OS sender address.
    /// The buffer bounds the copied bytes; a longer datagram is truncated.
    pub async fn recv_from(&mut self, bytes: &mut [u8]) -> io::Result<(usize, SocketAddr)> {
        let mut interest = None;
        poll_fn(|cx| {
            loop {
                match self.0.recv_from(bytes) {
                    Ok(received) => return Poll::Ready(Ok(received)),
                    Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                    Err(error) if error.kind() == io::ErrorKind::WouldBlock => {
                        return wait(self.0.as_fd(), cx, &mut interest, EVFILT_READ);
                    }
                    Err(error) => return Poll::Ready(Err(error)),
                }
            }
        })
        .await
    }
}
