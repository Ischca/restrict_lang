# Internal Testing Guidelines

**Status:** Current repository policy

This document defines how compiler and tooling tests should be added without
reintroducing excessive Rust linking or nondeterministic parallel execution.
Public language behavior remains governed by `LANGUAGE_SPECIFICATION.md`.

## Test Layout

Root compiler integration tests use two layers:

```text
tests/
├── cases/                 individual test source files
├── integration_01.rs      shared Cargo test harnesses
├── integration_03.rs
├── integration_05.rs
├── integration_07.rs
└── quality_gates.rs       focused merge-critical cases
```

Every top-level Rust file directly under `tests/` becomes a separate Cargo test
target and therefore a separately linked executable. Do not add one top-level
file per feature. Put the implementation in `tests/cases/test_<topic>.rs` and
register it as a module in one existing `integration_*.rs` harness.

Add a new harness only when an existing harness cannot provide the isolation a
test requires. A new harness needs a short explanation in the pull request
because it permanently adds another link step to local and CI test builds.

`quality_gates.rs` is reserved for a small set of merge-critical checks and
focused sample/generic coverage used by `mise run test-fast` and `mise run
preflight`. Do not add a test there merely because it is important. Add it only
when it must run in the quick gate and its runtime remains bounded.

## Choosing the Test Layer

- Put parser, type-checker, IR, and code-generation behavior next to the Rust
  module as a unit test when it does not need a public crate boundary.
- Use `tests/cases/` when the test needs the public library API, compiled CLI,
  repository fixtures, or a WebAssembly runtime.
- Use Warder's own unit or integration tests for package-manager behavior. Do
  not make the compiler suite build Warder merely to test Warder internals.
- Keep documentation hygiene in `quality_gates.rs`; public examples are part of
  the release surface and should fail quickly when syntax drifts.

Before adding or changing Restrict source examples, read
`LANGUAGE_SPECIFICATION.md`. Tests must use current OSV calls, `val`, `mut val`,
colon record fields, and the documented statement-boundary rules.

## Isolation and Concurrency

Compilation and linking may run in parallel. Test execution that touches shared
state must not.

- Do not launch Cargo test executables manually with `xargs -P`, background
  jobs, or another process-level fan-out.
- Repository commands pass `--test-threads=1` for integration and Warder suites.
  Cargo can still use its normal parallel build jobs before execution starts.
- Use `tempfile::TempDir` for writable projects and directories.
- If a test writes directly to the system temporary directory, include both a
  descriptive test-specific stem and `std::process::id()` in every path.
- Never write generated artifacts to a checked-in fixture directory.
- Do not depend on another test having run first or on a test leaving state
  behind.
- Avoid fixed ports. Prefer an OS-assigned port or a socket inside a temporary
  directory.
- A test for locking or concurrent behavior should create concurrency inside
  one isolated temporary project, then wait on an explicit readiness signal.

If a test is safe only under serial execution, explain the shared resource in a
comment near the test. Serial execution is a correctness boundary, not a way to
hide an unexplained race.

## Commands

Use the repository-managed Rust toolchain through `mise`:

```bash
# Normal compiler work
mise run test-fast

# Complete compiler suite, serial execution with parallel compilation
mise run test

# Warder only
mise run test-warder

# Compiler, Warder, and slow release entrypoints
mise run test-full

# Merge preflight
mise run preflight
```

Focused examples:

```bash
mise exec -- cargo test --lib parser::tests::
mise exec -- cargo test --test integration_03 test_generics:: -- --test-threads=1
mise exec -- cargo test --test quality_gates test_docs_hygiene:: -- --test-threads=1
```

When adding `--exact`, include the module-qualified test name, for example
`test_release_example_hygiene::standalone_release_examples_compile_through_cli`.
Cargo exits successfully when a filter matches zero tests, so `running 0 tests`
is an invalid focused-test result rather than evidence that the check passed.

Do not add `-j 1` to avoid test races. Cargo's `-j` option controls build jobs,
not libtest execution. Use `-- --test-threads=1` at the test-harness boundary.

## Adding a Test Case

1. Decide whether a unit test is sufficient.
2. For an integration test, create `tests/cases/test_<topic>.rs`.
3. Add one `#[path = "cases/test_<topic>.rs"] mod test_<topic>;` entry to an
   existing harness.
4. Use unique temporary paths and avoid shared mutable repository state.
5. Run the containing harness with one test thread.
6. Run `mise run test-fast`.
7. Run `mise run preflight` before merge when compiler or release behavior
   changes.

Keep test names behavioral and specific. Prefer
`wasm_core_rejects_host_output` over `test_codegen_2`. A failure should state
the source behavior, expected diagnostic or result, and the actual output.

## Slow and Release Tests

Tests that compile every standalone example or publication fixture may use
`#[ignore]` when they are covered by `mise run check`, the full preflight, and
CI release gates. The ignore reason should name the command that runs the test.
Do not mark a flaky or unexpectedly slow test ignored without identifying its
underlying cost or race.

Benchmark workloads are correctness-checked but remain separate from ordinary
tests. Use `mise run bench-smoke` for benchmark infrastructure validation and
`mise run bench` only when recording a local baseline.
