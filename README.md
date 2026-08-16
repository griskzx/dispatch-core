# dispatch-core

[![Crates.io](https://img.shields.io/crates/v/dispatch-core.svg)](https://crates.io/crates/dispatch-core)
[![Documentation](https://docs.rs/dispatch-core/badge.svg)](https://docs.rs/dispatch-core)
[![CI](https://github.com/griskzx/dispatch-core/actions/workflows/ci.yml/badge.svg)](https://github.com/griskzx/dispatch-core/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/dispatch-core.svg)](https://github.com/griskzx/dispatch-core#license)

`dispatch-core` is a small, `no_std`, allocation-free command dispatcher for
embedded and other resource-constrained systems. Commands and optional
pipeline stages declare the capabilities they need as function parameters;
the dispatcher validates those accesses, injects the resources, and calls the
functions.

The crate is not an ECS or an application runtime. It is a predictable
execution core for static command routers built above drivers, a HAL, an OS,
or another source of system capabilities.

## Design

The API keeps four application concepts separate:

- **Context** is mutable state for the current request, such as an opcode,
  payload, session, or state-machine position.
- **Resources** are longer-lived system capabilities, such as I/O, crypto,
  storage, timers, and service handles.
- **Commands** are ordinary functions whose parameters state exactly which
  resources they use.
- **Pipeline stages** are independent before, after, and response functions
  that apply common policy without being repeated in every command.

Context does not need to grow into a container for every system capability,
and the command table contains routing only:

```text
fn version(context: &mut Context)
fn send(context: &mut Context, io: ResMut<Io>)
fn sign(context: &mut Context, crypto: Res<Crypto>, io: ResMut<Io>)

static COMMANDS = [
    command!(Opcode::Version => version),
    command!(Opcode::Send => send),
    command!(Opcode::Sign => sign),
];
```

## Installation

```toml
[dependencies]
dispatch-core = "0.4"
```

## Complete example

```rust
use dispatch_core::{
    CommandEntry, CommandKey, Dispatcher, KeyMatcher, Res, ResMut, command,
    resources,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Opcode {
    Version,
    Send,
    SignAndSend,
}

struct Context {
    opcode: Opcode,
    payload: u32,
}

impl CommandKey<Opcode> for Context {
    fn command_key(&self) -> &Opcode {
        &self.opcode
    }
}

#[derive(Default)]
struct Io {
    last_value: u32,
}

struct Crypto {
    key: u32,
}

#[derive(Default)]
struct Metrics {
    requests: u32,
    responses: u32,
}

resources! {
    struct Resources {
        io: Io,
        crypto: Crypto,
        metrics: Metrics,
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Response(u32);

#[derive(Debug, PartialEq, Eq)]
struct Packet {
    status: u8,
    value: u8,
}

#[derive(Debug, PartialEq, Eq)]
enum Error {
    Overflow,
}

fn version(_context: &mut Context) -> Result<Response, Error> {
    Ok(Response(3))
}

fn send(context: &mut Context, mut io: ResMut<'_, Io>) -> Result<Response, Error> {
    io.last_value = context.payload;
    Ok(Response(io.last_value))
}

fn sign_and_send(
    context: &mut Context,
    crypto: Res<'_, Crypto>,
    mut io: ResMut<'_, Io>,
) -> Result<Response, Error> {
    let signed = context.payload.checked_add(crypto.key).ok_or(Error::Overflow)?;
    io.last_value = signed;
    Ok(Response(signed))
}

fn before(
    _context: &mut Context,
    mut metrics: ResMut<'_, Metrics>,
) -> Result<(), Error> {
    metrics.requests += 1;
    Ok(())
}

fn after(_context: &mut Context, response: &mut Response) -> Result<(), Error> {
    response.0 = response.0.min(255);
    Ok(())
}

fn response(
    _context: &mut Context,
    response: Response,
    mut metrics: ResMut<'_, Metrics>,
) -> Result<Packet, Error> {
    metrics.responses += 1;
    Ok(Packet {
        status: 0,
        value: response.0 as u8,
    })
}

type Entry = CommandEntry<Opcode, Context, Resources, Response, Error>;

static COMMANDS: &[Entry] = &[
    command!(Opcode::Version => version),
    command!(Opcode::Send => send),
    command!(Opcode::SignAndSend => sign_and_send),
];

let dispatcher = Dispatcher::new(COMMANDS, KeyMatcher)
    .with_before(before)
    .with_after(after)
    .with_response(response);

let mut context = Context {
    opcode: Opcode::SignAndSend,
    payload: 40,
};
let mut resources = Resources {
    io: Io::default(),
    crypto: Crypto { key: 2 },
    metrics: Metrics::default(),
};

let packet = dispatcher.dispatch(&mut context, &mut resources);

assert_eq!(packet, Ok(Packet { status: 0, value: 42 }));
assert_eq!(resources.io.last_value, 42);
assert_eq!(resources.metrics.requests, 1);
assert_eq!(resources.metrics.responses, 1);
```

## Execution pipeline

A configured dispatcher runs these stages in order:

1. scan the command table and select the first matching entry;
2. run `before`, if configured;
3. validate and inject the command parameters;
4. execute the command;
5. run `after`, if configured, with mutable access to the successful output;
6. run `response`, if configured, consuming that output and producing the
   final return type.

Processing stops at the first error. `after` is skipped when `before` or the
command fails, and `response` is skipped when any earlier stage fails. Response
handling applies to successful command outputs; application errors remain
structured `DispatchError` values.

The three extensions are independent. Configure only the behavior an
application needs:

```rust
# use dispatch_core::{CommandEntry, Dispatcher, KeyMatcher};
# struct Context;
# struct Resources;
# struct Error;
# type Entry = CommandEntry<u8, Context, Resources, u32, Error>;
# let commands: &[Entry] = &[];
# fn before(_: &mut Context) -> Result<(), Error> { Ok(()) }
# fn after(_: &mut Context, _: &mut u32) -> Result<(), Error> { Ok(()) }
# fn response(_: &mut Context, value: u32) -> Result<u64, Error> { Ok(value.into()) }
let only_before = Dispatcher::new(commands, KeyMatcher).with_before(before);
let only_after = Dispatcher::new(commands, KeyMatcher).with_after(after);
let only_response = Dispatcher::new(commands, KeyMatcher).with_response(response);
# let _ = (only_before, only_after, only_response);
```

Omitting `before` or `after` installs a zero-sized no-op stage. Omitting
`response` returns the command output unchanged. Commands and all three stage
functions support zero through eight injected resource parameters.

Each stage obtains and releases its resource views before the next stage
starts. Two stages may therefore use the same `ResMut<T>` sequentially without
conflicting; alias checks apply to the simultaneously live parameters inside
one function.

## Static command table

`CommandEntry` stores only a key and a monomorphized function pointer. A
`static` entry slice can remain in read-only memory and requires no runtime
registration or heap allocation. Table order defines priority.

The convenience `dispatch` function uses exact key matching and the identity
pipeline:

```rust
# use core::convert::Infallible;
# use dispatch_core::{CommandEntry, CommandKey, command, dispatch};
# #[derive(PartialEq)] enum Opcode { Ping }
# struct Context(Opcode);
# impl CommandKey<Opcode> for Context { fn command_key(&self) -> &Opcode { &self.0 } }
# struct Resources;
# fn ping(_: &mut Context) -> Result<u8, Infallible> { Ok(1) }
# type Entry = CommandEntry<Opcode, Context, Resources, u8, Infallible>;
let commands: &[Entry] = &[command!(Opcode::Ping => ping)];
let mut context = Context(Opcode::Ping);
let mut resources = Resources;

assert_eq!(dispatch(commands, &mut context, &mut resources), Ok(1));
```

Construct `Dispatcher` directly when custom matching or pipeline stages are
needed.

## Resource parameters

`Res<T>` requests shared access to a resource. `ResMut<T>` requests exclusive
access. Both are transparent borrowing wrappers that implement `Deref`, and
neither allocates memory.

The `resources!` macro defines the application-owned container and generates
typed providers for its fields:

```rust
# use dispatch_core::resources;
# struct Io;
# struct Crypto;
resources! {
    struct Resources {
        io: Io,
        crypto: Crypto,
    }
}
```

A resource type may appear once without a tag. Tags distinguish multiple
fields with the same type:

```rust
# use dispatch_core::{ResMut, resources};
struct Io;
struct ControlPort;
struct DataPort;

resources! {
    struct Resources {
        control: Io => ControlPort,
        data: Io => DataPort,
    }
}

# struct Context;
# struct Response;
# struct Error;
fn send_control(
    _context: &mut Context,
    _io: ResMut<'_, Io, ControlPort>,
) -> Result<Response, Error> {
    Ok(Response)
}
```

Shared views of one field may coexist. A shared and exclusive view, or two
exclusive views, conflict and fail before their function runs. Resource types
and tags used by the standard views are `'static` types; the resource
container value itself may live on the stack and is borrowed only for the
current stage.

### Safety boundary

Directly producing several field references from one resource container
requires a small unsafe core. The boundary is explicit:

- `dispatch` holds an exclusive `&mut Resources` while each stage prepares its
  arguments and runs;
- `ResourceProvider` connects a resource type and optional tag to one field;
- `Res` and `ResMut` turn that provider into shared or exclusive access;
- every pair of accesses is checked before any reference is created;
- only `ResourceProvider::get` and `SystemParam::fetch` receive raw pointers;
- `resources!` generates resource identity and field projection together, so
  ordinary containers require no application-written unsafe code;
- pointers are checked for nullness and alignment before references are made;
- manual `unsafe impl ResourceProvider` or `SystemParam` is reserved for custom
  backends and carries documented alias and lifetime obligations.

The crate uses `#![deny(unsafe_op_in_unsafe_fn)]`. Unsafe operations remain in
explicit, locally auditable blocks.

## Matching

The convenience `dispatch` function uses `CommandKey` and exact equality.
Applications needing ranges, masks, priorities, protocol versions, or other
selection rules can construct a `Dispatcher` with a custom `Matcher`:

```rust
# use core::convert::Infallible;
# use dispatch_core::{CommandEntry, Dispatcher};
# struct Context { opcode: u8 }
# struct Resources;
# type Entry = CommandEntry<u8, Context, Resources, (), Infallible>;
# let commands: &[Entry] = &[];
let dispatcher = Dispatcher::new(commands, |key: &u8, context: &Context| {
    *key == (context.opcode & 0x0f)
});
# let _ = dispatcher;
```

Matching sees shared references only. Context and resources become mutable
after a command has been selected.

## Errors

`DispatchError<E>` preserves the point of failure:

- `NotFound` means no command matched;
- `Param { stage, error }` identifies both the pipeline stage and its access or
  fetch failure;
- `Before(E)`, `Command(E)`, `After(E)`, and `Response(E)` preserve the
  application error from that function.

No panic is used for ordinary lookup, resource, hook, command, or response
failures.

## Complexity and guarantees

- Command selection is a deterministic `O(n)` linear scan.
- Resource validation is `O(p²)` per executed stage, where `p` is at most
  eight and uses only a small fixed-size stack array.
- Each configured stage runs at most once and only after all preceding stages
  succeed.
- The crate performs no heap allocation and has no dependencies.
- Static command tables require no mutable registration state.
- Context and Resources remain separate application-owned values.
- Resource dependencies appear only in function signatures, not in the
  command table or dispatcher configuration.
- Dispatch is synchronous. Async execution, transactions, dynamic plugins,
  runtime resource ownership, and transport-specific error encoding are
  intentionally outside this core.

## Embedded build

The repository includes a convenience command for Cortex-M0 targets:

```text
cargo build-thumbv6m
```

Normal `cargo build`, `cargo test`, and `cargo clippy` commands use the host
target, so the test harness remains available.

## Minimum supported Rust version

`dispatch-core` requires Rust 1.85 or newer and uses the Rust 2024 edition.

## License

Licensed under either of the following, at your option:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE));
- MIT License ([LICENSE-MIT](LICENSE-MIT)).

## AI authorship disclosure

See [AI_DISCLOSURE.md](AI_DISCLOSURE.md) for the repository's disclosure.
