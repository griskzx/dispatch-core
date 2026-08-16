use core::convert::Infallible;

use dispatch_core::{
    CommandEntry, CommandKey, DispatchError, Dispatcher, FetchError, ParamError, ResourceAccess,
    SystemParam, command, dispatch, resource_param,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Opcode {
    Version,
    Read,
    Update,
    SharedTwice,
    Conflict,
    Unavailable,
    Fail,
}

#[derive(Debug, PartialEq, Eq)]
struct Context {
    opcode: Opcode,
    input: u32,
    calls: u8,
}

impl CommandKey<Opcode> for Context {
    fn command_key(&self) -> &Opcode {
        &self.opcode
    }
}

#[derive(Default)]
struct Resources {
    first: u32,
    second: u32,
}

resource_param!(FirstRef for Resources => first: u32, shared);
resource_param!(FirstMut for Resources => first: u32, exclusive);
resource_param!(SecondMut for Resources => second: u32, exclusive);

struct Missing;

// SAFETY: this marker does not return a value or access the resource container.
unsafe impl SystemParam<Resources> for Missing {
    type Item<'resources> = ();

    const ACCESS: ResourceAccess = ResourceAccess::none();

    unsafe fn fetch<'resources>(
        _resources: *mut Resources,
    ) -> Result<Self::Item<'resources>, FetchError> {
        Err(FetchError::Unavailable)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestError {
    Failed,
}

fn version(context: &mut Context) -> Result<u32, TestError> {
    context.calls += 1;
    Ok(3)
}

fn read(context: &mut Context, first: &u32) -> Result<u32, TestError> {
    context.calls += 1;
    Ok(*first)
}

fn update(context: &mut Context, first: &mut u32, second: &mut u32) -> Result<u32, TestError> {
    context.calls += 1;
    *first += context.input;
    *second += *first;
    Ok(*second)
}

fn shared_twice(context: &mut Context, first: &u32, same_first: &u32) -> Result<u32, TestError> {
    context.calls += 1;
    Ok(*first + *same_first)
}

fn conflicting(
    context: &mut Context,
    _first: &mut u32,
    _same_first: &u32,
) -> Result<u32, TestError> {
    context.calls += 1;
    Ok(0)
}

fn unavailable(context: &mut Context, _missing: ()) -> Result<u32, TestError> {
    context.calls += 1;
    Ok(0)
}

fn fail(context: &mut Context) -> Result<u32, TestError> {
    context.calls += 1;
    Err(TestError::Failed)
}

type Entry = CommandEntry<Opcode, Context, Resources, u32, TestError>;

static COMMANDS: &[Entry] = &[
    command!(Opcode::Version, version),
    command!(Opcode::Read, read; FirstRef),
    command!(Opcode::Update, update; FirstMut, SecondMut),
    command!(Opcode::SharedTwice, shared_twice; FirstRef, FirstRef),
    command!(Opcode::Conflict, conflicting; FirstMut, FirstRef),
    command!(Opcode::Unavailable, unavailable; Missing),
    command!(Opcode::Fail, fail),
];

fn context(opcode: Opcode) -> Context {
    Context {
        opcode,
        input: 5,
        calls: 0,
    }
}

#[test]
fn dispatches_a_context_only_command() {
    let mut context = context(Opcode::Version);
    let mut resources = Resources::default();

    let output = dispatch(COMMANDS, &mut context, &mut resources).unwrap();

    assert_eq!(output, 3);
    assert_eq!(context.calls, 1);
}

#[test]
fn injects_a_shared_resource() {
    let mut context = context(Opcode::Read);
    let mut resources = Resources {
        first: 11,
        second: 0,
    };

    let output = dispatch(COMMANDS, &mut context, &mut resources).unwrap();

    assert_eq!(output, 11);
    assert_eq!(context.calls, 1);
}

#[test]
fn injects_distinct_exclusive_resources() {
    let mut context = context(Opcode::Update);
    let mut resources = Resources {
        first: 2,
        second: 10,
    };

    let output = dispatch(COMMANDS, &mut context, &mut resources).unwrap();

    assert_eq!(output, 17);
    assert_eq!(resources.first, 7);
    assert_eq!(resources.second, 17);
    assert_eq!(context.calls, 1);
}

#[test]
fn permits_repeated_shared_access() {
    let mut context = context(Opcode::SharedTwice);
    let mut resources = Resources {
        first: 9,
        second: 0,
    };

    let output = dispatch(COMMANDS, &mut context, &mut resources).unwrap();

    assert_eq!(output, 18);
    assert_eq!(context.calls, 1);
}

#[test]
fn rejects_conflicting_access_before_invoking_the_command() {
    let mut context = context(Opcode::Conflict);
    let mut resources = Resources::default();

    let result = dispatch(COMMANDS, &mut context, &mut resources);

    assert!(matches!(
        result,
        Err(DispatchError::Param(ParamError::AccessConflict {
            first: 0,
            second: 1,
            ..
        }))
    ));
    assert_eq!(context.calls, 0);
}

#[test]
fn reports_the_position_of_a_fetch_failure() {
    let mut context = context(Opcode::Unavailable);
    let mut resources = Resources::default();

    let result = dispatch(COMMANDS, &mut context, &mut resources);

    assert_eq!(
        result,
        Err(DispatchError::Param(ParamError::Fetch {
            index: 0,
            error: FetchError::Unavailable,
        }))
    );
    assert_eq!(context.calls, 0);
}

#[test]
fn preserves_command_errors() {
    let mut context = context(Opcode::Fail);
    let mut resources = Resources::default();

    let result = dispatch(COMMANDS, &mut context, &mut resources);

    assert_eq!(result, Err(DispatchError::Command(TestError::Failed)));
    assert_eq!(context.calls, 1);
}

#[test]
fn returns_not_found_without_touching_context_or_resources() {
    let table: &[Entry] = &[];
    let mut context = context(Opcode::Version);
    let mut resources = Resources {
        first: 1,
        second: 2,
    };

    let result = dispatch(table, &mut context, &mut resources);

    assert_eq!(result, Err(DispatchError::NotFound));
    assert_eq!(context.calls, 0);
    assert_eq!(resources.first, 1);
    assert_eq!(resources.second, 2);
}

fn first(context: &mut Context) -> Result<u32, TestError> {
    context.calls += 1;
    Ok(1)
}

fn second(context: &mut Context) -> Result<u32, TestError> {
    context.calls += 1;
    Ok(2)
}

#[test]
fn table_order_defines_priority() {
    let table: &[Entry] = &[
        command!(Opcode::Version, first),
        command!(Opcode::Version, second),
    ];
    let mut context = context(Opcode::Version);
    let mut resources = Resources::default();

    let output = dispatch(table, &mut context, &mut resources).unwrap();

    assert_eq!(output, 1);
    assert_eq!(context.calls, 1);
}

#[test]
fn dispatcher_accepts_a_custom_matcher() {
    let dispatcher = Dispatcher::new(COMMANDS, |key: &Opcode, context: &Context| {
        *key == Opcode::Read && context.input == 99
    });
    let mut context = Context {
        opcode: Opcode::Version,
        input: 99,
        calls: 0,
    };
    let mut resources = Resources {
        first: 42,
        second: 0,
    };

    let output = dispatcher.dispatch(&mut context, &mut resources).unwrap();

    assert_eq!(output, 42);
    assert_eq!(context.calls, 1);
}

#[test]
fn dispatcher_exposes_its_configuration() {
    let mut dispatcher = Dispatcher::new(COMMANDS, |_key: &Opcode, _context: &Context| false);

    assert_eq!(dispatcher.table().len(), COMMANDS.len());
    let _ = dispatcher.matcher();
    let _ = dispatcher.matcher_mut();
}

#[test]
fn infallible_command_tables_are_supported() {
    fn ping(context: &mut Context) -> Result<u32, Infallible> {
        context.calls += 1;
        Ok(7)
    }

    type InfallibleEntry = CommandEntry<Opcode, Context, Resources, u32, Infallible>;
    let table: &[InfallibleEntry] = &[command!(Opcode::Version, ping)];
    let mut context = context(Opcode::Version);
    let mut resources = Resources::default();

    assert_eq!(dispatch(table, &mut context, &mut resources), Ok(7));
}
