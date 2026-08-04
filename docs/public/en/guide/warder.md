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
| `warder add <alias> --path <dir>` | Add a direct local path dependency |
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
local_utils = { path = "../local-utils" }

[build]
target = "wasm32"
optimize = true
output = "dist/"
```

The `package` table names the package, version, entry source file, and edition.
`description` and `authors` are optional metadata fields.

The dependency-table key is the source namespace used by `import`. In the
current v0.0.1 buildable slice, every dependency must be a **direct local path
dependency**. The alias must be one non-keyword Restrict identifier, so use
`local_utils` rather than `local-utils`; Warder never converts hyphens
implicitly. The alias `std` is reserved.

Registry versions, Git repositories, foreign WASM dependencies, and
dependencies declared by a local dependency are not resolved yet. `warder
add`, `warder build`, and `warder test` reject those forms explicitly instead
of writing placeholder lock entries. Use `warder wrap` separately for
experimental local evaluation of a foreign WASM file.

The `build` table controls the target name, whether optimization is requested,
and the output directory. The current default output directory is `dist/`.

## Dependencies

Add a local dependency:

```bash
warder add local_utils --path ../local-utils
```

The referenced package must contain its own manifest and library root:

```text
local-utils/
├── package.rl.toml
└── src/
    ├── lib.rl
    └── numbers.rl
```

The dependency manifest supplies the version recorded in the lock file. A
minimal `src/lib.rl` might export a function:

```restrict
pub fun score: () -> Int32 = {
    42
}
```

Remove a dependency:

```bash
warder remove local_utils
```

### Package namespaces

The manifest binding mounts the dependency's `src/` directory under its alias:

| Source import | Dependency file |
|---------------|-----------------|
| `import local_utils.{score}` | `../local-utils/src/lib.rl` |
| `import local_utils` | `../local-utils/src/lib.rl` |
| `import local_utils.numbers.{double}` | `../local-utils/src/numbers.rl` |

An unqualified import inside a dependency stays package-local. For example,
`import numbers.{double}` inside `local-utils/src/lib.rl` resolves to that
package's `src/numbers.rl`, not an application module.

This is a manifest-to-compiler namespace binding. It does not add source-level
`import ... as` aliases or re-exports; both remain unsupported.

On a successful build, Warder rewrites `restrict-lock.toml` with the local
package's manifest version and a deterministic SHA-256 over its manifest and
Restrict source files. Dependency validation happens first, so an unsupported
or invalid dependency does not create a fake lock entry.

Warder compiles immutable snapshots of both the application source tree and
each dependency source tree. It rejects overlapping application, dependency,
and output roots. Builds for one project are serialized, and the WAT, WASM,
Cage, and lock file are published together through a recoverable transaction;
a failed compile keeps the previous artifact set intact.

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

In v0.0.1, `warder test` type-checks `.rl` files under `tests/`. It resolves the
same direct-local dependency roots as `warder build`, so test files can use the
same package imports. Compiler fallback resolution is anchored at the project
root, independent of the directory from which Warder was invoked. There is no
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
loads `restrict-lock.toml` when present, and detects missing, malformed, or
source-stale locks for direct local dependencies. It also checks for a Restrict
compiler and reports whether a WASM runtime such as Wasmtime or Wasmer is
available.
