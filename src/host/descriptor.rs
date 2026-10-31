use super::{EV_ADD, EV_ONESHOT, EVFILT_READ, EVFILT_WRITE, Event, Events, Interest};
use core::{
    ffi::{c_int, c_void},
    future::poll_fn,
    ptr,
    task::{Context, Poll},
};
use std::{
    io,
    os::fd::{AsFd, AsRawFd, BorrowedFd, OwnedFd},
};

const F_GETFL: c_int = 3;
const F_SETFL: c_int = 4;
const F_SETNOSIGPIPE: c_int = 73;
const O_NONBLOCK: c_int = 0x4;
const SOL_SOCKET: c_int = 0xffff;
const SO_TYPE: c_int = 0x1008;
const MSG_NOSIGNAL: c_int = 0x80000;
const ENOTSOCK: c_int = 38;

unsafe extern "C" {
    fn fcntl(fd: c_int, command: c_int, ...) -> c_int;
    fn read(fd: c_int, buffer: *mut c_void, length: usize) -> isize;
    fn write(fd: c_int, buffer: *const c_void, length: usize) -> isize;
    fn send(fd: c_int, buffer: *const c_void, length: usize, flags: c_int) -> isize;
    fn getsockopt(
        fd: c_int,
        level: c_int,
        option: c_int,
        value: *mut c_void,
        length: *mut u32,
    ) -> c_int;
}

#[derive(Clone, Copy)]
enum Kind {
    Socket,
    File,
}

/// An owned wake direction for an external producer.
///
/// Enqueue the producer's data before calling [`notify`](Self::notify). Wakes
/// may coalesce; the receiving I/O owner must check its data before waiting.
/// Clones own separate OS descriptors and contain no scoped Rust pointers.
pub struct Notification(OwnedFd);

/// Open a notification direction and its asynchronous reader.
///
/// The reader receives bytes while producers are alive, and EOF after the last
/// producer is dropped and queued bytes are read. The bytes carry no payload or
/// count: read readiness means the I/O owner should check its external data.
pub fn notification() -> io::Result<(Notification, Io)> {
    let (reader, writer) = std::os::unix::net::UnixStream::pair()?;
    let writer = Io::new(writer.into())?;
    Ok((Notification(writer.into()), Io::new(reader.into())?))
}

impl Notification {
    /// Duplicate the wake descriptor for another producer.
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self(self.0.try_clone()?))
    }

    /// Make the reader ready without waiting for it or allocating Rust memory.
    /// A full socket already holds a wake, so additional wakes coalesce.
    /// A closed reader returns an I/O error without raising SIGPIPE.
    pub fn notify(&self) -> io::Result<()> {
        let byte = [1u8];
        loop {
            // SAFETY: the socket is nonblocking and the complete one-byte
            // buffer remains valid for the call. NOSIGNAL converts a closed
            // reader into an error. This private descriptor retains its mode.
            let written = unsafe {
                send(
                    self.0.as_raw_fd(),
                    byte.as_ptr().cast(),
                    byte.len(),
                    MSG_NOSIGNAL,
                )
            };
            if written == 1 {
                return Ok(());
            }
            if written == 0 {
                return Err(io::ErrorKind::WriteZero.into());
            }
            let error = io::Error::last_os_error();
            match error.kind() {
                io::ErrorKind::Interrupted => continue,
                io::ErrorKind::WouldBlock => return Ok(()),
                _ => return Err(error),
            }
        }
    }
}

/// One nonblocking host descriptor, owned by its I/O participant.
///
/// Reads and writes borrow this handle exclusively. For independent directions,
/// duplicate the descriptor with [`try_clone`](Self::try_clone) and give each
/// handle to its owner. Pending operations use the host entry point's queue;
/// dropping them removes their readiness registration.
pub struct Io {
    fd: OwnedFd,
    kind: Kind,
}

