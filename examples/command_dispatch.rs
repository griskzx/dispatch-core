use dispatch_core::{Dispatcher, Matcher};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Read,
    Write,
}

struct Request {
    command: Command,
    value: u32,
}

#[derive(Debug, PartialEq, Eq)]
struct Response {
    value: u32,
}

#[derive(Debug, PartialEq, Eq)]
enum Error {
    Overflow,
}

#[derive(Default)]
struct AppState {
    execution_count: u32,
}

type RouteHandler = fn(&Request, &mut AppState) -> Result<Response, Error>;

struct Route {
    command: Command,
    handler: RouteHandler,
}

struct CommandMatcher;

impl Matcher<Route, Request> for CommandMatcher {
    fn matches(&self, route: &Route, request: &Request) -> bool {
        route.command == request.command
    }
}

fn read(request: &Request, state: &mut AppState) -> Result<Response, Error> {
    state.execution_count += 1;
    Ok(Response {
        value: request.value,
    })
}

fn double(request: &Request, state: &mut AppState) -> Result<Response, Error> {
    state.execution_count += 1;
    let value = request.value.checked_mul(2).ok_or(Error::Overflow)?;
    Ok(Response { value })
}

fn main() {
    let routes = [
        Route {
            command: Command::Read,
            handler: read,
        },
        Route {
            command: Command::Write,
            handler: double,
        },
    ];
    let dispatcher = Dispatcher::new(&routes, CommandMatcher);
    let mut state = AppState::default();
    let request = Request {
        command: Command::Write,
        value: 21,
    };

    let response = dispatcher
        .dispatch(&request, |route, request| {
            (route.handler)(request, &mut state)
        })
        .unwrap();

    assert_eq!(response, Response { value: 42 });
    assert_eq!(state.execution_count, 1);
    println!(
        "value={}, execution_count={}",
        response.value, state.execution_count
    );
}
