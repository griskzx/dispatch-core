#![no_std]
#![forbid(unsafe_code)]
#![doc = include_str!("../README.md")]

/// A dispatcher that owns the components used to select and wrap a handler.
///
/// The dispatch table is supplied for each call, so the dispatcher itself does
/// not own application data or state. This makes it suitable as a small core
/// for higher-level abstractions such as a state machine.
pub struct Dispatcher<M, W> {
    matcher: M,
    middleware: W,
}

impl<M, W> Dispatcher<M, W> {
    /// Creates a dispatcher from a matcher and middleware.
    pub const fn new(matcher: M, middleware: W) -> Self {
        Self {
            matcher,
            middleware,
        }
    }

    /// Dispatches an input to the first matching item in `table`.
    pub fn dispatch<Item, Input, Output, Error>(
        &mut self,
        table: &[Item],
        input: &Input,
    ) -> Result<Output, DispatchError<Error>>
    where
        M: Matcher<Item, Input>,
        W: Middleware<Item, Input, Output, Error>,
        Item: Handler<Input = Input, Output = Output, Error = Error>,
    {
        dispatch(self, table, input)
    }

    /// Returns a shared reference to the matcher.
    pub const fn matcher(&self) -> &M {
        &self.matcher
    }

    /// Returns a mutable reference to the matcher.
    pub fn matcher_mut(&mut self) -> &mut M {
        &mut self.matcher
    }

    /// Returns a shared reference to the middleware.
    pub const fn middleware(&self) -> &W {
        &self.middleware
    }

    /// Returns a mutable reference to the middleware.
    pub fn middleware_mut(&mut self) -> &mut W {
        &mut self.middleware
    }

    /// Splits the dispatcher into its matcher and middleware.
    pub fn into_parts(self) -> (M, W) {
        (self.matcher, self.middleware)
    }
}

/// An error raised while dispatching an input.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DispatchError<E> {
    /// No item in the table matched the input.
    NotFound,

    /// Middleware rejected the input before the handler ran.
    Before(E),

    /// The selected handler failed.
    Execute(E),

    /// Middleware failed while processing the handler output.
    After(E),
}

/// Selects table items for an input.
pub trait Matcher<Item, Input> {
    /// Returns `true` when `item` can handle `input`.
    fn matches(&self, item: &Item, input: &Input) -> bool;
}

/// Handles a dispatched input.
pub trait Handler {
    /// Input accepted by the handler.
    type Input;

    /// Value produced by the handler.
    type Output;

    /// Error produced by the handler.
    type Error;

    /// Handles `input` and returns an output.
    fn handle(&self, input: &Self::Input) -> Result<Self::Output, Self::Error>;
}

/// Runs logic immediately before and after the selected handler.
///
/// The input is immutable because item selection has already happened. This
/// prevents middleware from invalidating the match. The output remains mutable
/// so middleware can decorate or normalize successful results.
pub trait Middleware<Item, Input, Output, Error> {
    /// Runs after an item is selected and before its handler is called.
    fn before(&mut self, item: &Item, input: &Input) -> Result<(), Error>;

    /// Runs after the handler succeeds.
    ///
    /// This hook is skipped when [`Self::before`] or [`Handler::handle`] fails.
    fn after(&mut self, item: &Item, input: &Input, output: &mut Output) -> Result<(), Error>;
}

/// Middleware that performs no work.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct NoopMiddleware;

impl<Item, Input, Output, Error> Middleware<Item, Input, Output, Error> for NoopMiddleware {
    fn before(&mut self, _item: &Item, _input: &Input) -> Result<(), Error> {
        Ok(())
    }

    fn after(&mut self, _item: &Item, _input: &Input, _output: &mut Output) -> Result<(), Error> {
        Ok(())
    }
}

/// Dispatches `input` to the first matching item in `table`.
///
/// Execution order is:
///
/// 1. select the first item accepted by the matcher;
/// 2. call [`Middleware::before`];
/// 3. call [`Handler::handle`];
/// 4. call [`Middleware::after`].
///
/// Processing stops at the first error. In particular, `after` only runs when
/// both `before` and the handler succeed.
pub fn dispatch<M, W, Item, Input, Output, Error>(
    dispatcher: &mut Dispatcher<M, W>,
    table: &[Item],
    input: &Input,
) -> Result<Output, DispatchError<Error>>
where
    M: Matcher<Item, Input>,
    W: Middleware<Item, Input, Output, Error>,
    Item: Handler<Input = Input, Output = Output, Error = Error>,
{
    let item = table
        .iter()
        .find(|item| dispatcher.matcher.matches(item, input))
        .ok_or(DispatchError::NotFound)?;

    dispatcher
        .middleware
        .before(item, input)
        .map_err(DispatchError::Before)?;

    let mut output = item.handle(input).map_err(DispatchError::Execute)?;

    dispatcher
        .middleware
        .after(item, input, &mut output)
        .map_err(DispatchError::After)?;

    Ok(output)
}

