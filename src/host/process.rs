use super::{EV_ADD, EV_ONESHOT, Event, Events, Interest, Io};
use core::{future::poll_fn, ptr, task::Poll};
use std::{
    io,
    process::{Child, Command, ExitStatus},
};

const EVFILT_PROC: i16 = -5;
const NOTE_EXIT: u32 = 0x8000_0000;

/// An owned host child process. Dropping it terminates and reaps it.
///
/// The I/O participant owns this handle and its pipes. Waiting uses the host
/// entry point's event queue. Cancelling a wait leaves the process alive;
/// cancelling its owning localside drops the handle and terminates the child.
pub struct Process {
    child: Child,
}

impl Process {
    /// Start the command with its configured arguments, environment and I/O.
    pub fn spawn(command: &mut Command) -> io::Result<Self> {
        Ok(Self {
            child: command.spawn()?,
        })
    }

    /// Take the configured piped stdin and stdout as independent I/O handles.
    /// The first handle writes to the child; the second reads its output.
    pub fn io(&mut self) -> io::Result<(Io, Io)> {
        let input = self.child.stdin.take().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "child stdin is not piped or was already taken",
            )
        })?;
        let output = self.child.stdout.take().ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::InvalidInput,
                "child stdout is not piped or was already taken",
            )
        })?;
        Ok((Io::new(input.into())?, Io::new(output.into())?))
    }

    /// The operating system's process identifier.
    pub fn id(&self) -> u32 {
        self.child.id()
    }

    /// Wait for exit without blocking the other localsides. Close an untaken
    /// stdin first, as `std::process::Child::wait` does.
    pub async fn wait(&mut self) -> io::Result<ExitStatus> {
        drop(self.child.stdin.take());
        let mut interest = None;
        poll_fn(|cx| {
            interest = None;
            if let Some(status) = self.child.try_wait()? {
                return Poll::Ready(Ok(status));
            }
            let registration = Events::current(cx).and_then(|queue| {
                Interest::new(
                    queue,
                    Event {
                        ident: self.child.id() as usize,
                        filter: EVFILT_PROC,
                        flags: EV_ADD | EV_ONESHOT,
                        fflags: NOTE_EXIT,
                        data: 0,
                        udata: ptr::null_mut(),
                    },
                )
            });
            // Exit can race registration. Reap a completed child even when
            // registration reports that its process no longer exists.
            if let Some(status) = self.child.try_wait()? {
                if let Ok(registration) = registration {
                    interest = Some(registration);
                }
                return Poll::Ready(Ok(status));
            }
            match registration {
                Ok(registration) => {
                    interest = Some(registration);
                    Poll::Pending
                }
                Err(error) if error.raw_os_error() == Some(3) => {
                    // Darwin can stop accepting EVFILT_PROC during exit before
                    // waitpid can reap the child. No future event can then be
                    // registered; poll again to collect its actual exit status.
                    cx.waker().wake_by_ref();
                    Poll::Pending
                }
                Err(error) => Poll::Ready(Err(error)),
            }
        })
        .await
    }

    /// Terminate a running child and reap it. An already exited child keeps its
    /// original exit status. This also closes any untaken stdin.
    pub fn terminate(&mut self) -> io::Result<ExitStatus> {
        if let Some(status) = self.child.try_wait()? {
            return Ok(status);
        }
        if let Err(error) = self.child.kill() {
            if let Some(status) = self.child.try_wait()? {
                return Ok(status);
            }
            return Err(error);
        }
        self.child.wait()
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        self.terminate()
            .expect("terminate and reap owned host child");
    }
}
