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

An entry point may return `()` when it cannot fail. This also works with Rust's
`#[test]`: put `#[hibana_runtime::main(driver)]` before `#[test]`. Returning from
the entry point waits for its declared children, including on an early return.
Fallible entry points keep their ordinary `Result` return type.

Use a declaration loop for a fixed array of participants. Each iteration becomes
its own localside; the runtime stores and drives them together. Ordinary `for`
loops remain sequential. Array size comes from the inputs, with no task capacity,
spawn handles, result collection loop or heap allocation.

```rust
#[hibana_runtime::scope]
async fn image(outputs: [&core::cell::Cell<u8>; 3]) -> Result<(), core::convert::Infallible> {
    for output in outputs {
        async move {
            hibana_runtime::yield_now().await;
            output.set(7);
        };
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

`host::Datagram` owns a nonblocking Unix datagram socket. Its `send_to` and
`recv_from` preserve message boundaries and the OS sender address. Each pending
operation owns its readiness registration; cancellation removes that registration.
Descriptor duplication preserves the bound address without sharing Rust mutable
storage. Datagram buffers stay with the I/O participant.

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
    async { sides::server(server).await?; };
    async { sides::store(store).await?; };
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

Inside a `#[hibana_runtime::scope]` function, a standalone `async { ... };`
declares a child. Its complete receive/compute/send flow stays visible:

```rust
async {
    let [mut a, mut b] = endpoint.par()?;
    async {
        let request = a.recv::<Request>().await?;
        let answer = calculate(request);
        a.send::<Answer>(&answer).await?;
    };
    async { b.send::<Status>(&status).await?; };
    Ok(())
}.await?;
```

Children run concurrently with the remainder of their async body. Successful
return finishes registered children, including computation after their final
Endpoint operation. Errors cancel unfinished futures in place. `par()` supplies
the protocol fragments; the runtime polls their Rust computations.

A body ending in child declarations returns their successful values. One child
returns its value, several return a tuple in declaration order:

```rust
let (first, second) = async {
    async { first_endpoint.recv::<First>().await };
    async { second_endpoint.recv::<Second>().await };
}.await?;
use_results(first, second);
```

The async expression is an ordinary Future: it can also be stored in a variable
or returned by a function. Its children may return different types or borrow
caller-owned data. An explicit parent `Ok(value)` or early successful return
keeps that value instead, after finishing children already declared. Parent
effects between child declarations preserve declaration order in the results.

A fixed-array loop containing one async child declares an array of children.
It returns an array of successful values with the same length; nested arrays
keep their shape:

```rust
let replies = async {
    for mut view in views {
        async move { view.recv::<Reply>().await };
    }
}.await?;
// replies is [ReplyValue; N].
```

Attributes on an array's child apply to that child. Disabling it with `cfg` or
`cfg_attr` still evaluates the array input and drops its elements in iteration
order, including pattern bindings. No child future is created or polled, and
the disabled declaration contributes no result value. Attributes on the outer
`for` control the entire declaration, including input evaluation.

Array length supplies the storage size. No declaration attribute, result handle,
or tuple-arity table is required. Ordinary loops with no async child remain
sequential. Future variables stay lazy until driven; constructors inside a child
run when that child is polled. Construct input futures before declaring the
children when their constructors must run first.

The await boundary releases future storage and input borrows before its caller
continues. Returned values keep their own borrows. To wait for either of two
futures, `race(left, right).await` drops the unfinished future when one completes.
Cancellation follows each future's contract and never restores delegated rights
or rolls back accepted I/O.

Ordinary `{}` blocks do not await their children. Declarations belong directly
to an async body. Conditions and loops can await their own async scopes, while
closures and nested functions retain their Rust boundaries. Scope functions
return `Result<T, E>`, including aliases; a unit-valued child body infers
`Ok(())`. Return, error, loop-label and drop behavior is preserved when an
immediately awaited scope can use its caller's storage directly.

The function attribute lowers the signature to an ordinary Rust 2024 function
returning a Future. Localsides can write `Endpoint<CLIENT>` with elided input
lifetimes. The future retains argument ownership and lazy pattern destructuring.
Children need not be `Send`, `'static` or `Unpin`.

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
