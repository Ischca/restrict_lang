# Testing

Restrict has two practical testing layers today: repository tests for compiler
development and Warder smoke tests for user projects.

## Repository Tests

From the repository root, use the project-managed toolchain:

```bash
mise run test-fast
```

Useful focused commands:

```bash
mise exec -- cargo test --test quality_gates test_docs_hygiene::
mise exec -- cargo test --test integration_07 test_wat_validation::
mise run check
mise run preflight
mise run preflight-pages
```

`mise run test-fast` is the normal local gate for compiler work. It runs
formatting, focused library checks, and one combined Cargo invocation for docs,
release-surface, sample, and generic integration tests without the slow release
example CLI sweep. `mise run preflight` is the one-command merge gate: it runs
cheap checks first, then the complete workspace suite, ignored release example
checks, and the checked-in browser runtime smoke. Use `mise run
preflight-pages` for a publication handoff that must also rebuild the mdBook,
WebAssembly package, and assembled Pages artifact.

GitHub Actions runs the same coverage as separate quality, compiler-core,
integration, Warder, and release-gate jobs. Integration test targets are split
deterministically across four shards, while the final `test` job preserves the
stable branch-protection check name.

`mise run check` executes the slow release example CLI entrypoint validation.
Those tests are ignored in the default `cargo test` run so active compiler
iteration stays fast while CI/release checks keep full coverage.

`test_docs_hygiene` checks public documentation and examples for removed syntax
such as `let`, `fn`, function-first calls, stale record initializers, and
unsupported import forms. That test is intentionally part of the docs workflow:
examples should not drift away from the language specification.

## Warder Project Smoke Tests

A Warder project normally contains `.rl` files under `tests/`:

```text
hello-world/
├── src/main.rl
└── tests/main_test.rl
```

Run:

```bash
warder test
```

For v0.0.1, Warder test files are type-checking smoke files. They use ordinary
Restrict functions rather than a dedicated test declaration syntax:

```restrict
fun test_math_smoke: () -> Boolean = {
    2 + 2 == 4
}

fun test_pipeline_smoke: () -> Int32 = {
    val value = 41;
    value + 1
}
```

## Runtime Examples

Compiler tests also execute selected examples through WebAssembly runtimes where
possible. Keep host-visible runtime examples scalar at the boundary:

```restrict
export fun exported_score: () -> Int32 = {
    42
}
```

Composite values are useful inside examples, but exported records, strings,
lists, `Option`, `Result`, and generic functions remain outside the v0.0.1 host
ABI.

## Documentation Examples

When adding docs:

- use `val`, never `let`
- use OSV calls, never `function(args)`
- use `mut val`, never `val mut`
- use `:` in record fields and record literals
- avoid stdin, filesystem, and network APIs in quick-start runnable examples
- keep current user enum examples closed, non-generic, and non-recursive, with
  qualified variants and exhaustive matches
- keep form examples method-only, adoptions concrete and non-generic, and `of`
  bounds explicit
- mark TAT, generic or recursive enums, enum host ABI, associated types,
  generic/conditional/enum adoptions, dynamic dispatch, and composite host ABI
  as future work

The docs tests are meant to enforce those rules automatically.
