use dispatch_core::{
    CommandEntry, CommandKey, Dispatcher, KeyMatcher, Res, ResMut, command, resources,
};

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

#[derive(Default)]
struct Audit {
    before_calls: u32,
    after_calls: u32,
    response_calls: u32,
}

resources! {
    struct Resources {
        io: Io,
        crypto: Crypto,
        audit: Audit,
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Response {
    value: u32,
}

#[derive(Debug, PartialEq, Eq)]
struct WireResponse {
    status: u8,
    value: u32,
}

#[derive(Debug, PartialEq, Eq)]
enum Error {
    Overflow,
}

fn version(_context: &mut Context) -> Result<Response, Error> {
    Ok(Response { value: 3 })
}

fn send(context: &mut Context, mut io: ResMut<'_, Io>) -> Result<Response, Error> {
    io.last_value = context.payload;
    io.writes += 1;
    Ok(Response {
        value: io.last_value,
    })
}

fn sign_and_send(
    context: &mut Context,
    crypto: Res<'_, Crypto>,
    mut io: ResMut<'_, Io>,
) -> Result<Response, Error> {
    let signed = context
        .payload
        .checked_add(crypto.key)
        .ok_or(Error::Overflow)?;
    io.last_value = signed;
    io.writes += 1;
    Ok(Response { value: signed })
}

fn before(_context: &mut Context, mut audit: ResMut<'_, Audit>) -> Result<(), Error> {
    audit.before_calls += 1;
    Ok(())
}

fn after(
    _context: &mut Context,
    _response: &mut Response,
    mut audit: ResMut<'_, Audit>,
) -> Result<(), Error> {
    audit.after_calls += 1;
    Ok(())
}

fn response(
    _context: &mut Context,
    response: Response,
    mut audit: ResMut<'_, Audit>,
) -> Result<WireResponse, Error> {
    audit.response_calls += 1;
    Ok(WireResponse {
        status: 0,
        value: response.value,
    })
}

type Entry = CommandEntry<Opcode, Context, Resources, Response, Error>;

static COMMANDS: &[Entry] = &[
    command!(Opcode::Version => version),
    command!(Opcode::Send => send),
    command!(Opcode::SignAndSend => sign_and_send),
];

fn main() {
    let mut context = Context {
        opcode: Opcode::SignAndSend,
        payload: 40,
    };
    let mut resources = Resources {
        io: Io::default(),
        crypto: Crypto { key: 2 },
        audit: Audit::default(),
    };

    let dispatcher = Dispatcher::new(COMMANDS, KeyMatcher)
        .with_before(before)
        .with_after(after)
        .with_response(response);
    let response = dispatcher.dispatch(&mut context, &mut resources).unwrap();

    assert_eq!(
        response,
        WireResponse {
            status: 0,
            value: 42,
        }
    );
    assert_eq!(resources.io.last_value, 42);
    assert_eq!(resources.io.writes, 1);
    assert_eq!(resources.audit.before_calls, 1);
    assert_eq!(resources.audit.after_calls, 1);
    assert_eq!(resources.audit.response_calls, 1);
    println!(
        "status={}, value={}, writes={}",
        response.status, response.value, resources.io.writes
    );
}
