use core::{convert::Infallible, mem::size_of};

use dispatch_core::{
    CommandEntry, CommandKey, DispatchError, DispatchStage, Dispatcher, FetchError, KeyMatcher,
    ParamError, Res, ResMut, ResourceAccess, ResourceProvider, SystemParam, command, dispatch,
    resources,
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

struct First;
struct Second;

resources! {
    #[derive(Default)]
    struct Resources {
        first: u32 => First,
        second: u32 => Second,
    }
}

struct Missing;

// SAFETY: this marker does not return a value or access the resource container.
unsafe impl SystemParam<Resources> for Missing {
    type Item<'resources> = Missing;

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
    Before,
    After,
    Response,
}

fn version(context: &mut Context) -> Result<u32, TestError> {
    context.calls += 1;
    Ok(3)
}

fn read(context: &mut Context, first: Res<'_, u32, First>) -> Result<u32, TestError> {
    context.calls += 1;
    Ok(*first)
}

fn update(
    context: &mut Context,
    mut first: ResMut<'_, u32, First>,
    mut second: ResMut<'_, u32, Second>,
) -> Result<u32, TestError> {
    context.calls += 1;
    *first += context.input;
    *second += *first;
    Ok(*second)
}

fn shared_twice(
    context: &mut Context,
    first: Res<'_, u32, First>,
    same_first: Res<'_, u32, First>,
) -> Result<u32, TestError> {
    context.calls += 1;
    Ok(*first + *same_first)
}

fn conflicting(
    context: &mut Context,
    _first: ResMut<'_, u32, First>,
    _same_first: Res<'_, u32, First>,
) -> Result<u32, TestError> {
    context.calls += 1;
    Ok(0)
}

fn unavailable(context: &mut Context, _missing: Missing) -> Result<u32, TestError> {
    context.calls += 1;
    Ok(0)
}

fn fail(context: &mut Context) -> Result<u32, TestError> {
    context.calls += 1;
    Err(TestError::Failed)
}

type Entry = CommandEntry<Opcode, Context, Resources, u32, TestError>;

static COMMANDS: &[Entry] = &[
    command!(Opcode::Version => version),
    command!(Opcode::Read => read),
    command!(Opcode::Update => update),
    command!(Opcode::SharedTwice => shared_twice),
    command!(Opcode::Conflict => conflicting),
    command!(Opcode::Unavailable => unavailable),
    command!(Opcode::Fail => fail),
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
        Err(DispatchError::Param {
            stage: DispatchStage::Command,
            error: ParamError::AccessConflict {
                first: 0,
                second: 1,
                ..
            }
        })
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
        Err(DispatchError::Param {
            stage: DispatchStage::Command,
            error: ParamError::Fetch {
                index: 0,
                error: FetchError::Unavailable,
            },
        })
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
        command!(Opcode::Version => first),
        command!(Opcode::Version => second),
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
    let table: &[InfallibleEntry] = &[command!(Opcode::Version => ping)];
    let mut context = context(Opcode::Version);
    let mut resources = Resources::default();

    assert_eq!(dispatch(table, &mut context, &mut resources), Ok(7));
}

#[test]
fn resource_views_have_reference_size() {
    assert_eq!(size_of::<Res<'_, u32, First>>(), size_of::<&u32>());
    assert_eq!(size_of::<ResMut<'_, u32, First>>(), size_of::<&mut u32>());
}

#[test]
fn tags_give_same_typed_fields_distinct_identities() {
    let first = <Resources as ResourceProvider<u32, First>>::ID;
    let second = <Resources as ResourceProvider<u32, Second>>::ID;

    assert_ne!(first, second);
}

struct InvalidResources;

// SAFETY: this deliberately unavailable provider returns null, which is a
// permitted fetch failure, and never exposes memory under its resource ID.
unsafe impl ResourceProvider<u32> for InvalidResources {
    const ID: dispatch_core::ResourceId = dispatch_core::ResourceId::new(0);

    unsafe fn get(_resources: *mut Self) -> *mut u32 {
        core::ptr::null_mut()
    }
}

fn invalid_resource(context: &mut Context, _value: Res<'_, u32>) -> Result<u32, Infallible> {
    context.calls += 1;
    Ok(0)
}

#[test]
fn rejects_an_invalid_provider_pointer_before_invocation() {
    type InvalidEntry = CommandEntry<Opcode, Context, InvalidResources, u32, Infallible>;
    let table: &[InvalidEntry] = &[command!(Opcode::Version => invalid_resource)];
    let mut context = context(Opcode::Version);
    let mut resources = InvalidResources;

    let result = dispatch(table, &mut context, &mut resources);

    assert_eq!(
        result,
        Err(DispatchError::Param {
            stage: DispatchStage::Command,
            error: ParamError::Fetch {
                index: 0,
                error: FetchError::InvalidPointer,
            },
        })
    );
    assert_eq!(context.calls, 0);
}

fn before_only(context: &mut Context, mut first: ResMut<'_, u32, First>) -> Result<(), TestError> {
    context.calls += 1;
    *first += 10;
    Ok(())
}

fn after_only(
    context: &mut Context,
    output: &mut u32,
    mut second: ResMut<'_, u32, Second>,
) -> Result<(), TestError> {
    context.calls += 1;
    *output *= 2;
    *second = *output;
    Ok(())
}

#[test]
fn before_and_after_can_be_configured_independently() {
    let before_dispatcher = Dispatcher::new(COMMANDS, KeyMatcher).with_before(before_only);
    let mut before_context = context(Opcode::Version);
    let mut before_resources = Resources::default();

    let before_output = before_dispatcher
        .dispatch(&mut before_context, &mut before_resources)
        .unwrap();

    assert_eq!(before_output, 3);
    assert_eq!(before_context.calls, 2);
    assert_eq!(before_resources.first, 10);
    assert_eq!(before_resources.second, 0);

    let after_dispatcher = Dispatcher::new(COMMANDS, KeyMatcher).with_after(after_only);
    let mut after_context = context(Opcode::Version);
    let mut after_resources = Resources::default();

    let after_output = after_dispatcher
        .dispatch(&mut after_context, &mut after_resources)
        .unwrap();

    assert_eq!(after_output, 6);
    assert_eq!(after_context.calls, 2);
    assert_eq!(after_resources.first, 0);
    assert_eq!(after_resources.second, 6);
}

#[derive(Debug, PartialEq, Eq)]
struct FinalResponse {
    value: u64,
}

fn finalize_response(
    context: &mut Context,
    output: u32,
    first: Res<'_, u32, First>,
) -> Result<FinalResponse, TestError> {
    context.calls += 1;
    Ok(FinalResponse {
        value: u64::from(output + *first),
    })
}

#[test]
fn response_stage_can_inject_resources_and_change_the_final_type() {
    let dispatcher = Dispatcher::new(COMMANDS, KeyMatcher).with_response(finalize_response);
    let mut context = context(Opcode::Version);
    let mut resources = Resources {
        first: 4,
        second: 0,
    };

    let response = dispatcher.dispatch(&mut context, &mut resources).unwrap();

    assert_eq!(response, FinalResponse { value: 7 });
    assert_eq!(context.calls, 2);
}

fn pipeline_command(context: &mut Context) -> Result<u32, TestError> {
    context.input = context.input * 10 + 2;
    Ok(5)
}

fn pipeline_before(context: &mut Context) -> Result<(), TestError> {
    context.input = context.input * 10 + 1;
    Ok(())
}

fn pipeline_after(context: &mut Context, output: &mut u32) -> Result<(), TestError> {
    context.input = context.input * 10 + 3;
    *output += 1;
    Ok(())
}

fn pipeline_response(context: &mut Context, output: u32) -> Result<u64, TestError> {
    context.input = context.input * 10 + 4;
    Ok(u64::from(output) * 10)
}

#[test]
fn runs_the_complete_pipeline_in_order() {
    let table: &[Entry] = &[command!(Opcode::Version => pipeline_command)];
    let dispatcher = Dispatcher::new(table, KeyMatcher)
        .with_before(pipeline_before)
        .with_after(pipeline_after)
        .with_response(pipeline_response);
    let mut context = context(Opcode::Version);
    context.input = 0;
    let mut resources = Resources::default();

    let response = dispatcher.dispatch(&mut context, &mut resources).unwrap();

    assert_eq!(context.input, 1_234);
    assert_eq!(response, 60);
}

fn conflicting_before(
    context: &mut Context,
    _first: ResMut<'_, u32, First>,
    _same_first: Res<'_, u32, First>,
) -> Result<(), TestError> {
    context.calls += 1;
    Ok(())
}

#[test]
fn reports_parameter_failures_with_the_pipeline_stage() {
    let dispatcher = Dispatcher::new(COMMANDS, KeyMatcher).with_before(conflicting_before);
    let mut context = context(Opcode::Version);
    let mut resources = Resources::default();

    let result = dispatcher.dispatch(&mut context, &mut resources);

    assert!(matches!(
        result,
        Err(DispatchError::Param {
            stage: DispatchStage::Before,
            error: ParamError::AccessConflict {
                first: 0,
                second: 1,
                ..
            }
        })
    ));
    assert_eq!(context.calls, 0);
}

fn fail_before(context: &mut Context) -> Result<(), TestError> {
    context.calls += 1;
    Err(TestError::Before)
}

fn fail_after(context: &mut Context, _output: &mut u32) -> Result<(), TestError> {
    context.calls += 1;
    Err(TestError::After)
}

fn fail_response(context: &mut Context, _output: u32) -> Result<u64, TestError> {
    context.calls += 1;
    Err(TestError::Response)
}

#[test]
fn stops_at_the_first_failed_pipeline_stage() {
    let before_dispatcher = Dispatcher::new(COMMANDS, KeyMatcher)
        .with_before(fail_before)
        .with_after(after_only)
        .with_response(finalize_response);
    let mut before_context = context(Opcode::Version);
    let mut before_resources = Resources::default();

    let before_result = before_dispatcher.dispatch(&mut before_context, &mut before_resources);

    assert_eq!(before_result, Err(DispatchError::Before(TestError::Before)));
    assert_eq!(before_context.calls, 1);
    assert_eq!(before_resources.second, 0);

    let command_dispatcher = Dispatcher::new(COMMANDS, KeyMatcher)
        .with_after(after_only)
        .with_response(finalize_response);
    let mut command_context = context(Opcode::Fail);
    let mut command_resources = Resources::default();

    let command_result = command_dispatcher.dispatch(&mut command_context, &mut command_resources);

    assert_eq!(
        command_result,
        Err(DispatchError::Command(TestError::Failed))
    );
    assert_eq!(command_context.calls, 1);
    assert_eq!(command_resources.second, 0);

    let after_dispatcher = Dispatcher::new(COMMANDS, KeyMatcher)
        .with_after(fail_after)
        .with_response(finalize_response);
    let mut after_context = context(Opcode::Version);
    let mut after_resources = Resources::default();

    let after_result = after_dispatcher.dispatch(&mut after_context, &mut after_resources);

    assert_eq!(after_result, Err(DispatchError::After(TestError::After)));
    assert_eq!(after_context.calls, 2);

    let response_dispatcher = Dispatcher::new(COMMANDS, KeyMatcher).with_response(fail_response);
    let mut response_context = context(Opcode::Version);
    let mut response_resources = Resources::default();

    let response_result =
        response_dispatcher.dispatch(&mut response_context, &mut response_resources);

    assert_eq!(
        response_result,
        Err(DispatchError::Response(TestError::Response))
    );
    assert_eq!(response_context.calls, 2);
}
