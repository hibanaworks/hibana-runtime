# hibana-runtime

A small local executor for Hibana localsides. Rust 2024, unconditional
`no_std`, no allocator, and no normal dependencies.

Write each independent localside as an async function. Register its task once
at startup. Keep `send`, `recv`, and `offer` in that function, and protocol
dependencies in the global choreography.

```rust
use core::pin::pin;
use hibana_runtime::{Executor, Task};

// Integration has already attached the endpoints and supplied the driver.
let mut owner = pin!(Task::new(owner_localside(owner_endpoint)));
let mut worker = pin!(Task::new(worker_localside(worker_endpoint)));

Executor::new([
    owner.as_mut().register(),
    worker.as_mut().register(),
]).run(&waker, wait)?;
```

`Task` owns the future inline. Pinning its caller-owned storage supports
self-borrowing async state, scoped borrows, and `!Send` localsides. The
registration borrows the pinned task exclusively and owns cancellation. The
array determines the exact task count; there is no pool capacity to guess.
Every localside returns `Result<(), E>` with the same error type.

The executor can also be awaited as a `Future`. `run` is the standalone entry
point when the integration supplies a Waker and a wake-preserving wait function.

## Scheduling and lifetime

Each executor poll visits every live task at most once. The first slot rotates
between sweeps. A pending task does not prevent another task from running.
Completion immediately drops that task's future in place and removes its
registration. The first error drops every remaining future before returning;
dropping an unfinished executor also cancels all its tasks.

All live tasks receive the current driver Waker. A wake requests another sweep;
wakes may coalesce. Each sweep costs O(task count), with O(task count) pinned
references and no ready queue or per-task atomic storage. This deliberate tradeoff
keeps the same implementation usable on Cortex-M0+ and hosts. Task Wakers never
point into the scoped executor, so retaining them after cancellation is safe
under the driver's ordinary Waker contract.

`wait` must retain a wake that arrives before it begins waiting. Thread
`park`/`unpark` tokens or a properly integrated board event latch provide that
handshake. The board or host supplies interrupt/I/O readiness and timers.
The executor contains no device driver, network stack, timer service, thread
pool, or protocol progress state.

Concurrency is cooperative: each `poll` must return. Independent computations
can run in separate tasks and yield through futures; a long synchronous
computation inside one poll blocks that executor. The runtime does not split
one sequential async function into independent work.

Cancellation drops local future state. It does not undo transport acceptance,
restore a delegated endpoint, or establish that a remote peer has stopped.
Those guarantees remain at Hibana and its integration boundary.

## Run real localsides

```sh
cargo +stable run --example localsides
```

The [example](examples/localsides.rs) registers four separate localsides. Its
global contains two independent request/reply arms under `g::par`; one arm uses
`g::route`, and the receiver handles both choices with `offer()`. Receivers are
registered first, so both arms initially wait before senders run. The bounded
loopback carrier and atomic wake latch allocate no memory. The closed simulation
fails if it stalls without a wake; it has no external I/O to wait for.

Hibana `0.9.9` is an exact **dev dependency** for the runnable example. The
executor library is independent of the endpoint representation and has no
Hibana dependency. No Hibana or QUIC source is changed by this repository.
The example checks ordinary communication and route choices, not completion
of Hibana's separate owned-endpoint delegation work.

## Verify

```sh
python3 tools/validate.py
```

This runs checks sequentially under the shared
[resource guard](tools/resource_guard.py). It monitors aggregate descendant RSS
and kills the job at 3,000,000,000 bytes. Reports and full logs go in
`validation/`; a resource stop is a failed, unverified check. Each checkout uses
its own target directory. The report fingerprints the checked sources and records
compiler versions.

Prerequisites are current stable Rust with rustfmt, clippy, and
`thumbv6m-none-eabi`, Z3, and Lean `v4.34.1`. Miri uses
`nightly-2026-10-12` with its `miri` and `rust-src` components. This compiler is
only for Miri; ordinary development and target checks use updated stable.

Tests exercise scoped `!Send` and self-borrowing futures, simultaneous pending
tasks, fairness, completion, failure and cancellation, pin stability, Waker
replacement/retention, cross-thread wakeups, and a counting allocator around
construction, registration, execution, and cancellation. Rustdoc includes an
actual compile-fail check for duplicate registration.

[Lean](proofs/Scheduler.lean) and [Z3](proofs/scheduler.smt2) establish scoped
scheduler arithmetic and live-slot removal properties in independent models.
They do not prove the Rust implementation, Waker/pin safety, protocol progress,
or arbitrary Rust computation termination. Executable tests and Miri check the
Rust task lifetime implementation. Kani is not required for a second model of
the same small scheduler loop.

The [Pico link probe](examples/pico.rs) links two tasks without `std` or an
allocator against the RP2040 capacity map. It uses SEV/WFE for wake-before-wait
retention and exports the task/executor storage sizes. It has no boot block,
vector table, or board setup, so it is a sizing probe rather than flashable
firmware. Actual board execution, interrupt-driver integration, and whole
application stack high-water measurement remain deployment checks.
