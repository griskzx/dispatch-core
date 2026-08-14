use core::cell::Cell;

use dispatch_core::{DispatchError, Dispatcher, Handler, Matcher};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestError {
    Failed,
}

#[derive(Default)]
struct TestContext {
    total: u16,
}

struct Rule {
    key: u8,
    value: u8,
    fails: bool,
    calls: Cell<u8>,
}

impl Handler for Rule {
    type Input = u8;
    type Context = TestContext;
    type Output = u8;
    type Error = TestError;

    fn handle(
        &self,
        _input: &Self::Input,
        context: &mut Self::Context,
    ) -> Result<Self::Output, Self::Error> {
        self.calls.set(self.calls.get() + 1);

        if self.fails {
            return Err(TestError::Failed);
        }

        context.total += u16::from(self.value);
        Ok(self.value)
    }
}

struct CountingMatcher {
    calls: Cell<u8>,
}

impl Matcher<Rule, u8> for CountingMatcher {
    fn matches(&self, item: &Rule, input: &u8) -> bool {
        self.calls.set(self.calls.get() + 1);
        item.key == *input
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
    let matcher = CountingMatcher {
        calls: Cell::new(0),
    };
    let dispatcher = Dispatcher::new(&table, matcher);
    let mut context = TestContext::default();

    let output = dispatcher.dispatch(&1, &mut context).unwrap();

    assert_eq!(output, 10);
    assert_eq!(context.total, 10);
    assert_eq!(table[0].calls.get(), 1);
    assert_eq!(table[1].calls.get(), 0);
    assert_eq!(dispatcher.matcher().calls.get(), 1);
}

#[test]
fn select_does_not_execute_the_item() {
    let table = [rule(1, 10)];
    let dispatcher = Dispatcher::new(&table, |item: &Rule, input: &u8| item.key == *input);

    let selected = dispatcher.select(&1).unwrap();

    assert_eq!(selected.value, 10);
    assert_eq!(selected.calls.get(), 0);
}

#[test]
fn returns_not_found_without_changing_context() {
    let table = [rule(1, 10)];
    let dispatcher = Dispatcher::new(&table, |item: &Rule, input: &u8| item.key == *input);
    let mut context = TestContext::default();

    let result = dispatcher.dispatch(&2, &mut context);

    assert_eq!(result, Err(DispatchError::NotFound));
    assert_eq!(context.total, 0);
    assert_eq!(table[0].calls.get(), 0);
}

#[test]
fn wraps_handler_errors() {
    let mut failing_rule = rule(1, 10);
    failing_rule.fails = true;
    let table = [failing_rule];
    let dispatcher = Dispatcher::new(&table, |item: &Rule, input: &u8| item.key == *input);
    let mut context = TestContext::default();

    let result = dispatcher.dispatch(&1, &mut context);

    assert_eq!(result, Err(DispatchError::Execute(TestError::Failed)));
    assert_eq!(context.total, 0);
    assert_eq!(table[0].calls.get(), 1);
}

struct PureRule;

impl Handler for PureRule {
    type Input = u8;
    type Context = ();
    type Output = u8;
    type Error = TestError;

    fn handle(
        &self,
        input: &Self::Input,
        _context: &mut Self::Context,
    ) -> Result<Self::Output, Self::Error> {
        Ok(input.saturating_mul(2))
    }
}

#[test]
fn supports_handlers_without_runtime_context() {
    let table = [PureRule];
    let dispatcher = Dispatcher::new(&table, |_item: &PureRule, _input: &u8| true);

    assert_eq!(dispatcher.dispatch(&21, &mut ()), Ok(42));
}

#[test]
fn exposes_and_returns_its_configuration() {
    let table = [rule(1, 10)];
    let mut dispatcher = Dispatcher::new(
        &table,
        CountingMatcher {
            calls: Cell::new(0),
        },
    );

    assert_eq!(dispatcher.table().len(), 1);
    dispatcher.matcher_mut().calls.set(3);
    assert_eq!(dispatcher.matcher().calls.get(), 3);

    let (returned_table, returned_matcher) = dispatcher.into_parts();
    assert_eq!(returned_table.len(), 1);
    assert_eq!(returned_matcher.calls.get(), 3);
}
