# dispatch-core

[![Crates.io](https://img.shields.io/crates/v/dispatch-core.svg)](https://crates.io/crates/dispatch-core)
[![Documentation](https://docs.rs/dispatch-core/badge.svg)](https://docs.rs/dispatch-core)
[![CI](https://github.com/griskzx/dispatch-core/actions/workflows/ci.yml/badge.svg)](https://github.com/griskzx/dispatch-core/actions/workflows/ci.yml)
[![License](https://img.shields.io/crates/l/dispatch-core.svg)](https://github.com/griskzx/dispatch-core#license)

`dispatch-core` is a small, `no_std`, allocation-free command dispatcher for
embedded and other resource-constrained systems. Commands declare the
capabilities they need as function parameters; the dispatcher selects a
command, validates its resource accesses, injects those parameters, and calls
the function.

The crate is not an ECS or an application runtime. It provides the narrow
execution core needed to build static command routers with predictable memory
use.

## Design

The API keeps three application concepts separate:

- **Context** is mutable state for the current command, such as an opcode,
  request payload, session, or state-machine position.
- **Resources** are longer-lived system capabilities, such as I/O, crypto,
  storage, and timers.
- **Commands** are ordinary functions whose parameters state exactly which
  resources they use.

For example, one command may need no system capability while another borrows
crypto and I/O:

```text
fn version(context: &mut Context)
fn send(context: &mut Context, io: &mut Io)
fn sign(context: &mut Context, crypto: &Crypto, io: &mut Io)
```

Context never needs to grow into a container for every system capability.

## Installation

The resource-injection API is currently unreleased. Track the main branch while
it is being prepared for the next crates.io release:

```toml
[dependencies]
dispatch-core = { git = "https://github.com/griskzx/dispatch-core" }
```

## Complete static-table example

```rust
use dispatch_core::{CommandEntry, CommandKey, command, dispatch, resource_param};

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

struct Resources {
    io: Io,
    crypto: Crypto,
}

resource_param!(IoMut for Resources => io: Io, exclusive);
resource_param!(CryptoRef for Resources => crypto: Crypto, shared);

#[derive(Debug, PartialEq, Eq)]
struct Response(u32);

#[derive(Debug, PartialEq, Eq)]
enum Error {
    Overflow,
}

fn version(_context: &mut Context) -> Result<Response, Error> {
    Ok(Response(3))
}

fn send(context: &mut Context, io: &mut Io) -> Result<Response, Error> {
    io.last_value = context.payload;
    Ok(Response(io.last_value))
}

fn sign_and_send(
    context: &mut Context,
    crypto: &Crypto,
    io: &mut Io,
) -> Result<Response, Error> {
    let signed = context.payload.checked_add(crypto.key).ok_or(Error::Overflow)?;
    io.last_value = signed;
    Ok(Response(signed))
}

type Entry = CommandEntry<Opcode, Context, Resources, Response, Error>;

static COMMANDS: &[Entry] = &[
    command!(Opcode::Version, version),
    command!(Opcode::Send, send; IoMut),
    command!(Opcode::SignAndSend, sign_and_send; CryptoRef, IoMut),
];

let mut context = Context {
    opcode: Opcode::SignAndSend,
    payload: 40,
};
let mut resources = Resources {
    io: Io::default(),
    crypto: Crypto { key: 2 },
};

let response = dispatch(COMMANDS, &mut context, &mut resources);

assert_eq!(response, Ok(Response(42)));
assert_eq!(resources.io.last_value, 42);
```

The parameter markers after the semicolon correspond to the command's
resource arguments in order. A zero-resource command needs no marker list.

## Execution model

Each dispatch performs these steps:

1. scan the borrowed command table from beginning to end;
2. select the first key accepted by the matcher;
3. validate the selected handler's declared resource accesses;
4. fetch each resource argument;
5. invoke the command function exactly once;
6. return its output or a structured error.

`CommandEntry` stores only a key and a monomorphized function pointer. A
`static` entry slice can therefore remain in read-only memory and requires no
runtime registration or heap allocation.

## Resource parameters

`SystemParam<R>` maps a marker type to the argument a command receives. The
`resource_param!` macro covers direct struct-field access:

```text
resource_param!(ConfigRef for Resources => config: Config, shared);
resource_param!(IoMut for Resources => io: Io, exclusive);
```

Shared access produces `&T`. Exclusive access produces `&mut T`. Shared
accesses to one field may coexist; any access containing an exclusive borrow
conflicts with another access to that field.

Handlers with zero through eight injected resource parameters are supported.
Passing lightweight references, guards, or handles keeps stack usage
predictable.

### Safety boundary

Directly producing several field references from one resource container
requires a small unsafe core. The boundary is explicit:

- `dispatch` holds an exclusive `&mut Resources` for the invocation;
- every `SystemParam` declares a stable `ResourceId` and either shared or
  exclusive access;
- the handler checks every pair of accesses before creating any reference;
- only `SystemParam::fetch` receives the raw resource pointer;
- `resource_param!` generates the access declaration and raw-pointer field
  projection together, so ordinary field injection does not require
  application-written unsafe code;
- manual `unsafe impl SystemParam` is reserved for custom backends and carries
  a documented contract requiring accurate alias metadata and lifetimes.

The crate uses `#![deny(unsafe_op_in_unsafe_fn)]`. Unsafe operations remain in
explicit blocks and are small enough to audit locally.

## Matching

The convenience `dispatch` function uses `CommandKey` and exact equality.
Applications needing ranges, masks, priorities, protocol versions, or other
selection rules can construct a `Dispatcher` with a custom `Matcher`:

```rust
# use dispatch_core::{CommandEntry, Dispatcher};
# struct Context { opcode: u8 }
# struct Resources;
# type Entry = CommandEntry<u8, Context, Resources, (), core::convert::Infallible>;
# let commands: &[Entry] = &[];
let dispatcher = Dispatcher::new(commands, |key: &u8, context: &Context| {
    *key == (context.opcode & 0x0f)
});
```

Matching sees shared references only. Context and resources become mutable
only after a command has been selected.

## Errors

Dispatch returns `DispatchError<E>`:

- `NotFound` means no command matched;
- `Param` reports either conflicting parameter access or a fetch failure with
  its argument position;
- `Command(E)` preserves the selected command's application error.

No panic is used for ordinary lookup, resource, or command failures.

## Complexity and guarantees

- Command selection is a deterministic `O(n)` linear scan.
- Resource validation is `O(p²)` for the selected command, where `p` is at
  most eight and requires only a small fixed-size stack array.
- The selected command runs exactly once; no command runs on lookup or
  parameter failure.
- The crate performs no heap allocation and has no dependencies.
- Static command tables require no mutable registration state.
- Context and Resources remain separate application-owned values.
- Dispatch is synchronous. Async execution, middleware, transactions, dynamic
  plugins, and runtime resource ownership are intentionally outside this core.

The public `Handler` and `SystemParam` abstractions can support additional
registry backends later. The current built-in registry is the static borrowed
table, which is the smallest and most predictable backend for firmware.

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
