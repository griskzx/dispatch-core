use crate::{
    After, AfterFn, AfterHandler, Before, BeforeFn, DispatchError, DispatchStage, Handler,
    IdentityResponse, NoAfter, NoBefore, ResponseFn, ResponseHandler, ResponseStage, StageError,
};

/// Erased function pointer stored by a [`CommandEntry`].
pub type CommandHandler<C, R, O, E> = fn(&mut C, &mut R) -> Result<O, StageError<E>>;

/// One command in a static dispatch table.
///
/// The entry stores only its key and an erased function pointer. It owns no
/// context or resources and requires no heap allocation.
pub struct CommandEntry<K, C, R, O, E> {
    key: K,
    handler: CommandHandler<C, R, O, E>,
}

impl<K, C, R, O, E> CommandEntry<K, C, R, O, E> {
    /// Creates an entry from a key and already-erased handler function.
    ///
    /// Applications normally use [`crate::command!`] to generate `handler`.
    pub const fn new(key: K, handler: CommandHandler<C, R, O, E>) -> Self {
        Self { key, handler }
    }

    /// Returns the command key.
    pub const fn key(&self) -> &K {
        &self.key
    }

    /// Invokes the command without performing matching.
    pub fn invoke(&self, context: &mut C, resources: &mut R) -> Result<O, StageError<E>> {
        (self.handler)(context, resources)
    }
}

/// Determines whether a command key accepts the current context.
///
/// Matching receives only shared references and should not produce application
/// side effects. The selected command receives mutable context and resources
/// only after matching has finished.
pub trait Matcher<K, C: ?Sized> {
    /// Returns `true` when `key` identifies the command for `context`.
    fn matches(&self, key: &K, context: &C) -> bool;
}

impl<K, C, F> Matcher<K, C> for F
where
    C: ?Sized,
    F: Fn(&K, &C) -> bool,
{
    fn matches(&self, key: &K, context: &C) -> bool {
        self(key, context)
    }
}

/// Exposes the command key stored in an application context.
///
/// Implementing this trait enables the convenience [`dispatch`] function and
/// [`KeyMatcher`]. Custom [`Matcher`] implementations need not use it.
pub trait CommandKey<K> {
    /// Returns the key of the command that should handle this context.
    fn command_key(&self) -> &K;
}

/// Matches an entry key against [`CommandKey::command_key`].
#[derive(Debug, Clone, Copy, Default)]
pub struct KeyMatcher;

impl<K, C> Matcher<K, C> for KeyMatcher
where
    K: PartialEq,
    C: CommandKey<K>,
{
    fn matches(&self, key: &K, context: &C) -> bool {
        key == context.command_key()
    }
}

/// A borrowed command table paired with a matching strategy.
///
/// Table order defines priority: the first matching entry is executed.
pub struct Dispatcher<'table, K, C, R, O, E, M, B = NoBefore, A = NoAfter, S = IdentityResponse> {
    table: &'table [CommandEntry<K, C, R, O, E>],
    matcher: M,
    before: B,
    after: A,
    response: S,
}

/// Dispatcher type returned by [`Dispatcher::with_response`].
pub type ResponseDispatcher<'table, K, C, R, O, E, M, B, A, F, Params, T> =
    Dispatcher<'table, K, C, R, O, E, M, B, A, ResponseFn<F, Params, T>>;

impl<'table, K, C, R, O, E, M>
    Dispatcher<'table, K, C, R, O, E, M, NoBefore, NoAfter, IdentityResponse>
{
    /// Creates a dispatcher over `table`.
    pub const fn new(table: &'table [CommandEntry<K, C, R, O, E>], matcher: M) -> Self {
        Self {
            table,
            matcher,
            before: NoBefore,
            after: NoAfter,
            response: IdentityResponse,
        }
    }
}

impl<'table, K, C, R, O, E, M, B, A, S> Dispatcher<'table, K, C, R, O, E, M, B, A, S> {
    /// Installs a resource-injected before function.
    ///
    /// This does not install an after function. Calling `with_before` again
    /// replaces only the previous before stage.
    pub fn with_before<F, Params>(
        self,
        function: F,
    ) -> Dispatcher<'table, K, C, R, O, E, M, BeforeFn<F, Params>, A, S>
    where
        F: Handler<C, R, (), E, Params>,
    {
        Dispatcher {
            table: self.table,
            matcher: self.matcher,
            before: BeforeFn::new(function),
            after: self.after,
            response: self.response,
        }
    }

