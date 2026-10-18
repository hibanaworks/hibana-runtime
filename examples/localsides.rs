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
    for alternate in [false, true] {
        // Physical transport, storage and initial issuance are bootstrap work.
        let mut slab = [0_u8; 16 * 1024];
        let mut storage = SessionKitStorage::<Carrier>::uninit();
        let kit = storage.init();
        let rendezvous = kit
            .rendezvous(&mut slab, Carrier::new())
            .expect("rendezvous");
        let session = SessionId::new(1);
        // These roles are inferred from the localside signatures below.
        let left = rendezvous
            .enter(session, &project(&global))
            .expect("attach");
        let left_peer = rendezvous
            .enter(session, &project(&global))
            .expect("attach");
        let right = rendezvous
            .enter(session, &project(&global))
            .expect("attach");
        let right_peer = rendezvous
            .enter(session, &project(&global))
            .expect("attach");
        let left_value = Cell::new(0);
        let right_value = Cell::new(0);

        {
            #[localside]
            sides::left_peer(left_peer);
            #[localside]
            sides::right_peer(right_peer);
            #[localside]
            sides::left(left, alternate, &left_value);
            #[localside]
            sides::right(right, &right_value);
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
