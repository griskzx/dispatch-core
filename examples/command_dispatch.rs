use dispatch_core::{CommandEntry, CommandKey, command, dispatch, resource_param};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
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
    writes: u32,
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
struct Response {
    value: u32,
}

#[derive(Debug, PartialEq, Eq)]
enum Error {
    Overflow,
}

fn version(_context: &mut Context) -> Result<Response, Error> {
    Ok(Response { value: 3 })
}

fn send(context: &mut Context, io: &mut Io) -> Result<Response, Error> {
    io.last_value = context.payload;
    io.writes += 1;
    Ok(Response {
        value: io.last_value,
    })
}

fn sign_and_send(context: &mut Context, crypto: &Crypto, io: &mut Io) -> Result<Response, Error> {
    let signed = context
        .payload
        .checked_add(crypto.key)
        .ok_or(Error::Overflow)?;
    io.last_value = signed;
    io.writes += 1;
    Ok(Response { value: signed })
}

type Entry = CommandEntry<Opcode, Context, Resources, Response, Error>;

static COMMANDS: &[Entry] = &[
    command!(Opcode::Version, version),
    command!(Opcode::Send, send; IoMut),
    command!(Opcode::SignAndSend, sign_and_send; CryptoRef, IoMut),
];

fn main() {
    let mut context = Context {
        opcode: Opcode::SignAndSend,
        payload: 40,
    };
    let mut resources = Resources {
        io: Io::default(),
        crypto: Crypto { key: 2 },
    };

    let response = dispatch(COMMANDS, &mut context, &mut resources).unwrap();

    assert_eq!(response, Response { value: 42 });
    assert_eq!(resources.io.last_value, 42);
    assert_eq!(resources.io.writes, 1);
    println!("value={}, writes={}", response.value, resources.io.writes);
}