    /// Installs a resource-injected after function.
    ///
    /// The function receives mutable access to a successful command output.
    /// This does not install a before function.
    pub fn with_after<F, Params>(
        self,
        function: F,
    ) -> Dispatcher<'table, K, C, R, O, E, M, B, AfterFn<F, Params>, S>
    where
        F: AfterHandler<C, R, O, E, Params>,
    {
        Dispatcher {
            table: self.table,
            matcher: self.matcher,
            before: self.before,
            after: AfterFn::new(function),
            response: self.response,
        }
    }

    /// Installs a resource-injected successful-response function.
    ///
    /// The function consumes the command output after the after stage and may
    /// convert it to a different final type.
    pub fn with_response<F, Params, T>(
        self,
        function: F,
    ) -> ResponseDispatcher<'table, K, C, R, O, E, M, B, A, F, Params, T>
    where
        F: ResponseHandler<C, R, O, T, E, Params>,
    {
        Dispatcher {
            table: self.table,
            matcher: self.matcher,
            before: self.before,
            after: self.after,
            response: ResponseFn::new(function),
        }
    }

    /// Returns the first command accepted by the matcher.
    pub fn select(&self, context: &C) -> Option<&'table CommandEntry<K, C, R, O, E>>
    where
        M: Matcher<K, C>,
    {
        self.table
            .iter()
            .find(|command| self.matcher.matches(command.key(), context))
    }

    /// Selects a command and runs the configured execution pipeline.
    ///
    /// Execution stops at the first failure. The after stage runs only after a
    /// successful command, and response handling runs only after a successful
    /// after stage.
    pub fn dispatch(
        &self,
        context: &mut C,
        resources: &mut R,
    ) -> Result<S::Output, DispatchError<E>>
    where
        M: Matcher<K, C>,
        B: Before<C, R, E>,
        A: After<C, R, O, E>,
        S: ResponseStage<C, R, O, E>,
    {
        let command = self.select(context).ok_or(DispatchError::NotFound)?;

        self.before
            .run_before(context, resources)
            .map_err(|error| DispatchError::at(DispatchStage::Before, error))?;

        let mut output = command
            .invoke(context, resources)
            .map_err(|error| DispatchError::at(DispatchStage::Command, error))?;

        self.after
            .run_after(context, &mut output, resources)
            .map_err(|error| DispatchError::at(DispatchStage::After, error))?;

        self.response
            .run_response(context, output, resources)
            .map_err(|error| DispatchError::at(DispatchStage::Response, error))
    }

    /// Returns the borrowed command table.
    pub const fn table(&self) -> &'table [CommandEntry<K, C, R, O, E>] {
        self.table
    }

    /// Returns the matcher.
    pub const fn matcher(&self) -> &M {
        &self.matcher
    }

    /// Returns a mutable matcher reference for between-dispatch configuration.
    pub fn matcher_mut(&mut self) -> &mut M {
        &mut self.matcher
    }

    /// Returns the configured before stage.
    pub const fn before(&self) -> &B {
        &self.before
    }

    /// Returns a mutable reference to the configured before stage.
    pub fn before_mut(&mut self) -> &mut B {
        &mut self.before
    }

    /// Returns the configured after stage.
    pub const fn after(&self) -> &A {
        &self.after
    }

    /// Returns a mutable reference to the configured after stage.
    pub fn after_mut(&mut self) -> &mut A {
        &mut self.after
    }

    /// Returns the configured response stage.
    pub const fn response(&self) -> &S {
        &self.response
    }

    /// Returns a mutable reference to the configured response stage.
    pub fn response_mut(&mut self) -> &mut S {
        &mut self.response
    }
}

/// Dispatches through a table using the key exposed by the context.
///
/// This is the smallest API for the common exact-key case. Construct a
/// [`Dispatcher`] when matching requires a custom strategy.
pub fn dispatch<K, C, R, O, E>(
    table: &[CommandEntry<K, C, R, O, E>],
    context: &mut C,
    resources: &mut R,
) -> Result<O, DispatchError<E>>
where
    K: PartialEq,
    C: CommandKey<K>,
{
    Dispatcher::new(table, KeyMatcher).dispatch(context, resources)
}
