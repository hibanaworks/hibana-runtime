//! The host event queue owns wake delivery; localsides keep their inline storage.

#[cfg(not(target_os = "macos"))]
compile_error!("the host driver currently supports macOS; use main(driver) on other platforms");

use crate::Driver;
use core::{
    ffi::{c_int, c_void},
    future::Future,
    marker::PhantomData,
    pin::Pin,
    ptr,
    task::{Context, Poll, RawWaker, RawWakerVTable, Waker},
};
use std::{
    io,
    os::fd::{AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd},
    time::{Duration, Instant},
};

mod descriptor;
pub use descriptor::Io;
mod process;
pub use process::Process;

const EVFILT_READ: i16 = -1;
const EVFILT_WRITE: i16 = -2;
const EVFILT_TIMER: i16 = -7;
const EVFILT_USER: i16 = -10;
const EV_ADD: u16 = 0x1;
const EV_DELETE: u16 = 0x2;
const EV_ONESHOT: u16 = 0x10;
const EV_CLEAR: u16 = 0x20;
const NOTE_NSECONDS: u32 = 0x4;
const NOTE_TRIGGER: u32 = 0x0100_0000;
const WAKE: usize = 1;

#[derive(Clone, Copy)]
#[repr(C, packed(4))]
struct Event {
    ident: usize,
    filter: i16,
    flags: u16,
    fflags: u32,
    data: isize,
    udata: *mut c_void,
}

impl Event {
    const fn wake(flags: u16, fflags: u32) -> Self {
        Self {
            ident: WAKE,
            filter: EVFILT_USER,
            flags,
            fflags,
            data: 0,
            udata: ptr::null_mut(),
        }
    }
}

unsafe extern "C" {
    fn kqueue() -> c_int;
    fn kevent(
        queue: c_int,
        changes: *const Event,
        change_count: c_int,
        events: *mut Event,
        event_count: c_int,
        timeout: *const c_void,
    ) -> c_int;
}

struct Events(OwnedFd);

impl Events {
    fn current(cx: &Context<'_>) -> io::Result<Self> {
        let wake = cx.waker();
        if !ptr::eq(wake.vtable(), &VTABLE) {
            return Err(io::ErrorKind::InvalidInput.into());
        }
        // SAFETY: this vtable is private and its Wakers own the descriptor in
        // data. The Context keeps that Waker alive while the fd is duplicated.
        let queue = unsafe { BorrowedFd::borrow_raw(descriptor(wake.data())) };
        Ok(Self(queue.try_clone_to_owned()?))
    }

    fn new() -> io::Result<Self> {
        // SAFETY: kqueue takes no arguments and returns a fresh owned descriptor.
        let fd = unsafe { kqueue() };
        if fd < 0 {
            return Err(io::Error::last_os_error());
        }
        // SAFETY: a successful kqueue call transferred ownership of this fd.
        let queue = Self(unsafe { OwnedFd::from_raw_fd(fd) });
        queue.change(Event::wake(EV_ADD | EV_CLEAR, 0))?;
        Ok(queue)
    }

    fn change(&self, event: Event) -> io::Result<()> {
        loop {
            // SAFETY: the queue and complete Event remain valid for the call.
            // No output is requested; no scoped address is stored by the kernel.
            if unsafe {
                kevent(
                    self.0.as_raw_fd(),
                    &event,
                    1,
                    ptr::null_mut(),
                    0,
                    ptr::null(),
                )
            } == 0
            {
                return Ok(());
            }
            let error = io::Error::last_os_error();
            if error.kind() != io::ErrorKind::Interrupted {
                return Err(error);
            }
        }
    }
}

struct Interest {
    queue: Events,
    ident: usize,
    filter: i16,
}

impl Interest {
    fn new(queue: Events, event: Event) -> io::Result<Self> {
        queue.change(event)?;
        Ok(Self {
            queue,
            ident: event.ident,
            filter: event.filter,
        })
    }
}

impl Drop for Interest {
    fn drop(&mut self) {
        let result = self.queue.change(Event {
            ident: self.ident,
            filter: self.filter,
            flags: EV_DELETE,
            fflags: 0,
            data: 0,
            udata: ptr::null_mut(),
        });
        if let Err(error) = result {
            assert_eq!(
                error.kind(),
                io::ErrorKind::NotFound,
                "cancel host wait: {error}"
            );
        }
    }
}

struct Timer {
    started: Instant,
    duration: Duration,
    interest: Option<Interest>,
}

impl Timer {
    fn new(duration: Duration) -> Self {
        Self {
            started: Instant::now(),
            duration,
            interest: None,
        }
    }
}

