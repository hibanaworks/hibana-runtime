use super::{EVFILT_WRITE, Io, descriptor};
use core::{
    ffi::{c_int, c_void},
    future::poll_fn,
    task::Poll,
};
use std::{
    io,
    net::SocketAddr,
    os::fd::{AsFd, AsRawFd, FromRawFd, OwnedFd},
};

const AF_INET: u8 = 2;
const AF_INET6: u8 = 30;
const SOCK_STREAM: c_int = 1;
const SOL_SOCKET: c_int = 0xffff;
const SO_ERROR: c_int = 0x1007;
const F_SETFD: c_int = 2;
const FD_CLOEXEC: c_int = 1;
const EINPROGRESS: c_int = 36;
const EALREADY: c_int = 37;
const ENOTCONN: c_int = 57;

unsafe extern "C" {
    fn socket(domain: c_int, kind: c_int, protocol: c_int) -> c_int;
    fn fcntl(fd: c_int, command: c_int, ...) -> c_int;
    fn connect(fd: c_int, address: *const c_void, length: u32) -> c_int;
    fn getsockopt(
        fd: c_int,
        level: c_int,
        option: c_int,
        value: *mut c_void,
        length: *mut u32,
    ) -> c_int;
    fn getpeername(fd: c_int, address: *mut c_void, length: *mut u32) -> c_int;
}

impl Io {
    /// Connect an owned TCP descriptor without blocking the host executor.
    ///
    /// Requires the host entry point while the connection is pending. Dropping
    /// this future closes its socket and removes its readiness registration.
    /// The returned handle uses the same read/write operations as other I/O.
    pub async fn connect(address: SocketAddr) -> io::Result<Self> {
        // sockaddr_in and sockaddr_in6 are encoded in aligned caller storage.
        // Darwin includes their byte length and family in the first two bytes.
        let mut storage = [0u32; 7];
        // SAFETY: all bytes of these initialized integers are writable; the
        // storage is suitably aligned for either socket address.
        let bytes =
            unsafe { core::slice::from_raw_parts_mut(storage.as_mut_ptr().cast::<u8>(), 28) };
        let (family, length) = match address {
            SocketAddr::V4(address) => {
                bytes[2..4].copy_from_slice(&address.port().to_be_bytes());
                bytes[4..8].copy_from_slice(&address.ip().octets());
                (AF_INET, 16u8)
            }
            SocketAddr::V6(address) => {
                bytes[2..4].copy_from_slice(&address.port().to_be_bytes());
                bytes[4..8].copy_from_slice(&address.flowinfo().to_be_bytes());
                bytes[8..24].copy_from_slice(&address.ip().octets());
                bytes[24..28].copy_from_slice(&address.scope_id().to_ne_bytes());
                (AF_INET6, 28u8)
            }
        };
        bytes[0] = length;
        bytes[1] = family;
        // SAFETY: socket takes only integer arguments and returns a fresh fd.
        let fd = unsafe { socket(c_int::from(family), SOCK_STREAM, 0) };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: successful socket creation transferred sole fd ownership.
        let fd = unsafe { OwnedFd::from_raw_fd(fd) };
        loop {
            // SAFETY: F_SETFD takes one int; no pointers are passed or retained.
            if unsafe { fcntl(fd.as_raw_fd(), F_SETFD, FD_CLOEXEC) } == 0 {
                break;
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
        let io = Self::new(fd)?;
        // SAFETY: address bytes have the family's complete initialized layout.
        // connect consumes them during this call and does not retain a pointer.
        if unsafe {
            connect(
                io.as_fd().as_raw_fd(),
                bytes.as_ptr().cast(),
                u32::from(length),
            )
        } == 0
        {
            return Ok(io);
        }
        let error = io::Error::last_os_error();
        if !matches!(error.raw_os_error(), Some(EINPROGRESS | EALREADY))
            && error.kind() != io::ErrorKind::Interrupted
        {
            return Err(error);
        }
        let mut interest = None;
        poll_fn(|cx| {
            loop {
                let mut error: c_int = 0;
                let mut length = size_of::<c_int>() as u32;
                // SAFETY: SO_ERROR writes one int to the complete local buffer.
                if unsafe {
                    getsockopt(
                        io.as_fd().as_raw_fd(),
                        SOL_SOCKET,
                        SO_ERROR,
                        (&mut error as *mut c_int).cast(),
                        &mut length,
                    )
                } < 0
                {
                    let error = io::Error::last_os_error();
                    if error.kind() == io::ErrorKind::Interrupted {
                        continue;
                    }
                    return Poll::Ready(Err(error));
                }
                if error != 0 {
                    return Poll::Ready(Err(io::Error::from_raw_os_error(error)));
                }
                let mut peer = [0u32; 7];
                let mut length = size_of_val(&peer) as u32;
                // SAFETY: the initialized, aligned local buffer is large enough
                // for either address. The call retains no Rust pointer.
                if unsafe {
                    getpeername(
                        io.as_fd().as_raw_fd(),
                        peer.as_mut_ptr().cast(),
                        &mut length,
                    )
                } == 0
                {
                    return Poll::Ready(Ok(()));
                }
                let error = io::Error::last_os_error();
                if error.kind() == io::ErrorKind::Interrupted {
                    continue;
                }
                if error.raw_os_error() != Some(ENOTCONN) {
                    return Poll::Ready(Err(error));
                }
                return descriptor::wait(io.as_fd(), cx, &mut interest, EVFILT_WRITE);
            }
        })
        .await?;
        Ok(io)
    }
}
