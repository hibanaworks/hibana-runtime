# hibana-runtime

Write localsides with direct Endpoint operations and application effects.
Their image supplies execution. No public task builder, `spawn`, `join!`,
`select!`, completion handles, or user pinning.
Rust 2024, unconditional `no_std`, no runtime allocation, caller-owned storage.

```rust
#[hibana_runtime::main(events)]
async fn image(
    (server, store): (Endpoint<SERVER>, Endpoint<STORE>),
    events: Events,
) -> Result<(), EndpointError> {
    #[localside]
    sides::server(server);
    #[localside]
    sides::store(store);
    Ok(())
}
```

This is the service image in the runnable [two-image example](examples/images.rs).
The same [global](examples/localsides/protocol.rs), containing `par` and `route`,
and the same [localside functions](examples/localsides/sides.rs), containing
`send`/`recv`/`offer` and effects, execute across either two OS threads or two
actual OS processes. The application image holds CLIENT and AUDITOR. No task or
Endpoint pointer crosses the socket.

## Async bodies own execution

Within an attributed function, `#[localside] future_expression;` registers an
existing future directly. Calls, methods, future variables, and future-valued
macros need no `async { ...await }` wrapper. A standalone `async { ... };`
declares an inline localside when writing its operations in place.

A declaration runs concurrently with the rest of its **async body**. The future
finishes its children before successfully returning. Constructors and arguments
execute at their source position; declarations are not hoisted over effects or
awaits. Localside signatures use role names, such as `Endpoint<CLIENT>`, without
lifetime placeholders. The attribute lowers the function to an ordinary Rust 2024
function returning a future; Rust captures the elided input lifetimes. Argument
ownership and lazy parameter destructuring retain async-function behavior.

Children may borrow caller-owned data and need not be `Send`, `'static`,
or `Unpin`.

Ordinary `{}` blocks do not introduce waiting, joining, or async destruction.
Registrations belong directly to an async body. For a nested phase, an ordinary
async expression gives the lifetime boundary and an explicit `.await` completes
it. For example, after receiving two owned views:

```rust
async {
    async {
        db.send::<Read>(&42).await?;
        record_value(db.recv::<Value>().await?);
    };
    async {
        auth.send::<Check>(&7).await?;
        record_permission(auth.recv::<Allowed>().await?);
    };
    Ok(())
}.await?;
worker.send::<Loan>(db).await?;
worker.send::<AuthLoan>(auth).await?;
```

The await releases the children's borrows before returning the views. A stored
async value does not run until polled. Conditions and loops use ordinary Rust;
an async body inside them can contain its own declarations. Sequential code is
not automatically split into independent computations. Closures and nested
functions retain their own boundary; a nested function can use its own attribute.

Functions return `Result<T, E>`, including aliases. Inline child bodies with no
explicit tail infer `Ok(())`. A child error, a parent `?`/returned error, or dropping
the unfinished enclosing future cancels unfinished local children in place.
Early successful returns finish the children. Loop labels, break values, and
returns use Rust's existing async boundaries; there is no synthetic exit enum or
rewriting of ordinary block/loop exits. An error never rolls back accepted
communication or delegated ownership. Remote failure observation belongs to the
carrier; local cancellation is not an instantaneous distributed rollback.

## Global, projection, placement

Global choreography and Hibana's projection/kernel own protocol order, route
knowledge, ownership, and permitted parallelism. The execution layer owns only
future lifetimes, polling, and cancellation. It does not interpret the global
a second time or track the current holder of a delegated role.

Projection and initial Endpoint issuance are explicit bootstrap work:
`project(&global)` followed by `rendezvous.enter`. The localside's Endpoint type
infers the role, without a second numeric role mapping at projection. The global
is independently defined outside the executing localsides. The attributes do
not mint Endpoints or silently project remote roles.

A role is a logical participant or operation right. An image is placed in a
physical thread/core/process. Multiple roles can reside in one image. Each image
constructs and pins its futures and kernel storage locally, and drives only its
own localsides. Moving an execution place does not require making local-only
Endpoint state `Send`/`Sync`. Processes and cores communicate through the carrier;
`offer()` retains its ordinary evidence-driven branch semantics.

