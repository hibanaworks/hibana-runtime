//! The sole communication specification. No execution or physical bindings.
use hibana::{g, runtime::program::Projectable};

pub type Ping = g::Msg<1, u32>;
pub type Pong = g::Msg<2, u32>;
pub type AlternatePing = g::Msg<3, u32>;
pub type AlternatePong = g::Msg<4, u32>;
pub type Read = g::Msg<5, u32>;
pub type Value = g::Msg<6, u32>;

// A separate, zero-sized definition. `impl Projectable` infers its complete
// choreography type without a second handwritten type-level specification.
pub const fn global() -> impl Projectable {
    g::par(
        g::route(
            g::seq(g::send::<0, 1, Ping>(), g::send::<1, 0, Pong>()),
            g::seq(
                g::send::<0, 1, AlternatePing>(),
                g::send::<1, 0, AlternatePong>(),
            ),
        ),
        g::seq(g::send::<2, 3, Read>(), g::send::<3, 2, Value>()),
    )
}
