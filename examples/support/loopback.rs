//! A finite, allocation-free carrier for the localsides example.
//!
//! Capacities come from that example's longest trace and concurrent receivers.
//! A device carrier must additionally connect interrupts and peer closure to
//! parked wakers; this in-process example has no external peers or I/O.

use core::{
    cell::RefCell,
    mem::size_of,
    task::{Context, Poll, Waker},
};

use hibana::runtime::{
    ids::SessionId,
    transport::{FrameHeader, Outgoing, PortOpen, ReceivedFrame, Transport, TransportError},
    wire::Payload,
};

pub struct Loopback<const FRAMES: usize, const RECEIVERS: usize> {
    state: RefCell<State<FRAMES, RECEIVERS>>,
}

impl<const FRAMES: usize, const RECEIVERS: usize> Loopback<FRAMES, RECEIVERS> {
    pub fn new() -> Self {
        Self {
            state: RefCell::new(State {
                frames: [None; FRAMES],
                len: 0,
                waiters: [const { None }; RECEIVERS],
            }),
        }
    }
}

struct State<const FRAMES: usize, const RECEIVERS: usize> {
    frames: [Option<Frame>; FRAMES],
    len: usize,
    waiters: [Option<Waiter>; RECEIVERS],
}

#[derive(Clone, Copy)]
struct Address {
    session: SessionId,
    lane: u8,
    role: u8,
}

impl Address {
    fn matches(self, other: Self) -> bool {
        self.session == other.session && self.lane == other.lane && self.role == other.role
    }
}

#[derive(Clone, Copy)]
struct Frame {
    destination: Address,
    source: u8,
    label: u8,
    payload: [u8; size_of::<u32>()],
}

impl Frame {
    fn received(&self) -> ReceivedFrame<'_> {
        let session = self.destination.session.raw().to_be_bytes();
        let header = FrameHeader::from_bytes([
            session[0],
            session[1],
            session[2],
            session[3],
            self.destination.lane,
            self.source,
            self.destination.role,
            self.label,
        ]);
        ReceivedFrame::framed(header, Payload::new(&self.payload))
    }
}

struct Waiter {
    address: Address,
    waker: Waker,
}

pub struct Tx(Address);

pub struct Rx {
    address: Address,
    current: Option<Frame>,
}

impl<const FRAMES: usize, const RECEIVERS: usize> Transport for Loopback<FRAMES, RECEIVERS> {
    type Tx<'a> = Tx;
    type Rx<'a> = Rx;

    fn open(&self, port: PortOpen) -> (Self::Tx<'_>, Self::Rx<'_>) {
        let address = Address {
            session: port.session_id(),
            lane: port.lane(),
            role: port.local_role(),
        };
        (
            Tx(address),
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
        _cx: &mut Context<'_>,
    ) -> Poll<Result<(), TransportError>>
    where
        'a: 'f,
    {
        if outgoing.lane() != tx.0.lane {
            return Poll::Ready(Err(TransportError::Failed));
        }
        let Ok(payload) = outgoing.payload().as_bytes().try_into() else {
            return Poll::Ready(Err(TransportError::Capacity));
        };
        let frame = Frame {
            destination: Address {
                session: tx.0.session,
                lane: tx.0.lane,
                role: outgoing.target_role(),
            },
            source: tx.0.role,
            label: outgoing.frame_label().raw(),
            payload,
        };
        let wake = {
            let mut state = self.state.borrow_mut();
            if state.len == FRAMES {
                return Poll::Ready(Err(TransportError::Capacity));
            }
            let index = state.len;
            state.frames[index] = Some(frame);
            state.len += 1;
            state
                .waiters
                .iter_mut()
                .find(|slot| {
                    slot.as_ref()
                        .is_some_and(|waiter| waiter.address.matches(frame.destination))
                })
                .and_then(Option::take)
                .map(|waiter| waiter.waker)
        };
        if let Some(waker) = wake {
            waker.wake();
        }
        Poll::Ready(Ok(()))
    }

    fn cancel_send<'a>(&self, _tx: &'a mut Self::Tx<'a>) {
        // A send commits immediately or fails without publishing a frame.
    }

    fn poll_recv<'a>(
        &'a self,
        rx: &'a mut Self::Rx<'a>,
        cx: &mut Context<'_>,
    ) -> Poll<Result<ReceivedFrame<'a>, TransportError>> {
        rx.current = None;
        let frame = {
            let mut state = self.state.borrow_mut();
            let index = state.frames[..state.len].iter().position(|slot| {
                slot.as_ref()
                    .is_some_and(|frame| frame.destination.matches(rx.address))
            });
            match index {
                Some(index) => {
                    let frame = state.frames[index].take();
                    for next in index + 1..state.len {
                        state.frames[next - 1] = state.frames[next].take();
                    }
                    state.len -= 1;
                    frame
                }
                None => {
                    let existing = state.waiters.iter().position(|slot| {
                        slot.as_ref()
                            .is_some_and(|waiter| waiter.address.matches(rx.address))
                    });
                    let slot = existing.or_else(|| state.waiters.iter().position(Option::is_none));
                    let Some(slot) = slot else {
                        return Poll::Ready(Err(TransportError::Capacity));
                    };
                    state.waiters[slot] = Some(Waiter {
                        address: rx.address,
                        waker: cx.waker().clone(),
                    });
                    None
                }
            }
        };
        match frame {
            Some(frame) => {
                rx.current = Some(frame);
                Poll::Ready(Ok(rx.current.as_ref().expect("stored frame").received()))
            }
            None => Poll::Pending,
        }
    }

    fn requeue(&self, rx: &mut Self::Rx<'_>) -> Result<(), TransportError> {
        let mut state = self.state.borrow_mut();
        if state.len == FRAMES {
            return Err(TransportError::Capacity);
        }
        let frame = rx.current.take().ok_or(TransportError::Failed)?;
        for index in (0..state.len).rev() {
            state.frames[index + 1] = state.frames[index].take();
        }
        state.frames[0] = Some(frame);
        state.len += 1;
        Ok(())
    }
}