impl Future for Timer {
    type Output = io::Result<()>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let timer = self.get_mut();
        let elapsed = timer.started.elapsed();
        timer.interest = None;
        if elapsed >= timer.duration {
            return Poll::Ready(Ok(()));
        }
        let result = (|| {
            let queue = Events::current(cx)?;
            // Each live registration owns a distinct fd. It supplies a timer
            // identity without a global counter or an address into a future.
            let ident = queue.0.as_raw_fd() as usize;
            let nanos = (timer.duration - elapsed)
                .as_nanos()
                .min(isize::MAX as u128)
                .max(1) as isize;
            Interest::new(
                queue,
                Event {
                    ident,
                    filter: EVFILT_TIMER,
                    flags: EV_ADD | EV_ONESHOT,
                    fflags: NOTE_NSECONDS,
                    data: nanos,
                    udata: ptr::null_mut(),
                },
            )
        })();
        match result {
            Ok(interest) => {
                timer.interest = Some(interest);
                Poll::Pending
            }
            Err(error) => Poll::Ready(Err(error)),
        }
    }
}

/// Wait for a duration measured from this call, using the entry point's event queue.
/// Dropping the future removes its timer. Use with `#[hibana_runtime::main]`.
pub fn sleep(duration: Duration) -> impl Future<Output = io::Result<()>> {
    Timer::new(duration)
}

struct Deadline<T, E> {
    timer: Timer,
    output: PhantomData<fn() -> (T, E)>,
}

impl<T, E: From<io::Error>> Future for Deadline<T, E> {
    type Output = Result<T, E>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        // Timer owns no pinned Rust address. T and E are result types only.
        Pin::new(&mut self.get_mut().timer)
            .poll(cx)
            .map(|result| match result {
                Ok(()) => Err(io::Error::from(io::ErrorKind::TimedOut).into()),
                Err(error) => Err(error.into()),
            })
    }
}

/// Return a timeout error after a duration. Race this with an operation:
/// `hibana_runtime::race(operation, deadline(duration)).await`.
/// The successful result and error types are inferred from the operation.
pub fn deadline<T, E: From<io::Error>>(duration: Duration) -> impl Future<Output = Result<T, E>> {
    Deadline {
        timer: Timer::new(duration),
        output: PhantomData,
    }
}

impl Driver for Events {
    fn waker(&self) -> Waker {
        // Each Waker owns a duplicated OS descriptor. Retained clones keep the
        // queue alive after this driver returns, without an application Arc or
        // an address pointing into scoped executor storage.
        // SAFETY: cloning duplicates this live owned descriptor before return.
        unsafe { Waker::from_raw(clone_waker(data(self.0.as_raw_fd()))) }
    }

    fn wait(&mut self) {
        loop {
            let mut event = Event::wake(0, 0);
            // SAFETY: one initialized output Event is writable for this call.
            // A null timeout waits until an event or an interrupt arrives.
            let count = unsafe {
                kevent(
                    self.0.as_raw_fd(),
                    ptr::null(),
                    0,
                    &mut event,
                    1,
                    ptr::null(),
                )
            };
            if count >= 0 {
                return;
            }
            let error = io::Error::last_os_error();
            assert_eq!(
                error.kind(),
                io::ErrorKind::Interrupted,
                "host wait: {error}"
            );
        }
    }
}

fn data(fd: RawFd) -> *const () {
    ptr::without_provenance(fd as usize)
}

fn descriptor(data: *const ()) -> RawFd {
    data.addr() as RawFd
}

unsafe fn clone_waker(value: *const ()) -> RawWaker {
    // SAFETY: this callback receives a descriptor owned by an existing Waker
    // (or a live Driver). The duplicate is owned independently and is closed on
    // exec, so starting a child process cannot retain an abandoned wake queue.
    let borrowed = unsafe { BorrowedFd::borrow_raw(descriptor(value)) };
    let owned = borrowed
        .try_clone_to_owned()
        .expect("clone host wake descriptor");
    RawWaker::new(data(owned.into_raw_fd()), &VTABLE)
}

unsafe fn wake_by_ref(value: *const ()) {
    let event = Event::wake(0, NOTE_TRIGGER);
    loop {
        // SAFETY: this Waker owns the queue descriptor until this call returns.
        // Triggering a retained queue with no driver is also valid.
        let result = unsafe {
            kevent(
                descriptor(value),
                &event,
                1,
                ptr::null_mut(),
                0,
                ptr::null(),
            )
        };
        if result == 0 {
            return;
        }
        let error = io::Error::last_os_error();
        assert_eq!(
            error.kind(),
            io::ErrorKind::Interrupted,
            "host wake: {error}"
        );
    }
}

unsafe fn drop_waker(value: *const ()) {
    // SAFETY: each raw Waker owns exactly one separately duplicated descriptor.
    drop(unsafe { OwnedFd::from_raw_fd(descriptor(value)) });
}

