# hibana-runtime

Write localsides with direct Endpoint operations and application effects.
Hibana supplies protocol order; the runtime drives the declared async bodies. No task builder, task spawning, `join!`,
`select!`, completion handles, or user pinning.
Rust 2024, unconditional `no_std`, no runtime allocation, caller-owned storage.

The execution core has no runtime dependencies. The `host` feature supplies a
macOS event queue using the operating system directly. It adds no async framework,
allocator, shared resource owner or protocol cursor. The host module provides
owned nonblocking descriptors, child processes, timers and deadlines for external I/O.

```rust
#[hibana_runtime::main]
async fn main() -> Result<(), std::convert::Infallible> {
    async { /* first localside */ };
    async { /* independent localside */ };
    Ok(())
}
```

Enable `host` for an argument-free entry point. Other platforms provide their
own `Driver` through `#[hibana_runtime::main(driver)]`. Both use the same scoped
scheduler. `yield_now().await` lets another localside run during a long computation.

Use a declaration loop for a fixed array of participants. Each iteration becomes
its own localside; the runtime stores and drives them together. Ordinary `for`
loops remain sequential. Array size comes from the inputs, with no task capacity,
spawn handles, result collection loop or heap allocation.

```rust
#[hibana_runtime::scope]
async fn image(outputs: [&core::cell::Cell<u8>; 3]) -> Result<(), core::convert::Infallible> {
    #[concurrent]
    for output in outputs {
        hibana_runtime::yield_now().await;
        output.set(7);
    }
    Ok(())
}
```

Each I/O participant owns its descriptor and buffers. With the host entry point,
pending reads, writes and timers wake the same scoped execution:

```rust
# #[cfg(all(feature = "host", target_os = "macos"))]
# fn example(reader: std::os::fd::OwnedFd, writer: std::os::fd::OwnedFd) -> std::io::Result<()> {
use hibana_runtime::host::{Io, deadline, sleep};
use std::{io, time::Duration};

#[hibana_runtime::main]
async fn image(mut reader: Io, mut writer: Io) -> io::Result<()> {
    async {
        let mut bytes = [0; 64];
        let count = hibana_runtime::race(
            reader.read(&mut bytes),
            deadline(Duration::from_secs(5)),
        ).await?;
        println!("received {count} bytes");
    };
    async {
        sleep(Duration::from_millis(1)).await?;
        writer.write_all(b"ready").await?;
    };
    Ok(())
}

image(Io::new(reader)?, Io::new(writer)?)?;
# Ok::<(), io::Error>(())
# }
```

Duplicate an OS descriptor with `Io::try_clone` when separate owners need its
read and write directions. Dropping a pending operation removes its event
registration. Cancelling a write preserves bytes already written. Timers count
from construction. Host operations that need to wait require the host entry
point; another driver returns an error. Rust heap allocation is unnecessary for
these operations, although each retained Waker and wait registration owns an OS
descriptor. Application buffers and decoding remain with their I/O owner.

External callbacks can enqueue their data and call `Notification::notify`.
`host::notification()` returns the owned notification handle and an `Io` reader.
Wakes can coalesce: the I/O participant drains its data before waiting on that
reader. Clones own OS descriptors, so callbacks retain no address into a scoped
future and do not need to keep its first Rust waker. Notification never waits
for the reader, and EOF follows the last producer after queued wakes are read.

`host::Process::spawn` starts an OS command with its configured environment and
stdio. Give its piped stdin and stdout to I/O owners with `process.io()`, then
await `process.wait()` alongside them. Cancelling that wait removes its event
registration and leaves the process alive. Dropping the process owner terminates
and reaps the child. The wait itself requires no Rust heap allocation; command
construction and starting an OS process use the standard library.

```rust
#[hibana_runtime::main(events)]
async fn image(
    (server, store): (Endpoint<SERVER>, Endpoint<STORE>),
    events: Events,
) -> Result<(), EndpointError> {
    #[concurrent]
    sides::server(server);
    #[concurrent]
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

Within an attributed function, `#[concurrent] future_expression;` registers an
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

Keep each Endpoint's receive, computation and send in its own inline async body.
Hibana's `par()` supplies the independent fragments; the runtime drives those
bodies without another protocol cursor or result channel. Branches need no
individual attributes.

When later code needs several Rust values together, one declaration waits for
those inputs concurrently, releases their future storage, and binds the values:

```rust
#[concurrent]
let (first, second) = (
    first_endpoint.recv::<First>(),
    second_endpoint.recv::<Second>(),
);
use_results(first, second);
```

The declaration waits only for its own inputs. Other scoped children continue
alongside it and its continuation. An input error cancels unfinished inputs and
propagates once through the enclosing async body. The continuation does not run.
Successful values have ordinary Rust ownership and drop behavior.

The pattern describes the input futures, so a variable or function returning a
tuple works in the same position:

```rust
let pending = make_futures();
#[concurrent]
let (first, second) = pending;
```

Tuple length comes from that pattern, with no tuple-arity trait table. A single
future returning a tuple takes one name: `#[concurrent] let pair = future;`, then
`let (first, second) = pair;`. Fixed arrays, including nested arrays, produce
values of the same shape:

```rust
#[concurrent]
let replies = views.map(|mut view| async move {
    view.recv::<Reply>().await
});
// replies is [ReplyValue; N], ready to use.
```

Array patterns can bind individual completed values. Array elements retain Rust's
requirement to have one concrete type; tuple inputs may have different types.
Input expressions execute once at their source positions. Ordinary stored futures
remain lazy until awaited or explicitly declared as concurrent work.

Future storage ends before the declaration's continuation, releasing input
borrows. Returned values may keep their own borrows of caller data. There are no
result receivers, extra result wakers or completion flags. To wait for either of
two futures, `race(left, right).await` drops the unfinished future when one
completes. Each future retains its normal cancellation contract.

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
slots) sweep. Completed slots remain inline. The macro constructs a balanced
tree of references to the futures' final inline slots once, preserving source
order with logarithmic storage-type depth.
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
HIBANA_RESOURCE_GUARD=/path/to/shared/resource-guard bash tools/check.sh
```

The second example uses Unix datagrams on Unix hosts. The OS starts two threads
and then two child processes; each pair executes both route choices under the
same parallel global. Readiness drives pending Endpoint operations without a
busy-poll loop. Host OS/driver setup uses `std` and owned OS resources; it does
not become a dependency or heap requirement of the no_std runtime. The carrier's
bounds come from this finite example's four frames and u32 payloads, not a
runtime-wide capacity.

Validation runs one job at a time under the supplied descendant-memory guard.
Use the same guard as the consuming Hibana project. Logs and source hashes stay
in `validation/`; retain the guard output with them. Tests cover exact borrow/move diagnostics, normal async boundaries,
allocation counting, bounded-stack construction, cancellation, retained wakers, simultaneous polls of non-Send
futures built on different threads, actual Endpoint communication across processes,
Miri, 256 declarations without a recursion-limit override, and Cortex-M0+ linking.
Lean and Z3 check scoped scheduler arithmetic in independent models; they do not
prove macro refinement, protocol correctness, or arbitrary computation termination.

Prerequisites: updated stable Rust with rustfmt/clippy and `thumbv6m-none-eabi`,
Lean `v4.34.1`, and Z3. Miri alone uses `nightly-2026-10-12` with `miri`/`rust-src`.

The [Pico link probe](examples/pico.rs) retains independent core0/core1 entries
with local inferred future storage. It is a size/link probe with no boot block,
vector table, core1 launch implementation, flashed-board test, or whole-application
stack high-water measurement.
