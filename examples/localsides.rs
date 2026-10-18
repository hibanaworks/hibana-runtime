//! Independent localsides are registered once; Hibana owns protocol progress.
//!
//! Run with `cargo run --example localsides`. Neither the tasks nor the carrier
//! allocate. Printing is the only host service used by this example.

#[path = "support/loopback.rs"]
mod loopback;

use core::{
    cell::Cell,
    pin::pin,
    ptr,
    sync::atomic::{AtomicBool, Ordering},
    task::{RawWaker, RawWakerVTable, Waker},
};

use hibana::{
    EndpointError,
    g::{self, Msg},
    runtime::{SessionKitStorage, ids::SessionId, program::project},
};
use hibana_runtime::{Executor, Task};
use loopback::Loopback;

type Ping = Msg<1, u32>;
type Pong = Msg<2, u32>;
type AlternatePing = Msg<3, u32>;
type AlternatePong = Msg<4, u32>;
type Read = Msg<5, u32>;
type Value = Msg<6, u32>;

// Each selected trace has four sends and at most four pending receivers.
const MESSAGE_COUNT: usize = 4;
const LOCALSIDE_COUNT: usize = 4;
type Carrier = Loopback<MESSAGE_COUNT, LOCALSIDE_COUNT>;

// This closed example has no external event producer. Keep an atomic wake
// token so a wake during a scheduler sweep survives until its wait operation.
static READY: AtomicBool = AtomicBool::new(false);

fn raw_waker() -> RawWaker {
    RawWaker::new(ptr::null(), &WAKER_VTABLE)
}

fn clone_waker(_: *const ()) -> RawWaker {
    raw_waker()
}

fn wake(_: *const ()) {
    READY.store(true, Ordering::Release);
}

fn drop_waker(_: *const ()) {}

static WAKER_VTABLE: RawWakerVTable = RawWakerVTable::new(clone_waker, wake, wake, drop_waker);

fn run(alternate: bool) {
    READY.store(false, Ordering::Release);
    let global = g::par(
        g::route(
            g::seq(g::send::<0, 1, Ping>(), g::send::<1, 0, Pong>()),
            g::seq(
                g::send::<0, 1, AlternatePing>(),
                g::send::<1, 0, AlternatePong>(),
            ),
        ),
        g::seq(g::send::<2, 3, Read>(), g::send::<3, 2, Value>()),
    );
    let left_program = project::<0>(&global);
    let left_peer_program = project::<1>(&global);
    let right_program = project::<2>(&global);
    let right_peer_program = project::<3>(&global);

    // This example's explicit Hibana workspace budget. Production integrations
    // size their own descriptor set and caller-provided storage.
    let mut slab = [0_u8; 16 * 1024];
    let mut storage = SessionKitStorage::<Carrier>::uninit();
    let kit = storage.init();
    let rendezvous = kit
        .rendezvous(&mut slab, Carrier::new())
        .expect("create rendezvous");
    let session = SessionId::new(1);
    let mut left = rendezvous.enter(session, &left_program).expect("left");
    let mut left_peer = rendezvous
        .enter(session, &left_peer_program)
        .expect("left peer");
    let mut right = rendezvous.enter(session, &right_program).expect("right");
    let mut right_peer = rendezvous
        .enter(session, &right_peer_program)
        .expect("right peer");

    let left_value = Cell::new(0);
    let right_value = Cell::new(0);

    let mut left_task = pin!(Task::new(async {
        let reply = if alternate {
            left.send::<AlternatePing>(&70).await?;
            left.recv::<AlternatePong>().await?
        } else {
            left.send::<Ping>(&7).await?;
            left.recv::<Pong>().await?
        };
        left_value.set(reply);
        Ok::<(), EndpointError>(())
    }));

    let mut left_peer_task = pin!(Task::new(async {
        let branch = left_peer.offer().await?;
        match branch.label() {
            1 => {
                let value = branch.recv::<Ping>().await?;
                left_peer.send::<Pong>(&(value + 1)).await?;
            }
            3 => {
                let value = branch.recv::<AlternatePing>().await?;
                left_peer.send::<AlternatePong>(&(value + 1)).await?;
            }
            _ => unreachable!("the global has exactly two route arms"),
        }
        Ok::<(), EndpointError>(())
    }));

    let mut right_task = pin!(Task::new(async {
        right.send::<Read>(&42).await?;
        right_value.set(right.recv::<Value>().await?);
        Ok::<(), EndpointError>(())
    }));

    let mut right_peer_task = pin!(Task::new(async {
        let key = right_peer.recv::<Read>().await?;
        right_peer.send::<Value>(&(key * 2)).await?;
        Ok::<(), EndpointError>(())
    }));

    // Register receivers first so both independent arms really park before
    // their senders run. Each receive installs its own task waker.
    let executor = Executor::new([
        left_peer_task.as_mut().register(),
        right_peer_task.as_mut().register(),
        left_task.as_mut().register(),
        right_task.as_mut().register(),
    ]);
    // SAFETY: the callbacks only access the static atomic latch, never the
    // unused data pointer. Clones remain valid on any thread for any lifetime.
    let waker = unsafe { Waker::from_raw(raw_waker()) };
    executor
        .run(&waker, || {
            assert!(
                READY.swap(false, Ordering::AcqRel),
                "the closed choreography stalled without a wake"
            );
        })
        .expect("complete all localsides");

    assert_eq!(left_value.get(), if alternate { 71 } else { 8 });
    assert_eq!(right_value.get(), 84);
    println!(
        "alternate={alternate}, left={}, right={}",
        left_value.get(),
        right_value.get(),
    );
}

fn main() {
    run(false);
    run(true);
}
