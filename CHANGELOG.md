# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- Added Bevy-inspired resource parameter injection through `SystemParam`.
- Added `command!` for erasing typed command functions into static-table
  function pointers.
- Added `resource_param!` for audited shared and exclusive field access.
- Added structured parameter fetch and access-conflict errors.
- Added exact context-key dispatch and custom matching strategies.

### Changed

- Redesigned `Dispatcher` around application-owned Context and Resources.
- Changed dispatch to select, validate resources, inject arguments, and invoke
  commands as one operation.
- Allowed carefully bounded unsafe code for direct resource-field projection,
  with access metadata validated before references are created.

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

[Unreleased]: https://github.com/griskzx/dispatch-core/compare/v0.3.0...HEAD
[0.3.0]: https://github.com/griskzx/dispatch-core/compare/v0.2.0...v0.3.0
[0.2.0]: https://github.com/griskzx/dispatch-core/compare/v0.1.0...v0.2.0
[0.1.0]: https://github.com/griskzx/dispatch-core/releases/tag/v0.1.0
