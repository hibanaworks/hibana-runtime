//! The host event queue owns wake delivery; localsides keep their inline storage.

#[cfg(not(target_os = "macos"))]
compile_error!("the host driver currently supports macOS; use main(driver) on other platforms");

use crate::Driver;
use core::{
    ffi::{c_int, c_void},
    ptr,
    task::{RawWaker, RawWakerVTable, Waker},
};
use std::{
    io,
    os::fd::{AsRawFd, BorrowedFd, FromRawFd, IntoRawFd, OwnedFd, RawFd},
};

const EVFILT_USER: i16 = -10;
const EV_ADD: u16 = 0x1;
const EV_CLEAR: u16 = 0x20;
const NOTE_TRIGGER: u32 = 0x0100_0000;
const WAKE: usize = 1;

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
}
