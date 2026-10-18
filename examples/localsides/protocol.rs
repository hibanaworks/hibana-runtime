//! The sole communication specification. No execution or physical bindings.
use hibana::{g, runtime::program::Projectable};

pub const CLIENT: u8 = 0;
pub const SERVER: u8 = 1;
pub const AUDITOR: u8 = 2;
pub const STORE: u8 = 3;

pub const FETCH: u8 = 1;
pub const UPDATE: u8 = 3;

pub type Fetch = g::Msg<FETCH, u32>;
pub type Fetched = g::Msg<2, u32>;
pub type Update = g::Msg<UPDATE, u32>;
pub type Updated = g::Msg<4, u32>;
pub type Inspect = g::Msg<5, u32>;
pub type Count = g::Msg<6, u32>;

// A separate, zero-sized definition. `impl Projectable` infers its complete
// choreography type without a second handwritten type-level specification.
pub const fn global() -> impl Projectable {
    g::par(
        g::route(
            g::seq(
                g::send::<CLIENT, SERVER, Fetch>(),
                g::send::<SERVER, CLIENT, Fetched>(),
            ),
            g::seq(
                g::send::<CLIENT, SERVER, Update>(),
                g::send::<SERVER, CLIENT, Updated>(),
            ),
        ),
        g::seq(
            g::send::<AUDITOR, STORE, Inspect>(),
            g::send::<STORE, AUDITOR, Count>(),
        ),
    )
}
