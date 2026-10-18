//! Independent localsides are registered once; Hibana owns protocol progress.
//!
//! Run with `cargo run --example localsides`. Neither the tasks nor the carrier
//! allocate. Printing is the only host service used by this example.

#[path = "support/loopback.rs"]
mod loopback;
#[path = "support/wake_latch.rs"]
mod wake_latch;

use core::cell::Cell;

use hibana::{
    Endpoint, EndpointError,
    g::{self, Msg},
    runtime::{SessionKitStorage, ids::SessionId, program::project},
};
use hibana_runtime::localside;
use loopback::Loopback;
use wake_latch::WakeLatch;

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

async fn left(
    mut endpoint: Endpoint<'_, 0>,
    alternate: bool,
    result: &Cell<u32>,
) -> Result<(), EndpointError> {
    let reply = if alternate {
        endpoint.send::<AlternatePing>(&70).await?;
        endpoint.recv::<AlternatePong>().await?
    } else {
        endpoint.send::<Ping>(&7).await?;
        endpoint.recv::<Pong>().await?
    };
    result.set(reply);
    Ok(())
}

async fn left_peer(mut endpoint: Endpoint<'_, 1>) -> Result<(), EndpointError> {
    let branch = endpoint.offer().await?;
    match branch.label() {
        1 => {
            let value = branch.recv::<Ping>().await?;
            endpoint.send::<Pong>(&(value + 1)).await?;
        }
        3 => {
            let value = branch.recv::<AlternatePing>().await?;
            endpoint.send::<AlternatePong>(&(value + 1)).await?;
        }
        _ => unreachable!("the global has exactly two route arms"),
    }
    Ok(())
}

async fn right(mut endpoint: Endpoint<'_, 2>, result: &Cell<u32>) -> Result<(), EndpointError> {
    endpoint.send::<Read>(&42).await?;
    result.set(endpoint.recv::<Value>().await?);
    Ok(())
}

async fn right_peer(mut endpoint: Endpoint<'_, 3>) -> Result<(), EndpointError> {
    let key = endpoint.recv::<Read>().await?;
    endpoint.send::<Value>(&(key * 2)).await?;
    Ok(())
}

#[localside]
async fn run(alternate: bool) -> Result<(), EndpointError> {
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
    let left_endpoint = rendezvous.enter(session, &left_program).expect("left");
    let left_peer_endpoint = rendezvous
        .enter(session, &left_peer_program)
        .expect("left peer");
    let right_endpoint = rendezvous.enter(session, &right_program).expect("right");
    let right_peer_endpoint = rendezvous
        .enter(session, &right_peer_program)
        .expect("right peer");

    let left_value = Cell::new(0);
    let right_value = Cell::new(0);

    // These declarations are driven automatically in their lexical scope.
    // Both receivers can wait without preventing either sender from running.
    {
        async { left_peer(left_peer_endpoint).await };
        async { right_peer(right_peer_endpoint).await };
        async { left(left_endpoint, alternate, &left_value).await };
        async { right(right_endpoint, &right_value).await };
    }

    assert_eq!(left_value.get(), if alternate { 71 } else { 8 });
    assert_eq!(right_value.get(), 84);
    println!(
        "alternate={alternate}, left={}, right={}",
        left_value.get(),
        right_value.get(),
    );
    Ok(())
}

#[hibana_runtime::main(WakeLatch::new())]
async fn main() -> Result<(), EndpointError> {
    run(false).await?;
    run(true).await
}
