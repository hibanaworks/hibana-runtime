# Execution rules

Hibana's global and kernel own protocol order, branch knowledge and Endpoint
ownership. This crate owns only future storage, pinning, polling and cancellation.
Do not interpret the global again or add an owner table or protocol cursor here.

Do not add extra structs, enums, managers, registries or wrappers without a
concrete data-ownership need that existing types, borrows and moves cannot
express. Never add another source of protocol progress, permission or completion.
Keep send/recv/offer/par, branches, loops and concurrent completion boundaries
visible in the participant's localside body; do not hide protocol steps in
helpers or generated workflow drivers. Codecs, buffers and actual I/O do not
become another coordination specification.

Consumers must be rewritten for this runtime, not mechanically translated from
Tokio. Support direct participant declarations, ordinary borrowed and owned
arguments, and each localside's complete operation flow. Do not require startup
join trees, task/result tables, duplicate completion checks, application
schedulers or conversion wrappers. Fix this runtime when direct localside code
needs an adapter. I/O participants own their data and final I/O; the runtime
owns polling, cancellation and waiting for children.
Use one function attribute, `#[hibana_runtime::scope]`, and ordinary Rust async
and await. Write each independent branch directly as `async { ... };`, including
a one-call branch. Do not add declaration attributes, task handles, result
receivers, per-result wakers, join trees or tuple-only await syntax.

Hibana's global and Endpoint::par determine protocol independence and joins.
The runtime polls the entire async child, including computation after its final
Endpoint operation. Dropping a fragment does not await its Rust computation.

An async body ending in child declarations returns their successful values:
one child returns its value; several return a tuple in declaration order. A
fixed-array loop containing one async child returns an array of the actual N.
Explicit parent success returns its own value after finishing registered
children. Parent effects between declarations do not regroup child results.
Errors and cancellation destroy unfinished futures in place and drop completed
values once. Stored futures remain lazy; ordinary loops remain sequential.
Use an async expression with `.await?` when values merge or borrows must end:

```rust
let values = async {
    let [mut a, mut b] = endpoint.par()?;
    async { a.recv::<First>().await };
    async { b.recv::<Second>().await };
}.await?;
```

The same async scope is an ordinary Future when stored or returned. Fixed arrays
use `for value in values { async move { ... }; }` inside that scope; do not
require task collections, spawn handles or result collection loops. Closures,
nested functions and ordinary blocks do not acquire an implicit async boundary.

Application localsides must not need `pin!`, `Box::pin`, `Pin` arguments or
manual stack-size configuration. Keep each declared future in one final inline
slot; an async adapter must not retain another copy of its input. Check future
layout and actual consumer execution when changing storage. A larger stack is
not a fix for duplicate storage. Pinning required by Rust's Future contract
stays inside the runtime, with in-place polling and destruction verified by Miri.

Keep the execution core `no_std`, allocation-free and caller-stored. Host support
uses the standard library and operating-system interfaces. Do not add Tokio or
another async framework, executor, reactor or channel crate. Syntax parsing for
procedural macros stays on the build host, outside generated execution code.

Do not introduce application progress flags, permission wrappers, completion
mailboxes or localside workarounds for legal globals. Return values are data;
the scheduler must manage their lifetimes without requiring application flags.
Cancellation drops unfinished futures in place and never restores transferred
Endpoint ownership or rolls back accepted I/O. Retained Wakers must remain valid
after their scoped futures and driver have gone away.

Use current stable Rust and retain `forbid(long_running_const_eval)`. Verify
borrow and move errors with actual compiler checks. Use Miri for pinning and
cancellation, real host tests for OS wake delivery, and no-allocation tests for
the execution path. Scheduler arithmetic proofs do not prove macro refinement.

Run `tools/check.sh` with the shared descendant-memory guard. One heavy workload
at a time; retain source hashes and failed verification. Do not use Docker or a
VM, raise the memory limit, or weaken a test to obtain a passing result.
Run the gates locally and in GitHub Actions on the published revision. Actions
is available again; do not skip required checks because of the former budget
limit. Distinguish queued, running and successful checks. Do not use development subagents.
