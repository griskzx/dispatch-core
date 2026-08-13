use dispatch_core::{Dispatcher, Handler, Matcher, Middleware};

/// 本例支持的输入命令。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Command {
    Read,
    Write,
}

/// 每次分发时携带的输入。
struct Request {
    command: Command,
    authenticated: bool,
    value: u32,
}

/// Handler 执行成功后的输出。
#[derive(Debug, PartialEq, Eq)]
struct Response {
    value: u32,
    audited: bool,
}

#[derive(Debug, PartialEq, Eq)]
enum Error {
    Unauthorized,
}

/// 分发表中的一项。
///
/// 每一项既包含匹配所需的命令，也实现具体的执行逻辑。
struct Route {
    command: Command,
    operation: Operation,
}

enum Operation {
    Read,
    Double,
}

impl Handler for Route {
    type Input = Request;
    type Output = Response;
    type Error = Error;

    fn handle(&self, request: &Request) -> Result<Response, Error> {
        let value = match self.operation {
            Operation::Read => request.value,
            Operation::Double => request.value * 2,
        };

        Ok(Response {
            value,
            audited: false,
        })
    }
}

/// 根据请求中的 command 选择 Route。
struct CommandMatcher;

impl Matcher<Route, Request> for CommandMatcher {
    fn matches(&self, route: &Route, request: &Request) -> bool {
        route.command == request.command
    }
}

/// 前置与后置处理。
struct SecurityAndAudit {
    before_count: u32,
    after_count: u32,
}

impl Middleware<Route, Request, Response, Error> for SecurityAndAudit {
    fn before(&mut self, _route: &Route, request: &Request) -> Result<(), Error> {
        self.before_count += 1;

        // 前置处理：Handler 执行前进行权限检查。
        if !request.authenticated {
            return Err(Error::Unauthorized);
        }

        Ok(())
    }

    fn after(
        &mut self,
        _route: &Route,
        _request: &Request,
        response: &mut Response,
    ) -> Result<(), Error> {
        self.after_count += 1;

        // 后置处理：Handler 成功后加工输出。
        response.audited = true;
        Ok(())
    }
}

fn main() {
    let routes = [
        Route {
            command: Command::Read,
            operation: Operation::Read,
        },
        Route {
            command: Command::Write,
            operation: Operation::Double,
        },
    ];

    let middleware = SecurityAndAudit {
        before_count: 0,
        after_count: 0,
    };
    let mut dispatcher = Dispatcher::new(CommandMatcher, middleware);

    let request = Request {
        command: Command::Write,
        authenticated: true,
        value: 21,
    };

    // 也可以写成 dispatch_core::dispatch(&mut dispatcher, &routes, &request)。
    let response = dispatcher.dispatch(&routes, &request).unwrap();

    assert_eq!(
        response,
        Response {
            value: 42,
            audited: true,
        }
    );
    assert_eq!(dispatcher.middleware().before_count, 1);
    assert_eq!(dispatcher.middleware().after_count, 1);

    println!("value={}, audited={}", response.value, response.audited);
}
