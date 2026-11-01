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
    for update in [false, true] {
        // Physical transport, storage and initial issuance are bootstrap work.
        let mut slab = [0_u8; 16 * 1024];
        let mut storage = SessionKitStorage::<Carrier>::uninit();
        let kit = storage.init();
        let rendezvous = kit
            .rendezvous(&mut slab, Carrier::new())
            .expect("rendezvous");
        let session = SessionId::new(1);
        // These roles are inferred from the localside signatures below.
        let client = rendezvous
            .enter(session, &project(&global))
            .expect("attach");
        let server = rendezvous
            .enter(session, &project(&global))
            .expect("attach");
        let auditor = rendezvous
            .enter(session, &project(&global))
            .expect("attach");
        let store = rendezvous
            .enter(session, &project(&global))
            .expect("attach");
        let response = Cell::new(0);
        let count = Cell::new(0);

        async {
            #[concurrent]
            sides::server(server);
            #[concurrent]
            sides::store(store);
            #[concurrent]
            sides::client(client, update, &response);
            #[concurrent]
            sides::auditor(auditor, &count);
            Ok(())
        }
        .await?;
        assert_eq!(response.get(), if update { 71 } else { 8 });
        assert_eq!(count.get(), 84);
        println!(
            "update={update}, client={}, auditor={}",
            response.get(),
            count.get()
        );
    }
    Ok(())
}