unsafe fn wake(value: *const ()) {
    // SAFETY: wake consumes this Waker after delivering its notification.
    unsafe {
        wake_by_ref(value);
        drop_waker(value);
    }
}

static VTABLE: RawWakerVTable = RawWakerVTable::new(clone_waker, wake, wake_by_ref, drop_waker);

pub(crate) fn block_on<F: core::future::Future>(future: F) -> F::Output {
    crate::__private::block_on(future, Events::new().expect("open host event queue"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn event_layout_matches_the_darwin_header() {
        assert_eq!(core::mem::size_of::<Event>(), 32);
        assert_eq!(core::mem::align_of::<Event>(), 4);
        assert_eq!(core::mem::offset_of!(Event, data), 16);
        assert_eq!(core::mem::offset_of!(Event, udata), 24);
    }

    #[test]
    fn wake_before_wait_is_retained_and_wakers_outlive_the_driver() {
        let wake = {
            let mut events = Events::new().unwrap();
            let wake = events.waker();
            wake.wake_by_ref();
            events.wait();
            wake
        };
        let retained = wake.clone();
        std::thread::spawn(move || {
            retained.wake();
            wake.wake_by_ref();
        })
        .join()
        .unwrap();
    }

    #[test]
    fn pending_timers_rebind_and_remove_each_registration_when_cancelled() {
        let first = Events::new().unwrap();
        let second = Events::new().unwrap();
        let wake_first = first.waker();
        let wake_second = second.waker();
        let mut timer = Timer::new(Duration::from_secs(3600));
        assert!(
            Pin::new(&mut timer)
                .poll(&mut Context::from_waker(&wake_first))
                .is_pending()
        );
        let ident_first = timer.interest.as_ref().unwrap().ident;
        assert!(
            Pin::new(&mut timer)
                .poll(&mut Context::from_waker(&wake_second))
                .is_pending()
        );
        let ident_second = timer.interest.as_ref().unwrap().ident;
        let deletion = |ident| Event {
            ident,
            filter: EVFILT_TIMER,
            flags: EV_DELETE,
            fflags: 0,
            data: 0,
            udata: ptr::null_mut(),
        };
        assert_eq!(
            first.change(deletion(ident_first)).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
        drop(timer);
        assert_eq!(
            second.change(deletion(ident_second)).unwrap_err().kind(),
            io::ErrorKind::NotFound
        );
    }

    #[test]
    fn a_pending_descriptor_read_can_move_between_host_drivers() {
        use std::{io::Write, os::unix::net::UnixStream};
        let first = Events::new().unwrap();
        let mut second = Events::new().unwrap();
        let wake_first = first.waker();
        let wake_second = second.waker();
        let (reader, mut writer) = UnixStream::pair().unwrap();
        let mut reader = Io::new(reader.into()).unwrap();
        let fd = std::os::fd::AsFd::as_fd(&reader).as_raw_fd();
        let mut bytes = [0; 1];
        let mut read = core::pin::pin!(reader.read(&mut bytes));
        assert!(
            read.as_mut()
                .poll(&mut Context::from_waker(&wake_first))
                .is_pending()
        );
        assert!(
            read.as_mut()
                .poll(&mut Context::from_waker(&wake_second))
                .is_pending()
        );
        assert_eq!(
            first
                .change(Event {
                    ident: fd as usize,
                    filter: EVFILT_READ,
                    flags: EV_DELETE,
                    fflags: 0,
                    data: 0,
                    udata: ptr::null_mut(),
                })
                .unwrap_err()
                .kind(),
            io::ErrorKind::NotFound
        );
        writer.write_all(&[9]).unwrap();
        second.wait();
        assert_eq!(
            read.as_mut()
                .poll(&mut Context::from_waker(&wake_second))
                .map(|r| r.unwrap()),
            Poll::Ready(1)
        );
    }

    #[test]
    fn pending_host_operations_reject_an_unrelated_driver() {
        use std::os::unix::net::UnixStream;
        let mut timer = Timer::new(Duration::from_secs(1));
        let mut cx = Context::from_waker(Waker::noop());
        assert_eq!(
            Pin::new(&mut timer)
                .poll(&mut cx)
                .map(|r| r.unwrap_err().kind()),
            Poll::Ready(io::ErrorKind::InvalidInput)
        );
        let (reader, _writer) = UnixStream::pair().unwrap();
        let mut reader = Io::new(reader.into()).unwrap();
        let mut bytes = [0; 1];
        let mut read = core::pin::pin!(reader.read(&mut bytes));
        assert_eq!(
            read.as_mut().poll(&mut cx).map(|r| r.unwrap_err().kind()),
            Poll::Ready(io::ErrorKind::InvalidInput)
        );
    }
}
