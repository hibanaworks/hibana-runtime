//! Global definition, projection/attachment, and localside bodies have distinct
//! homes. Run with `cargo +stable run --example localsides`.
#[path = "support/loopback.rs"]
mod loopback;
#[path = "support/wake_latch.rs"]
mod wake_latch;
mod localsides {
    pub mod protocol;
    pub mod sides;
}

use core::cell::Cell;
use hibana::{
    EndpointError,
    runtime::{SessionKitStorage, ids::SessionId, program::project},
};
use localsides::{protocol, sides};
use loopback::Loopback;
use wake_latch::WakeLatch;

// Each selected trace has four sends and four potentially waiting roles.
type Carrier = Loopback<4, 4>;

#[hibana_runtime::main(WakeLatch::new())]
async fn main() -> Result<(), EndpointError> {
    let global = protocol::global();
    // Projection is explicit and happens before Endpoint issuance/execution.
    let left_program = project::<0>(&global);
    let left_peer_program = project::<1>(&global);
    let right_program = project::<2>(&global);
    let right_peer_program = project::<3>(&global);

    for alternate in [false, true] {
        // Physical transport, storage and initial issuance are bootstrap work.
        let mut slab = [0_u8; 16 * 1024];
        let mut storage = SessionKitStorage::<Carrier>::uninit();
        let kit = storage.init();
        let rendezvous = kit
            .rendezvous(&mut slab, Carrier::new())
            .expect("rendezvous");
        let session = SessionId::new(1);
        let left = rendezvous.enter(session, &left_program).expect("left");
        let left_peer = rendezvous
            .enter(session, &left_peer_program)
            .expect("left peer");
        let right = rendezvous.enter(session, &right_program).expect("right");
        let right_peer = rendezvous
            .enter(session, &right_peer_program)
            .expect("right peer");
        let left_value = Cell::new(0);
        let right_value = Cell::new(0);

        {
            async { sides::left_peer(left_peer).await };
            async { sides::right_peer(right_peer).await };
            async { sides::left(left, alternate, &left_value).await };
            async { sides::right(right, &right_value).await };
        }
        assert_eq!(left_value.get(), if alternate { 71 } else { 8 });
        assert_eq!(right_value.get(), 84);
        println!(
            "alternate={alternate}, left={}, right={}",
            left_value.get(),
            right_value.get()
        );
    }
    Ok(())
}
