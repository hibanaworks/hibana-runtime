# hibana-runtime

Write localsides and their effects. An attribute supplies execution: no task
builder, `spawn`, `join!`, `select!`, user pinning, or completion handles.
Rust 2024, unconditional `no_std`, no runtime allocation, caller-owned storage.

```rust
use hibana_runtime::localside;

#[localside]
async fn worker(mut worker: Endpoint<'_, WORKER>) -> Result<(), EndpointError> {
    let mut db = worker.recv::<Loan>().await?;
    let mut auth = worker.recv::<AuthLoan>().await?;
    {
        async {
            db.send::<Read>(&42).await?;
            let value = db.recv::<Value>().await?;
            record_value(value); // ordinary application effects
        };
        async {
            auth.send::<Check>(&7).await?;
            let allowed = auth.recv::<Allowed>().await?;
            record_permission(allowed);
        };
    }
    worker.send::<Loan>(db).await?;
    worker.send::<AuthLoan>(auth).await
}
```

This is the implemented declaration syntax. The owned-Endpoint example above
requires the development core's unfinished delegation work; it is not a claim
that published Hibana has completed delegation. The runnable
[ordinary Endpoint example](examples/localsides.rs) uses published Hibana 0.9.9.
The [owned integration fixture](fixtures/owned_views.rs) runs actual Endpoints
against an isolated development-core copy, at its existing internal image-test
boundary. The public owned-projection guard remains enabled.

## Declaration and scope

In a `#[localside]` async function, a standalone `async { ... };` or
`async move { ... };` statement declares an independent localside. It runs with
the rest of its enclosing block. A pending child does not prevent later
statements or other children from progressing. A later declaration starts
when execution reaches it; it is not hoisted over earlier effects or awaits.

The block owns its children and finishes them before a successful exit,
including `return Ok(...)`, `break`, and `continue`. Closing the block releases
their borrows, so owned views can then be sent back. A child error, a parent
`?`/returned error, or dropping the unfinished enclosing future cancels the
unfinished children in place. There are no detached tasks. An error is not a
rollback of accepted communication or delegated ownership.

Functions return `Result<T, E>` (including aliases). A child statement body
needs no final `Ok(())`; a child with an explicit tail returns `Result<(), E>`. Function values, explicit `.await`, stored async values, and
closures retain their usual Rust meaning. The attribute does not split a
sequential function or infer independence from arbitrary Rust computation.
Nested functions have their own attribute boundary.

Application effects and localside composition stay in the source. Global
choreography and Hibana's existing projection/kernel remain the single source
of protocol ordering, route decisions, ownership, and permitted parallelism.
Declarations supply executable bodies, not a second protocol or holder table.
The execution layer owns only polling, future lifetimes, and cancellation.

## Entry and platform

A board or host integration supplies its wake/wait driver once at the entry
point. The same attributes and localside signatures work on hosts and Pico.

```rust
#[hibana_runtime::main(platform_events())]
async fn main() -> Result<(), EndpointError> {
    // Endpoint issuance and physical resource binding belong to the platform.
    async { owner(owner_endpoint, db_endpoint).await };
    async { worker(worker_endpoint).await };
    async { service(service_endpoint).await };
    Ok(())
}
```

There is no application `Runtime::new()` or `integration.run()` interface.
Integration implements `Driver` to supply a Waker and wait without losing a
wake received before waiting. Interrupts, UDP readiness, and timer services
belong to that platform boundary. This repository does not yet implement a
production UDP/timer platform or rewrite hibana-quic.

## Storage and scheduling

Future storage is inferred and inline; no fixed task capacity, allocator,
`Send`, `'static`, or `Unpin` requirement is introduced. Rust's borrow checker
still rejects using the same exclusive Endpoint in two children or moving it
before its borrowing scope finishes. Pinning uses ordinary enclosing async
state plus the entry point's internal stack pin.

Each scheduler sweep traverses the declared slots, polls every live task once,
and rotates its first slot.
All children receive the current parent Waker, which never points into scoped
runtime storage. A wake requests another O(declared slots) sweep. This is a small
cooperative scheduler, with no performance superiority claim over Tokio.
Every poll, including application computation, must return to permit progress.

Contiguous declarations use a binary carry forest, preserving source order
with logarithmic storage-type depth. Nested/interleaved declarations follow
lexical lifetimes; no dynamic pool or mutable global registration is needed.
The scheduler builder is an undocumented code-generation detail, not the
application API. Macros use `syn`/`quote` on the build host; the generated
runtime code uses only `core`.

## Run and verify

```sh
cargo +stable run --example localsides
python3 tools/validate.py
```

The example executes both route choices under a parallel global, using direct
`offer`/`send`/`recv`. The carrier and event latch allocate nothing.

Validation runs sequentially under the shared [3 GB descendant RSS
guard](tools/resource_guard.py). Logs, memory peaks, and source hashes are kept
in `validation/`. Rust execution, exact-code compile-fail checks, allocation
counting, Miri, the 256-declaration executable, and Cortex-M0+ compilation/linking
cover the implemented path. Lean and Z3 cover the unchanged scheduler arithmetic
in independent models; they do not prove macro transformation, protocol
correctness, pin safety, or arbitrary computation termination.

Prerequisites: updated stable Rust with rustfmt, clippy, and
`thumbv6m-none-eabi`; Lean `v4.34.1`; Z3. Miri alone uses
`nightly-2026-10-12` with `miri`/`rust-src`.

To include development-core evidence without editing its checkout:

```sh
HIBANA_DELEGATION_CORE=/absolute/path/to/core python3 tools/validate.py
```

This records the actual core revision and all source fingerprints, attaches the
same production-lowered private images used by core's internal tests, and checks
A-to-B-to-A across separate actor runtimes with two independent views and their
post-return continuation. It does not disable or qualify the public guard, prove
the whole delegation implementation, or establish QUIC integration.

The [Pico link probe](examples/pico.rs) executes the same attribute-generated
path and exports its future size. It is a capacity probe with no boot block,
vector table, hardware test, or whole-application stack high-water measurement.
