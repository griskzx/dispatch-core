# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.4.0] - 2026-08-16

### Added

- Added Bevy-inspired `Res<T>` and `ResMut<T>` command parameters.
- Added `command!` for erasing typed command functions into static-table
  function pointers.
- Added `resources!` and `ResourceProvider` for typed resource containers.
- Added resource tags for distinguishing fields with the same type.
- Added structured parameter fetch and access-conflict errors.
- Added exact context-key dispatch and custom matching strategies.
- Added independently configurable `before` and `after` functions with the
  same typed resource injection as commands.
- Added a response stage that consumes successful command output and can
  convert it into a different final type before returning to the caller.
- Added stage-aware parameter errors and distinct application errors for
  before, command, after, and response execution.

### Changed

- Redesigned `Dispatcher` around application-owned Context and Resources.
- Changed dispatch to select, validate resources, inject arguments, and invoke
  commands as one operation.
- Made `command!` infer resource requirements from handler signatures, keeping
  static tables limited to `key => handler` routing declarations.
- Allowed carefully bounded unsafe code for direct resource-field projection,
  with access metadata validated before references are created.
- Replaced the former combined middleware model with independently composable,
  zero-allocation pipeline stages.

### Removed

- Removed caller-provided executor closures from the dispatch path.

## [0.3.0] - 2026-08-14

### Changed

- Redesigned `Dispatcher::dispatch` to delegate execution to a caller-provided closure.
- Made execution state and borrowed resources entirely application-defined.
- Allowed executor outputs to borrow from the selected item or original input.

### Removed

- Removed the `Handler` trait and its fixed input, context, output, and error model.

## [0.2.0] - 2026-08-14

### Changed

- Redesigned `Dispatcher` to borrow its dispatch table at construction time.
- Added an explicit mutable handler context while keeping dispatch inputs immutable.
- Made the dispatcher usable through a shared reference during dispatch.
- Added selection without execution through `Dispatcher::select`.
- Simplified dispatch errors to selection and execution failures.
- Added closure support for matchers.

### Removed

- Removed built-in before/after middleware and `NoopMiddleware`.
- Removed the duplicate free `dispatch` function.

## [0.1.0] - 2026-08-13

### Added

- A `no_std`, allocation-free, table-driven dispatcher.
- Matcher, handler, and before/after middleware abstractions.
- Structured errors for selection and execution stages.
- No-op middleware for applications without hooks.
- Unit tests and a runnable command-dispatch example.

[Unreleased]: https://github.com/griskzx/dispatch-core/compare/v0.4.0...HEAD
[0.4.0]: https://github.com/griskzx/dispatch-core/compare/v0.3.0...v0.4.0
[0.3.0]: https://github.com/griskzx/dispatch-core/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/griskzx/dispatch-core/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/griskzx/dispatch-core/releases/tag/v0.1.0
