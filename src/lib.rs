#![no_std]
#![forbid(unsafe_code)]
#![warn(missing_docs)]
#![doc = include_str!("../README.md")]

use core::{error::Error, fmt};

/// A dispatcher backed by a borrowed table and a matching strategy.
///
/// The table is scanned from beginning to end. The first matching item is
/// selected, so table order defines dispatch priority.
pub struct Dispatcher<'a, Item, M> {
    table: &'a [Item],
    matcher: M,
}

impl<'a, Item, M> Dispatcher<'a, Item, M> {
    /// Creates a dispatcher backed by `table`.
    pub const fn new(table: &'a [Item], matcher: M) -> Self {
        Self { table, matcher }
    }

    /// Returns the first item accepted by the matcher.
    ///
    /// This method performs selection only and does not invoke the item's
    /// [`Handler`].
    pub fn select<Input>(&self, input: &Input) -> Option<&Item>
    where
        Input: ?Sized,
        M: Matcher<Item, Input>,
    {
        self.table
            .iter()
            .find(|item| self.matcher.matches(item, input))
    }

    /// Selects and executes the first item accepted by the matcher.
    ///
    /// `input` is immutable for the entire operation. Runtime state and other
    /// mutable resources should be carried by `context` instead.
    pub fn dispatch(
        &self,
        input: &Item::Input,
        context: &mut Item::Context,
    ) -> Result<Item::Output, DispatchError<Item::Error>>
    where
        Item: Handler,
        M: Matcher<Item, Item::Input>,
    {
        let item = self.select(input).ok_or(DispatchError::NotFound)?;

        item.handle(input, context).map_err(DispatchError::Execute)
    }

    /// Returns the dispatch table.
    pub const fn table(&self) -> &'a [Item] {
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
    pub fn into_parts(self) -> (&'a [Item], M) {
        (self.table, self.matcher)
    }
}

/// An error raised while dispatching an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchError<E> {
    /// No item in the table matched the input.
    NotFound,

    /// The selected item failed during execution.
    Execute(E),
}

impl<E> fmt::Display for DispatchError<E>
where
    E: fmt::Display,
{
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotFound => formatter.write_str("no matching dispatch item"),
            Self::Execute(error) => write!(formatter, "dispatch handler failed: {error}"),
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
    /// Returns `true` when `item` can handle `input`.
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

/// Executes a selected dispatch-table item.
///
/// The input is immutable. Mutable runtime state, services, buffers, or device
/// handles should be placed in [`Self::Context`]. Implementations that do not
/// need a context can use `()`.
pub trait Handler {
    /// Input accepted by the handler.
    type Input: ?Sized;

    /// Mutable runtime context used by the handler.
    type Context: ?Sized;

    /// Value produced by the handler.
    type Output;

    /// Error produced by the handler.
    type Error;

    /// Handles `input` using `context` and returns an output.
    fn handle(
        &self,
        input: &Self::Input,
        context: &mut Self::Context,
    ) -> Result<Self::Output, Self::Error>;
}
