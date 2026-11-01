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
Do not make a per-branch `#[localside]` or `let ()` list the standard consumer
style. Inline concurrent work uses one function attribute and direct
`async { ... };` bodies containing the actual Endpoint operations. Bind needed
parallel results together; keep branch registration, result storage, pinning and
child completion inside the runtime. Retain explicit boundaries needed to consume
results or release borrows; do not add async adapters or scopes merely to conceal management code.
An ordinary `async { ... }.await?` is a valid completion and borrowing boundary.
A one-call async child is valid when it directly expresses independent work.
Do not invent syntax or duplicate execution APIs just to remove these Rust forms.
Each result declaration waits only for its explicitly listed futures. Never
merge result bindings or unrelated children because their declarations are adjacent.

Use one function attribute, `#[hibana_runtime::localside]`, with Rust async/await
as the basic interface. Inside that function, write independent branches directly:

```rust
async {
    let [mut a, mut b] = endpoint.par()?;

    async { a.send::<First>(&first).await?; };
    async { b.send::<Second>(&second).await?; };

    Ok(())
}.await?;
```

The runtime drives these children concurrently and finishes them before the outer
await completes. A single-call async child is valid. Keep Endpoint operations
visible; the runtime owns pinning, task handles, join construction and child
completion. Keep the existing declaration grammar; do not add custom concurrency
syntax to replace these Rust forms.

Declare repeated participants from their actual fixed arrays. Do not make
consumers construct task collections, spawn handles, polling loops or result
collection loops. Ordinary unmarked Rust loops keep their sequential meaning.

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
Publish verified source with `[skip ci]`; run the gates locally while Actions
budget is unavailable. Do not use development subagents.
