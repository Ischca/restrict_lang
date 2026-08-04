# Quick Start Guide

This guide uses the v0.0.1 Warder CLI surface: `new`, `init`, `add`,
`remove`, `build`, `run`, `test`, `publish`, `wrap`, `unwrap`, and `doctor`.

## Verify Tools

After installing or building from source, check that both binaries are on your
`PATH`:

```bash
restrict_lang --version
warder --version
```

When using a local workspace build, both binaries are under `target/release`.

## Create A Project

```bash
warder new hello-world
cd hello-world
```

This creates:

```text
hello-world/
├── package.rl.toml
├── src/
│   └── main.rl
├── tests/
│   └── main_test.rl
├── README.md
└── .gitignore
```

The generated `src/main.rl` uses current Restrict syntax:

```restrict
fun main: () -> () = {
    "Hello, World!" |> println
}
```

## Check The Manifest

`package.rl.toml` controls the entry point, dependency table, and build output:

```toml
[package]
name = "hello-world"
version = "0.1.0"
description = "A first Restrict package"
authors = ["Your Name <you@example.com>"]
entry = "src/main.rl"
edition = "2025"

[dependencies]

[build]
target = "wasm32"
optimize = true
output = "dist/"
```

## Build And Run

```bash
warder build
warder run
warder test
```

The default build writes:

```text
dist/hello-world-0.1.0.wat
dist/hello-world-0.1.0.wasm
dist/hello-world-0.1.0.rgc
```

`warder run` builds first, then runs the generated `.wasm` with Wasmtime or
Wasmer when one is installed. `warder test` type-checks `.rl` files under
`tests/`; v0.0.1 does not include a dedicated test declaration syntax.

## Add A Small Pipeline

Replace `src/main.rl` with:

```restrict
fun add_bonus: (base: Int32, bonus: Int32) -> Int32 = {
    base + bonus
}

fun main: () -> () = {
    val total = (21, 4) add_bonus
    total |> print_int
}
```

Run it:

```bash
warder run
```

Host networking and file or stream I/O are outside the v0.0.1 quick-start
runnable path. Keep quick-start programs limited to the compiler-registered
surface such as `println` and `print_int`.

## Dependencies

```bash
warder add local_utils --path ../local-utils
warder remove local_utils
```

The dependency directory must contain `package.rl.toml` and `src/lib.rl`. The
alias `local_utils` is also its source namespace, so an exported `score`
function from the library root is imported with:

```restrict
import local_utils.{score}
```

Submodules follow the same mapping: `import local_utils.numbers.{double}` loads
`../local-utils/src/numbers.rl`. Aliases must be non-keyword Restrict
identifiers, `std` is reserved, and hyphens are not converted automatically.

`warder build` refreshes `restrict-lock.toml` with the dependency manifest
version and a deterministic source SHA-256. `warder test` uses the same local
package roots. Registry, Git, foreign WASM, and transitive dependencies fail
explicitly in the current direct-local slice.

Builds compile immutable application and dependency snapshots. Concurrent
builds of one project are serialized, and the generated WAT, WASM, Cage, and
lock file replace the previous set only after the full build succeeds.

## Local Cage Files

`warder build` creates a local cage file at
`dist/<name>-<version>.rgc`; there is no separate packaging subcommand.

For external WASM files:

```bash
warder wrap module.wasm --name module-name --version 0.1.0
warder unwrap module-name-0.1.0.rgc
```

Foreign WASM wrapping is experimental in v0.0.1 and is intended for local
evaluation.

## Diagnostics

```bash
warder doctor
```

`warder doctor` checks the manifest, entry source path, lock file integrity when
present, required compiler, and available WASM runtime.

## Next Steps

- Read the [Language Guide](../guide/README.md)
- Learn more about [Warder](../guide/warder.md)
- Explore the [Standard Library](../reference/stdlib.md)
