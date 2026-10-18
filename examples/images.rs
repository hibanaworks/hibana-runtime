//! The same global and localsides on two physical host execution places.
//! Each place hosts two roles. No Future or Endpoint pointer crosses the socket.
#[cfg(unix)]
#[path = "support/unix_image.rs"]
mod unix_image;
#[cfg(unix)]
mod localsides {
    pub mod protocol;
    pub mod sides;
}

#[cfg(unix)]
mod host {
    use super::{
        localsides::{protocol, sides},
        unix_image::{Events, ImageIo},
    };
    use core::cell::Cell;
    use hibana::{
        Endpoint, EndpointError,
        runtime::{SessionKitStorage, ids::SessionId, program::project},
    };
    use std::{
        os::{fd::AsFd, unix::net::UnixDatagram},
        process::{Command, Stdio},
        thread,
    };
    type Outcome = Result<(), EndpointError>;

    #[hibana_runtime::main(events)]
    async fn left(
        (left, right): (Endpoint<'_, 0>, Endpoint<'_, 2>),
        (alternate, values): (bool, &[Cell<u32>; 2]),
        events: Events,
    ) -> Outcome {
        #[localside]
        sides::left(left, alternate, &values[0]);
        #[localside]
        sides::right(right, &values[1]);
        Ok(())
    }

    #[hibana_runtime::main(events)]
    async fn right(endpoints: (Endpoint<'_, 1>, Endpoint<'_, 3>), events: Events) -> Outcome {
        #[localside]
        sides::left_peer(endpoints.0);
        #[localside]
        sides::right_peer(endpoints.1);
        Ok(())
    }

    fn image(is_left: bool, socket: UnixDatagram, alternate: bool) {
        // All local-only kernel state, borrows and futures are constructed in
        // their destination thread/process, with storage owned by that image.
        let io = ImageIo::new(socket);
        let events = io.events();
        let global = protocol::global();
        let mut slab = [0; 16 * 1024];
        let mut storage = SessionKitStorage::<ImageIo>::uninit();
        let kit = storage.init();
        let rendezvous = kit.rendezvous(&mut slab, io).unwrap();
        let session = SessionId::new(1);
        if is_left {
            let values = [Cell::new(0), Cell::new(0)];
            let endpoints = (
                rendezvous.enter(session, &project(&global)).unwrap(),
                rendezvous.enter(session, &project(&global)).unwrap(),
            );
            left(endpoints, (alternate, &values), events).unwrap();
            assert_eq!(values[0].get(), if alternate { 71 } else { 8 });
            assert_eq!(values[1].get(), 84);
        } else {
            let endpoints = (
                rendezvous.enter(session, &project(&global)).unwrap(),
                rendezvous.enter(session, &project(&global)).unwrap(),
            );
            right(endpoints, events).unwrap();
        }
    }

    pub fn main() {
        // Check the actual readiness driver's wake-before-wait handshake and
        // the lifetime of a wake handle retained after its driver is gone.
        {
            use hibana_runtime::Driver;
            let (socket, _peer) = UnixDatagram::pair().unwrap();
            let io = ImageIo::new(socket);
            let mut events = io.events();
            let wake = events.waker();
            wake.wake_by_ref();
            events.wait();
            drop(events);
            thread::spawn(move || wake.wake()).join().unwrap();
        }
        let args: Vec<_> = std::env::args().collect();
        if args.get(1).is_some_and(|arg| arg == "--image") {
            let socket = UnixDatagram::from(std::io::stdin().as_fd().try_clone_to_owned().unwrap());
            image(args[2] == "left", socket, args[3] == "alternate");
            println!("{} pid={} complete", args[2], std::process::id());
            return;
        }
        for alternate in [false, true] {
            let (a, b) = UnixDatagram::pair().unwrap();
            thread::scope(|scope| {
                let left = scope.spawn(move || image(true, a, alternate));
                let right = scope.spawn(move || image(false, b, alternate));
                left.join().unwrap();
                right.join().unwrap();
            });
            println!("two OS threads, roles 0+2 / 1+3, alternate={alternate}: passed");

            let (a, b) = UnixDatagram::pair().unwrap();
            let executable = std::env::current_exe().unwrap();
            let spawn = |side, socket: UnixDatagram| {
                Command::new(&executable)
                    .args([
                        "--image",
                        side,
                        if alternate { "alternate" } else { "primary" },
                    ])
                    .stdin(Stdio::from(std::os::fd::OwnedFd::from(socket)))
                    .stdout(Stdio::piped())
                    .stderr(Stdio::piped())
                    .spawn()
                    .unwrap()
            };
            let left = spawn("left", a);
            let right = spawn("right", b);
            assert_ne!(left.id(), right.id());
            let results = [
                left.wait_with_output().unwrap(),
                right.wait_with_output().unwrap(),
            ];
            for result in results {
                assert!(
                    result.status.success(),
                    "{}",
                    String::from_utf8_lossy(&result.stderr)
                );
                print!("{}", String::from_utf8_lossy(&result.stdout));
            }
            println!("two OS processes, same global and localsides, alternate={alternate}: passed");
        }
    }
}

#[cfg(unix)]
fn main() {
    host::main();
}
#[cfg(not(unix))]
fn main() {
    println!("This physical-image example uses Unix datagrams.");
}
