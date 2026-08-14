#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

use core::{error::Error, fmt};

/// A dispatcher backed by a borrowed table and a matching strategy.
///
/// The table is scanned from beginning to end. The first matching item is
/// selected, so table order defines dispatch priority.
pub struct Dispatcher<'table, Item, M> {
    table: &'table [Item],
    matcher: M,
}

impl<'table, Item, M> Dispatcher<'table, Item, M> {
    /// Creates a dispatcher backed by `table`.
    pub const fn new(table: &'table [Item], matcher: M) -> Self {
        Self { table, matcher }
    }

    /// Returns the first item accepted by the matcher.
    ///
    /// This method performs selection only. It does not invoke an executor or
    /// otherwise operate on the selected item.
    pub fn select<Input>(&self, input: &Input) -> Option<&'table Item>
    where
        Input: ?Sized,
        M: Matcher<Item, Input>,
    {
        self.table
            .iter()
            .find(|item| self.matcher.matches(item, input))
    }

    /// Selects an item and delegates execution to `execute`.
    ///
    /// The executor receives the selected item and the original input. It can
    /// capture arbitrary per-call state and borrowed resources without making
    /// them part of the dispatcher's type. The executor is not invoked when no
    /// item matches.
    pub fn dispatch<'input, Input, Output, ExecuteError, Execute>(
        &self,
        input: &'input Input,
        execute: Execute,
    ) -> Result<Output, DispatchError<ExecuteError>>
    where
        Input: ?Sized,
        M: Matcher<Item, Input>,
        Execute: FnOnce(&'table Item, &'input Input) -> Result<Output, ExecuteError>,
    {
        let item = self.select(input).ok_or(DispatchError::NotFound)?;

        execute(item, input).map_err(DispatchError::Execute)
    }

    /// Returns the dispatch table.
    pub const fn table(&self) -> &'table [Item] {
        self.table
    }

    /// Returns a shared reference to the matcher.
    pub const fn matcher(&self) -> &M {
        &self.matcher
    }

    /// Returns a mutable reference to the matcher.
    ///
    /// This can be used to reconfigure matching between dispatch calls. The
    /// matcher itself remains immutable while selection is in progress.
    pub fn matcher_mut(&mut self) -> &mut M {
        &mut self.matcher
    }

    /// Splits the dispatcher into its table and matcher.
    pub fn into_parts(self) -> (&'table [Item], M) {
        (self.table, self.matcher)
    }
}

/// An error raised while dispatching an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchError<E> {
    /// No item in the table matched the input.
    NotFound,

    /// The caller-provided executor failed.
    Execute(E),
}

impl<E> fmt::Display for DispatchError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => formatter.write_str("no matching dispatch item"),
            Self::Execute(error) => write!(formatter, "dispatch execution failed: {error}"),
        }
    }
}

impl<E> Error for DispatchError<E>
where
    E: Error + 'static,
{
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        match self {
            Self::NotFound => None,
            Self::Execute(error) => Some(error),
        }
    }
}

/// Determines whether an item accepts an input.
///
/// Matching receives immutable references and should not produce application
/// side effects. Stateful matching can still be configured between calls via
/// [`Dispatcher::matcher_mut`].
pub trait Matcher<Item, Input: ?Sized> {
    /// Returns `true` when `item` accepts `input`.
    fn matches(&self, item: &Item, input: &Input) -> bool;
}

impl<Item, Input, F> Matcher<Item, Input> for F
where
    Input: ?Sized,
    F: Fn(&Item, &Input) -> bool,
{
    fn matches(&self, item: &Item, input: &Input) -> bool {
        self(item, input)
    }
}
