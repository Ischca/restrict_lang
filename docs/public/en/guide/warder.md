# Warder

Warder is the project tool for Restrict Language v0.0.1. It creates project
layouts, edits `package.rl.toml`, builds Restrict source through the compiler,
creates local cage files, and runs basic project checks.

## Commands

The v0.0.1 CLI exposes these subcommands:

| Command | Description |
|---------|-------------|
| `warder new <name>` | Create a project directory |
| `warder init` | Initialize the current directory |
| `warder add <dep>` | Add a dependency |
| `warder remove <name>` | Remove a dependency |
| `warder build` | Build the package |
| `warder run [args...]` | Build, then run the generated WASM |
| `warder test [filter]` | Type-check `.rl` files under `tests/` |
| `warder publish` | Run publish preflight checks |
| `warder wrap <wasm>` | Wrap a WASM file into an experimental cage |
| `warder unwrap <cage>` | Extract a cage |
| `warder doctor` | Check project structure and tools |

The table above is the complete v0.0.1 subcommand surface.

## Projects

Create a new project:

```bash
warder new my-project
cd my-project
```

This creates:

```text
my-project/
├── package.rl.toml
├── src/
│   └── main.rl
├── tests/
│   └── main_test.rl
├── README.md
└── .gitignore
```

Initialize an existing directory:

```bash
warder init
```

`warder init` writes `package.rl.toml`, creates `src/` and `tests/` when
needed, and adds a starter `src/main.rl` only when there is no existing source
entry point.

## Manifest

`package.rl.toml` is the project manifest. A typical v0.0.1 manifest is:

```toml
[package]
name = "my-project"
version = "0.1.0"
description = "A short package description"
authors = ["Your Name <you@example.com>"]
entry = "src/main.rl"
edition = "2025"

[dependencies]
math = "0.1.0"
local_utils = { path = "../local-utils" }
json = { git = "https://example.com/json.git", tag = "v1.0.0" }
foreign_module = { wasm = "https://example.com/module.wasm", wit = "https://example.com/module.wit" }

[build]
target = "wasm32"
optimize = true
output = "dist/"
```

The `package` table names the package, version, entry source file, and edition.
`description` and `authors` are optional metadata fields.

The `dependencies` table supports registry versions, local paths, Git
repositories, and foreign WASM plus WIT references.

The `build` table controls the target name, whether optimization is requested,
and the output directory. The current default output directory is `dist/`.

## Dependencies

Add a registry dependency:

```bash
warder add http
warder add json@1.0.0
```

Add a local dependency:

```bash
warder add local-utils --path ../local-utils
```

Add a Git dependency:

```bash
warder add json@v1.0.0 --git https://example.com/json.git
```

Add a foreign WASM dependency:

```bash
warder add foreign-module --wasm https://example.com/module.wasm --wit https://example.com/module.wit
```

Remove a dependency:

```bash
warder remove http
```

`warder build` updates `restrict-lock.toml` from the manifest dependency table.

## Build Outputs

Build the project:

```bash
warder build
```

For a package named `my-project` at version `0.1.0`, the default outputs are:

```text
dist/my-project-0.1.0.wat
dist/my-project-0.1.0.wasm
dist/my-project-0.1.0.rgc
```

The output stem is always `<name>-<version>`. Change the output directory with
the manifest `build.output` field.

The build target is read from `package.rl.toml`, not from a command-line flag.

## Run And Test

Run the built program through an installed WASM runtime:

```bash
warder run
warder run -- arg1 arg2
```

`warder run` builds first, then looks for the generated
`dist/<name>-<version>.wasm`.

Run tests:

```bash
warder test
warder test main
```

In v0.0.1, `warder test` type-checks `.rl` files under `tests/`. There is no
dedicated test declaration syntax yet.

## Publish Preflight

```bash
warder publish
warder publish --registry https://example.com/registry
```

For v0.0.1, `warder publish` performs a release-style preflight build and
metadata validation. Registry upload, authentication, and signing remain
experimental and no package is uploaded.

## Cage Commands

Builds already write a local `.rgc` cage next to the `.wat` and `.wasm` files.

Wrap an external WASM module for local evaluation:

```bash
warder wrap module.wasm --name module-name --version 0.1.0
warder wrap module.wasm --name module-name --version 0.1.0 --wit interface.wit --output module-name-0.1.0.rgc
```

Extract a cage:

```bash
warder unwrap module-name-0.1.0.rgc
warder unwrap module-name-0.1.0.rgc --output extracted-module
```

Foreign WASM wrapping and component conversion are experimental in v0.0.1.

## Doctor

Check the current project:

```bash
warder doctor
```

`warder doctor` validates the manifest, checks that the entry source exists,
loads `restrict-lock.toml` when present, checks for a Restrict compiler, and
reports whether a WASM runtime such as Wasmtime or Wasmer is available.
