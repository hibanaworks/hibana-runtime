//! Host image boundary: datagram frames and a race-free socket readiness driver.
//! Storage bounds are the four frames and four u32 payloads in protocol.rs.
use core::{
    cell::RefCell,
    task::{Context, Poll, Waker},
};
use hibana::runtime::{
    ids::SessionId,
    transport::{FrameHeader, Outgoing, PortOpen, ReceivedFrame, Transport, TransportError},
    wire::Payload,
};
use hibana_runtime::Driver;
use std::{
    io,
    os::{fd::AsRawFd, unix::net::UnixDatagram},
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    task::Wake,
};

const HEADER: usize = 8;
const PAYLOAD: usize = size_of::<u32>();
const FRAMES: usize = 4;

pub struct ImageIo {
    socket: UnixDatagram,
    queue: RefCell<[Option<Frame>; FRAMES]>,
    writable: Arc<AtomicUsize>,
}

#[derive(Clone, Copy)]
struct Frame {
    bytes: [u8; HEADER + PAYLOAD],
    len: usize,
}
impl Frame {
    fn matches(&self, address: Address) -> bool {
        self.bytes[..4] == address.session.raw().to_be_bytes()
            && self.bytes[4] == address.lane
            && self.bytes[6] == address.role
    }
    fn received(&self) -> ReceivedFrame<'_> {
        ReceivedFrame::framed(
            FrameHeader::from_bytes(self.bytes[..HEADER].try_into().unwrap()),
            Payload::new(&self.bytes[HEADER..self.len]),
        )
    }
}

impl ImageIo {
    pub fn new(socket: UnixDatagram) -> Self {
        socket.set_nonblocking(true).unwrap();
        Self {
            socket,
            queue: RefCell::new([None; FRAMES]),
            writable: Arc::new(AtomicUsize::new(0)),
        }
    }
    pub fn events(&self) -> Events {
        let (reader, writer) = UnixDatagram::pair().unwrap();
        reader.set_nonblocking(true).unwrap();
        writer.set_nonblocking(true).unwrap();
        Events {
            socket: self.socket.try_clone().unwrap(),
            writable: self.writable.clone(),
            reader,
            wake: Arc::new(Signal(writer)),
        }
    }
}

struct Signal(UnixDatagram);
impl Wake for Signal {
    fn wake(self: Arc<Self>) {
        self.wake_by_ref();
    }
    fn wake_by_ref(self: &Arc<Self>) {
        // A full wake socket is already readable. After driver destruction,
        // retained wakers may safely encounter the closed peer and do nothing.
        loop {
            match self.0.send(&[1]) {
                Ok(_) => return,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                Err(error)
                    if matches!(
                        error.kind(),
                        io::ErrorKind::WouldBlock
                            | io::ErrorKind::ConnectionRefused
                            | io::ErrorKind::ConnectionReset
                            | io::ErrorKind::NotConnected
                            | io::ErrorKind::BrokenPipe
                    ) =>
                {
                    return;
                }
                Err(error) => panic!("image wake failed: {error}"),
            }
        }
    }
}

pub struct Events {
    socket: UnixDatagram,
    writable: Arc<AtomicUsize>,
    reader: UnixDatagram,
    wake: Arc<Signal>,
}
impl Driver for Events {
    fn waker(&self) -> Waker {
        Waker::from(self.wake.clone())
    }
    fn wait(&mut self) {
        let mut fds = [
            libc::pollfd {
                fd: self.socket.as_raw_fd(),
                events: libc::POLLIN
                    | if self.writable.load(Ordering::Acquire) != 0 {
                        libc::POLLOUT
                    } else {
                        0
                    },
                revents: 0,
            },
            libc::pollfd {
                fd: self.reader.as_raw_fd(),
                events: libc::POLLIN,
                revents: 0,
            },
        ];
        loop {
            // SAFETY: both descriptors remain owned for this call, and fds is
            // a writable array containing exactly the supplied descriptor count.
            let result = unsafe { libc::poll(fds.as_mut_ptr(), fds.len() as libc::nfds_t, 10_000) };
            if result < 0 && io::Error::last_os_error().kind() == io::ErrorKind::Interrupted {
                continue;
            }
            assert!(
                result > 0,
                "image readiness failure or ten-second test watchdog: {:?}",
                io::Error::last_os_error()
            );
            assert!(
                fds.iter()
                    .all(|fd| fd.revents & (libc::POLLERR | libc::POLLHUP | libc::POLLNVAL) == 0),
                "image socket closed"
            );
            let _ = self.reader.recv(&mut [0]);
            return;
        }
    }
}

#[derive(Clone, Copy)]
pub struct Address {
    session: SessionId,
    lane: u8,
    role: u8,
}
pub struct Rx {
    address: Address,
    current: Option<Frame>,
}

