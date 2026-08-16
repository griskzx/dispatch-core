use crate::DispatchError;

/// Erased function pointer stored by a [`CommandEntry`].
pub type CommandHandler<C, R, O, E> = fn(&mut C, &mut R) -> Result<O, DispatchError<E>>;

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
    pub fn invoke(&self, context: &mut C, resources: &mut R) -> Result<O, DispatchError<E>> {
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
pub struct Dispatcher<'table, K, C, R, O, E, M> {
    table: &'table [CommandEntry<K, C, R, O, E>],
    matcher: M,
}

impl<'table, K, C, R, O, E, M> Dispatcher<'table, K, C, R, O, E, M> {
    /// Creates a dispatcher over `table`.
    pub const fn new(table: &'table [CommandEntry<K, C, R, O, E>], matcher: M) -> Self {
        Self { table, matcher }
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

    /// Selects a command, prepares its resource parameters, and executes it.
    pub fn dispatch(&self, context: &mut C, resources: &mut R) -> Result<O, DispatchError<E>>
    where
        M: Matcher<K, C>,
    {
        let command = self.select(context).ok_or(DispatchError::NotFound)?;
        command.invoke(context, resources)
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