#[cfg(test)]
mod tests {
    use core::cell::Cell;

    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    enum TestError {
        Before,
        Execute,
        After,
    }

    struct Rule {
        key: u8,
        value: u8,
        fails: bool,
        calls: Cell<u8>,
    }

    impl Handler for Rule {
        type Input = u8;
        type Output = u8;
        type Error = TestError;

        fn handle(&self, _input: &Self::Input) -> Result<Self::Output, Self::Error> {
            self.calls.set(self.calls.get() + 1);
            if self.fails {
                Err(TestError::Execute)
            } else {
                Ok(self.value)
            }
        }
    }

    struct KeyMatcher;

    impl Matcher<Rule, u8> for KeyMatcher {
        fn matches(&self, item: &Rule, input: &u8) -> bool {
            item.key == *input
        }
    }

    #[derive(Default)]
    struct TestMiddleware {
        fail_before: bool,
        fail_after: bool,
        before_calls: u8,
        after_calls: u8,
    }

    impl Middleware<Rule, u8, u8, TestError> for TestMiddleware {
        fn before(&mut self, _item: &Rule, _input: &u8) -> Result<(), TestError> {
            self.before_calls += 1;
            if self.fail_before {
                Err(TestError::Before)
            } else {
                Ok(())
            }
        }

        fn after(&mut self, _item: &Rule, _input: &u8, output: &mut u8) -> Result<(), TestError> {
            self.after_calls += 1;
            if self.fail_after {
                Err(TestError::After)
            } else {
                *output *= 2;
                Ok(())
            }
        }
    }

    fn rule(key: u8, value: u8) -> Rule {
        Rule {
            key,
            value,
            fails: false,
            calls: Cell::new(0),
        }
    }

    #[test]
    fn dispatches_to_the_first_matching_item() {
        let table = [rule(1, 10), rule(1, 20)];
        let mut dispatcher = Dispatcher::new(KeyMatcher, TestMiddleware::default());

        let output = dispatcher.dispatch(&table, &1).unwrap();

        assert_eq!(output, 20);
        assert_eq!(table[0].calls.get(), 1);
        assert_eq!(table[1].calls.get(), 0);
        assert_eq!(dispatcher.middleware().before_calls, 1);
        assert_eq!(dispatcher.middleware().after_calls, 1);
    }

    #[test]
    fn returns_not_found_without_running_middleware() {
        let table = [rule(1, 10)];
        let mut dispatcher = Dispatcher::new(KeyMatcher, TestMiddleware::default());

        let result = dispatcher.dispatch(&table, &2);

        assert_eq!(result, Err(DispatchError::NotFound));
        assert_eq!(dispatcher.middleware().before_calls, 0);
        assert_eq!(dispatcher.middleware().after_calls, 0);
    }

    #[test]
    fn stops_when_before_fails() {
        let table = [rule(1, 10)];
        let middleware = TestMiddleware {
            fail_before: true,
            ..TestMiddleware::default()
        };
        let mut dispatcher = Dispatcher::new(KeyMatcher, middleware);

        let result = dispatcher.dispatch(&table, &1);

        assert_eq!(result, Err(DispatchError::Before(TestError::Before)));
        assert_eq!(table[0].calls.get(), 0);
        assert_eq!(dispatcher.middleware().after_calls, 0);
    }

    #[test]
    fn skips_after_when_handler_fails() {
        let mut failing_rule = rule(1, 10);
        failing_rule.fails = true;
        let table = [failing_rule];
        let mut dispatcher = Dispatcher::new(KeyMatcher, TestMiddleware::default());

        let result = dispatcher.dispatch(&table, &1);

        assert_eq!(result, Err(DispatchError::Execute(TestError::Execute)));
        assert_eq!(table[0].calls.get(), 1);
        assert_eq!(dispatcher.middleware().after_calls, 0);
    }

    #[test]
    fn reports_after_errors() {
        let table = [rule(1, 10)];
        let middleware = TestMiddleware {
            fail_after: true,
            ..TestMiddleware::default()
        };
        let mut dispatcher = Dispatcher::new(KeyMatcher, middleware);

        let result = dispatcher.dispatch(&table, &1);

        assert_eq!(result, Err(DispatchError::After(TestError::After)));
        assert_eq!(table[0].calls.get(), 1);
    }

    #[test]
    fn noop_middleware_requires_no_configuration() {
        let table = [rule(1, 10)];
        let mut dispatcher = Dispatcher::new(KeyMatcher, NoopMiddleware);

        let output = dispatcher.dispatch(&table, &1).unwrap();

        assert_eq!(output, 10);
    }
}
