use core::{cell::Cell, convert::Infallible, ptr};

use dispatch_core::{DispatchError, Dispatcher, Matcher};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TestError {
    Failed,
}

struct Rule {
    key: u8,
    value: u8,
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
    Rule { key, value }
}

#[test]
fn dispatches_to_the_first_matching_item() {
    let table = [rule(1, 10), rule(1, 20)];
    let matcher = CountingMatcher {
        calls: Cell::new(0),
    };
    let dispatcher = Dispatcher::new(&table, matcher);
    let executor_calls = Cell::new(0);

    let output = dispatcher
        .dispatch(&1, |item, input| {
            executor_calls.set(executor_calls.get() + 1);
            assert_eq!(*input, 1);
            Ok::<_, Infallible>(item.value)
        })
        .unwrap();

    assert_eq!(output, 10);
    assert_eq!(executor_calls.get(), 1);
    assert_eq!(dispatcher.matcher().calls.get(), 1);
}

#[test]
fn select_does_not_execute_application_logic() {
    let table = [rule(1, 10)];
    let dispatcher = Dispatcher::new(&table, |item: &Rule, input: &u8| item.key == *input);

    let selected = dispatcher.select(&1).unwrap();

    assert_eq!(selected.value, 10);
}

#[test]
fn not_found_does_not_invoke_the_executor() {
    let table = [rule(1, 10)];
    let dispatcher = Dispatcher::new(&table, |item: &Rule, input: &u8| item.key == *input);
    let executor_called = Cell::new(false);

    let result = dispatcher.dispatch(&2, |_item, _input| {
        executor_called.set(true);
        Ok::<_, Infallible>(())
    });

    assert_eq!(result, Err(DispatchError::NotFound));
    assert!(!executor_called.get());
}

#[test]
fn wraps_executor_errors() {
    let table = [rule(1, 10)];
    let dispatcher = Dispatcher::new(&table, |item: &Rule, input: &u8| item.key == *input);

    let result: Result<(), DispatchError<TestError>> =
        dispatcher.dispatch(&1, |_item, _input| Err(TestError::Failed));

    assert_eq!(result, Err(DispatchError::Execute(TestError::Failed)));
}

#[test]
fn executor_can_capture_short_lived_mutable_state() {
    let table = [rule(1, 10)];
    let dispatcher = Dispatcher::new(&table, |item: &Rule, input: &u8| item.key == *input);
    let mut total = 5_u16;

    let output = dispatcher
        .dispatch(&1, |item, _input| {
            total += u16::from(item.value);
            Ok::<_, Infallible>(item.value)
        })
        .unwrap();

    assert_eq!(output, 10);
    assert_eq!(total, 15);
}

#[test]
fn executor_can_return_a_value_borrowed_from_the_item() {
    let table = [rule(1, 10)];
    let dispatcher = Dispatcher::new(&table, |item: &Rule, input: &u8| item.key == *input);

    let value = dispatcher
        .dispatch(&1, |item, _input| Ok::<_, Infallible>(&item.value))
        .unwrap();

    assert!(ptr::eq(value, &table[0].value));
}

#[test]
fn executor_can_return_the_borrowed_input() {
    let table = [rule(1, 10)];
    let dispatcher = Dispatcher::new(&table, |item: &Rule, input: &u8| item.key == *input);
    let input = 1_u8;

    let returned_input = dispatcher
        .dispatch(&input, |_item, input| Ok::<_, Infallible>(input))
        .unwrap();

    assert!(ptr::eq(returned_input, &input));
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
