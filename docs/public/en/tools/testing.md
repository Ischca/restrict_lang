# Testing

Restrict has two practical testing layers today: repository tests for compiler
development and Warder smoke tests for user projects.

## Repository Tests

From the repository root, use the project-managed toolchain:

```bash
mise exec -- cargo test
```

Useful focused commands:

```bash
mise exec -- cargo test --test test_docs_hygiene
mise exec -- cargo test --test test_wat_validation
mise exec -- cargo test --test test_release_example_hygiene
```

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
    val value = 41
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
- mark TAT, user ADTs, `form`/`takes`, and composite host ABI as future work

The docs tests are meant to enforce those rules automatically.