pub struct Tx {
    address: Address,
    writable: Arc<AtomicUsize>,
    waiting: bool,
}
impl Tx {
    fn waiting(&mut self, waiting: bool) {
        if self.waiting == waiting {
            return;
        }
        self.waiting = waiting;
        if waiting {
            self.writable.fetch_add(1, Ordering::AcqRel);
        } else {
            self.writable.fetch_sub(1, Ordering::AcqRel);
        }
    }
}
impl Drop for Tx {
    fn drop(&mut self) {
        self.waiting(false);
    }
}

impl Transport for ImageIo {
    type Tx<'a> = Tx;
    type Rx<'a> = Rx;
    fn open(&self, port: PortOpen) -> (Self::Tx<'_>, Self::Rx<'_>) {
        let address = Address {
            session: port.session_id(),
            lane: port.lane(),
            role: port.local_role(),
        };
        (
            Tx {
                address,
                writable: self.writable.clone(),
                waiting: false,
            },
            Rx {
                address,
                current: None,
            },
        )
    }
    fn poll_send<'a, 'f>(
        &self,
        tx: &'a mut Self::Tx<'a>,
        outgoing: Outgoing<'f>,
        _: &mut Context<'_>,
    ) -> Poll<Result<(), TransportError>>
    where
        'a: 'f,
    {
        if outgoing.lane() != tx.address.lane || outgoing.payload().as_bytes().len() > PAYLOAD {
            tx.waiting(false);
            return Poll::Ready(Err(TransportError::Capacity));
        }
        let session = tx.address.session.raw().to_be_bytes();
        let mut bytes = [0; HEADER + PAYLOAD];
        bytes[..HEADER].copy_from_slice(&[
            session[0],
            session[1],
            session[2],
            session[3],
            tx.address.lane,
            tx.address.role,
            outgoing.target_role(),
            outgoing.frame_label().raw(),
        ]);
        let len = HEADER + outgoing.payload().as_bytes().len();
        bytes[HEADER..len].copy_from_slice(outgoing.payload().as_bytes());
        match self.socket.send(&bytes[..len]) {
            Ok(count) if count == len => {
                tx.waiting(false);
                Poll::Ready(Ok(()))
            }
            Err(error)
                if matches!(
                    error.kind(),
                    io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted
                ) =>
            {
                tx.waiting(true);
                Poll::Pending
            }
            _ => {
                tx.waiting(false);
                Poll::Ready(Err(TransportError::Failed))
            }
        }
    }
    fn cancel_send<'a>(&self, tx: &'a mut Self::Tx<'a>) {
        tx.waiting(false);
    }
    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<ReceivedFrame<'a>, TransportError>> {
        rx.current = None;
        // A bounded drain cannot monopolize this image's other localsides.
        for _ in 0..FRAMES {
            let mut queue = self.queue.borrow_mut();
            if let Some(index) = queue
                .iter()
                .position(|slot| slot.as_ref().is_some_and(|frame| frame.matches(rx.address)))
            {
                rx.current = queue[index].take();
                return Poll::Ready(Ok(rx.current.as_ref().unwrap().received()));
            }
            let Some(slot) = queue.iter_mut().find(|slot| slot.is_none()) else {
                return Poll::Ready(Err(TransportError::Capacity));
            };
            let mut frame = Frame {
                bytes: [0; HEADER + PAYLOAD],
                len: 0,
            };
            let mut received = [0; HEADER + PAYLOAD + 1];
            match self.socket.recv(&mut received) {
                Ok(len) if (HEADER..=HEADER + PAYLOAD).contains(&len) => {
                    frame.len = len;
                    frame.bytes[..len].copy_from_slice(&received[..len]);
                    *slot = Some(frame);
                    drop(queue);
                    cx.waker().wake_by_ref();
                }
                Err(error) if error.kind() == io::ErrorKind::WouldBlock => return Poll::Pending,
                Err(error) if error.kind() == io::ErrorKind::Interrupted => continue,
                _ => return Poll::Ready(Err(TransportError::Failed)),
            }
        }
        // Frames moved from socket to queue; re-poll even if the socket is now empty.
        // The parent wake socket preserves this request before Driver::wait.
        cx.waker().wake_by_ref();
        Poll::Pending
    }
    fn requeue(&self, rx: &mut Self::Rx<'_>) -> Result<(), TransportError> {
        let mut queue = self.queue.borrow_mut();
        let slot = queue
            .iter_mut()
            .find(|slot| slot.is_none())
            .ok_or(TransportError::Capacity)?;
        *slot = Some(rx.current.take().ok_or(TransportError::Failed)?);
        Ok(())
    }
}
