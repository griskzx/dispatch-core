use dispatch_core::{Dispatcher, Handler, Matcher};

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
struct AppContext {
    execution_count: u32,
}

type RouteHandler = fn(&Request, &mut AppContext) -> Result<Response, Error>;

struct Route {
    command: Command,
    handler: RouteHandler,
}

impl Handler for Route {
    type Input = Request;
    type Context = AppContext;
    type Output = Response;
    type Error = Error;

    fn handle(
        &self,
        request: &Self::Input,
        context: &mut Self::Context,
    ) -> Result<Self::Output, Self::Error> {
        (self.handler)(request, context)
    }
}

struct CommandMatcher;

impl Matcher<Route, Request> for CommandMatcher {
    fn matches(&self, route: &Route, request: &Request) -> bool {
        route.command == request.command
    }
}

fn read(request: &Request, context: &mut AppContext) -> Result<Response, Error> {
    context.execution_count += 1;
    Ok(Response {
        value: request.value,
    })
}

fn double(request: &Request, context: &mut AppContext) -> Result<Response, Error> {
    context.execution_count += 1;
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
    let mut context = AppContext::default();
    let request = Request {
        command: Command::Write,
        value: 21,
    };

    let response = dispatcher.dispatch(&request, &mut context).unwrap();

    assert_eq!(response, Response { value: 42 });
    assert_eq!(context.execution_count, 1);
    println!(
        "value={}, execution_count={}",
        response.value, context.execution_count
    );
}