This follows the placement boundary in `hibana-pico`: Core0 and Core1 instantiate
their own images and communicate through SIO. Host processes use the same
localside interface with an appropriate platform carrier. Placement supplies
physical bindings, not another choreography or resource-owner table.

## Platform and scheduling

A platform supplies `Driver` once at its image entry. Its Waker must be valid
for cross-thread/interrupt calls and retained clones, including after execution
ends. `wait` must preserve wakes that arrived before parking. The host example
uses socket readiness and a wake socket. The Pico link probe uses SEV/WFE.
There is no application `Runtime::new()` or `integration.run()` interface.

Within one image, each sweep visits declared slots, polls each live future at
most once, and rotates its starting slot. A wake requests another O(declared
slots) sweep. Completed slots remain inline. Contiguous declarations use a binary
carry forest, preserving source order with logarithmic storage-type depth.
There is no fixed task capacity or runtime allocator. Macros use `syn`/`quote` on
the build host; generated execution code uses only `core`.

One image executes cooperatively: application polls must return promptly.
Different images execute independently on their assigned workers. There is no
claim of speed superiority over Tokio or automatic CPU parallelism inside one
sequential Rust function.

## Run and verify

```sh
cargo +stable run --example localsides
cargo +stable run --example images
python3 tools/validate.py
```

The second example uses Unix datagrams on Unix hosts. The OS starts two threads
and then two child processes; each pair executes both route choices under the
same parallel global. Readiness drives pending Endpoint operations without a
busy-poll loop. Host OS/driver setup uses `std` and owned OS resources; it does
not become a dependency or heap requirement of the no_std runtime. The carrier's
bounds come from this finite example's four frames and u32 payloads, not a
runtime-wide capacity.

Validation runs one job at a time under the shared [3 GB descendant RSS
guard](tools/resource_guard.py). Logs, memory peaks, and source hashes stay in
`validation/`. Tests cover exact borrow/move diagnostics, normal async boundaries,
allocation counting, cancellation, retained wakers, simultaneous polls of non-Send
futures built on different threads, actual Endpoint communication across processes,
Miri, 256 declarations without a recursion-limit override, and Cortex-M0+ linking.
Lean and Z3 check scoped scheduler arithmetic in independent models; they do not
prove macro refinement, protocol correctness, or arbitrary computation termination.

Prerequisites: updated stable Rust with rustfmt/clippy and `thumbv6m-none-eabi`,
Lean `v4.34.1`, and Z3. Miri alone uses `nightly-2026-10-12` with `miri`/`rust-src`.

## Development-core boundary

Ordinary Endpoint examples use published Hibana 0.9.9. Owned delegation requires
the development core. To check the [real owned-view fixture](fixtures/owned_views.rs)
without editing that core checkout:

```sh
HIBANA_DELEGATION_CORE=/absolute/path/to/core python3 tools/validate.py
```

The fixture checks two independent views, A-to-B-to-A ownership transfer across
separate actor runtimes, and post-return continuation, on host and under Miri.
Native integration fixtures additionally exercise actual process-to-process
owned transfer, an unread frame surviving the former holder's process exit,
and route/resolver/par/roll composition with post-return continuation. These
fixtures use hibana-runtime for localside scheduling and the core's existing
Tokio-backed native test carrier for OS readiness and watchdog timers. Tokio is
not a dependency of the runtime. The finite host image example uses no Tokio.

Validation records the captured core revision and dirty-source fingerprints.
Only test setup changes in its isolated copy: integration modules are added and
the existing native mailbox helper becomes visible to sibling tests. Production
code and the public owned-projection guard stay unchanged. These internal tests
do not qualify public delegation, arbitrary remote deployments, or a hibana-quic
rewrite. The standalone two-process executable tests ordinary Endpoint traffic;
the separate native fixtures supply the owned-transfer evidence.

The [Pico link probe](examples/pico.rs) retains independent core0/core1 entries
with local inferred future storage. It is a size/link probe with no boot block,
vector table, core1 launch implementation, flashed-board test, or whole-application
stack high-water measurement. Production UDP/timer platforms and the hibana-quic
rewrite remain outside this runtime change.