impl Io {
    /// Take ownership of a descriptor and enable nonblocking I/O.
    /// File status flags are shared with any existing OS duplicates; keep
    /// nonblocking mode enabled while these handles are in use.
    pub fn new(fd: OwnedFd) -> io::Result<Self> {
        let flags = loop {
            // SAFETY: the owned descriptor is live and F_GETFL takes no third argument.
            let flags = unsafe { fcntl(fd.as_raw_fd(), F_GETFL) };
            if flags >= 0 {
                break flags;
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        };
        let kind = loop {
            let mut socket_type: c_int = 0;
            let mut length = size_of::<c_int>() as u32;
            // SAFETY: SO_TYPE writes an int into the complete writable buffer.
            // Querying the descriptor's kind performs no stream operation.
            if unsafe {
                getsockopt(
                    fd.as_raw_fd(),
                    SOL_SOCKET,
                    SO_TYPE,
                    (&mut socket_type as *mut c_int).cast(),
                    &mut length,
                )
            } == 0
            {
                break Kind::Socket;
            }
            let error = io::Error::last_os_error();
            match error.kind() {
                io::ErrorKind::Interrupted => continue,
                _ if error.raw_os_error() == Some(ENOTSOCK) => break Kind::File,
                _ => return Err(error),
            }
        };
        loop {
            // SAFETY: F_SETFL takes one promoted int containing file status flags.
            if unsafe { fcntl(fd.as_raw_fd(), F_SETFL, flags | O_NONBLOCK) } == 0 {
                break;
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
        if matches!(kind, Kind::Socket) {
            return Ok(Self { fd, kind });
        }
        loop {
            // SAFETY: this Darwin command takes an int. Suppressing SIGPIPE
            // for this open file turns a closed peer into an ordinary error.
            if unsafe { fcntl(fd.as_raw_fd(), F_SETNOSIGPIPE, 1) } == 0 {
                return Ok(Self { fd, kind });
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }

    /// Duplicate this OS descriptor without sharing Rust mutable storage.
    pub fn try_clone(&self) -> io::Result<Self> {
        Ok(Self {
            fd: self.fd.try_clone()?,
            kind: self.kind,
        })
    }

    /// Read into the caller's buffer. Zero bytes reports EOF or an empty buffer.
    /// Requires the host entry point when the descriptor is not ready.
    pub async fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
        let mut interest = None;
        poll_fn(|cx| {
            loop {
                // SAFETY: the descriptor is live and buffer is exclusively writable
                // for its declared length. The call retains neither pointer.
                let count = unsafe {
                    read(
                        self.fd.as_raw_fd(),
                        buffer.as_mut_ptr().cast(),
                        buffer.len(),
                    )
                };
                if count >= 0 {
                    return Poll::Ready(Ok(count as usize));
                }
                let error = io::Error::last_os_error();
                match error.kind() {
                    io::ErrorKind::Interrupted => continue,
                    io::ErrorKind::WouldBlock => {
                        return self.wait(cx, &mut interest, EVFILT_READ);
                    }
                    _ => return Poll::Ready(Err(error)),
                }
            }
        })
        .await
    }

    /// Write every byte from the caller's buffer. Cancellation preserves bytes
    /// already written and removes the pending readiness registration.
    pub async fn write_all(&mut self, buffer: &[u8]) -> io::Result<()> {
        let mut written = 0;
        let mut interest = None;
        poll_fn(|cx| {
            loop {
                if written == buffer.len() {
                    return Poll::Ready(Ok(()));
                }
                let remaining = &buffer[written..];
                // SAFETY: the descriptor and the complete readable byte slice stay
                // live for this call. The OS does not retain the buffer pointer.
                let count = unsafe {
                    match self.kind {
                        Kind::Socket => send(
                            self.fd.as_raw_fd(),
                            remaining.as_ptr().cast(),
                            remaining.len(),
                            MSG_NOSIGNAL,
                        ),
                        Kind::File => write(
                            self.fd.as_raw_fd(),
                            remaining.as_ptr().cast(),
                            remaining.len(),
                        ),
                    }
                };
                if count > 0 {
                    written += count as usize;
                    continue;
                }
                if count == 0 {
                    return Poll::Ready(Err(io::ErrorKind::WriteZero.into()));
                }
                let error = io::Error::last_os_error();
                match error.kind() {
                    io::ErrorKind::Interrupted => continue,
                    io::ErrorKind::WouldBlock => {
                        return self.wait(cx, &mut interest, EVFILT_WRITE);
                    }
                    _ => return Poll::Ready(Err(error)),
                }
            }
        })
        .await
    }

    fn wait<T>(
        &self,
        cx: &Context<'_>,
        interest: &mut Option<Interest>,
        filter: i16,
    ) -> Poll<io::Result<T>> {
        // Rebind on each pending poll: the future may have moved to a different
        // driver, and a one-shot event may already have been consumed.
        *interest = None;
        let result = Events::current(cx).and_then(|queue| {
            Interest::new(
                queue,
                Event {
                    ident: self.fd.as_raw_fd() as usize,
                    filter,
                    flags: EV_ADD | EV_ONESHOT,
                    fflags: 0,
                    data: 0,
                    udata: ptr::null_mut(),
                },
            )
        });
        match result {
            Ok(registration) => {
                *interest = Some(registration);
                Poll::Pending
            }
            Err(error) => Poll::Ready(Err(error)),
        }
    }
}

impl AsFd for Io {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.fd.as_fd()
    }
}

impl From<Io> for OwnedFd {
    fn from(io: Io) -> Self {
        io.fd
    }
}
