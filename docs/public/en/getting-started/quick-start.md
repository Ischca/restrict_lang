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

When using a local source build, the binaries are normally under
`target/release`. If Warder was built from its package directory separately,
also check `warder/target/release`.

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
warder add math
warder add json@1.0.0
warder add local-utils --path ../local-utils
warder add json@v1.0.0 --git https://example.com/json.git
warder remove math
```

`warder build` refreshes `restrict-lock.toml` from `package.rl.toml`.

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
