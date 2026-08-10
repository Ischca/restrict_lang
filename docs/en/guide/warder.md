# Warder

Warder is the project tool for Restrict Language v0.0.1. It creates project
layouts, edits the repository-root `warder.toml`, builds Restrict source
through the compiler, creates local cage files, and runs basic project checks.

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
| `warder test [filter]` | Compile and execute Boolean tests under `tests/` |
| `warder publish` | Run publish preflight checks |
| `warder wrap <wasm>` | Wrap a WASM file into an experimental cage |
| `warder unwrap <cage>` | Extract a cage |
| `warder doctor` | Check project structure and tools |

The table above is the complete v0.0.1 subcommand surface.

Every subcommand accepts `--project <dir>`, so scripts can select a project
without changing the caller's working directory.

## Projects

Create a new project:

```bash
warder new my-project
cd my-project
```

This creates:

```text
my-project/
├── warder.toml
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

`warder init` writes the root `warder.toml`, creates `src/` and `tests/` when
needed, and adds a starter `src/main.rl` only when there is no existing source
entry point.

## Repository Configuration

`warder.toml` is the only authored Warder configuration in a managed
repository. `warder new` and `warder init` generate this minimal form:

```toml
manifest-version = 1

[package]
name = "my-project"
version = "0.1.0"
```

The project conventions supply `src/main.rl` as the entry, edition `2025`,
and the default build settings (`wasm32`, optimization enabled, and `dist/`).
Empty dependencies are omitted. Write `package.entry`, `package.edition`, or a
`build` table only when overriding those conventions; `description` and
`authors` remain optional metadata.

Adding a local dependency creates a separate table when it is needed:

```toml
[dependencies]
local_utils = { path = "../local-utils" }
```

The current implementation has one publishable root package. The
`dependencies` table selects direct local source packages centrally; each
needs only `src/lib.rl` and has no leaf manifest or lock. Registry, Git,
foreign-Wasm, and transitive resolution fail closed. Future multi-publish
workspace support will extend the same root file, and public identity will
remain independent of directory placement.

Dependency aliases `std` and `project` are reserved. During test compilation,
`project` maps to the directory containing `package.entry` (`src/` by default).

An explicit `build` table controls the target name, whether optimization is
requested, and the output directory when the defaults are overridden.

## Dependencies

Add a local dependency:

```bash
warder add local_utils --path ../local-utils
```

Remove a dependency:

```bash
warder remove local_utils
```

The dependency directory needs `src/lib.rl`; it has no Warder leaf file.
`warder build` generates one root `warder.lock` for nondeterministic external
resolution. Direct local source packages are not lock entries, so the current
local-only lock has no resolved package entries. Commit `warder.lock` when
Warder generates it; do not edit it by hand.

## Build Outputs

Build the project:

```bash
warder build
```

For a package named `my-project` at version `0.1.0` without `[component]`, the
default outputs are:

```text
dist/my-project-0.1.0.wat
dist/my-project-0.1.0.wasm
dist/my-project-0.1.0.rgc
```

The output stem is always `<name>-<version>`. Change the output directory with
the root configuration's `build.output` field.

The build target is read from `warder.toml`, not from a command-line flag.

`warder build --watch` performs an initial build and then rebuilds for changes
to `warder.toml`, project `.rl` files, and `.rl` files in direct local source
dependencies. It is a full rebuild watcher, not an incremental compiler.

### Component build

A Component project requires a schema-v1 or schema-v2 `[component]` selection
in the root `warder.toml`. Ordinary `pub` and `export` declarations are not
published implicitly. For example, schema v1 selects a public root function:

```toml
[component]
schema = 1
package = "acme:my-project@0.1.0"
world = "my-project"
abi = "canonical-v1-sync"

[[component.exports]]
interface = "api"
source = "run"
name = "run"
```

With `[component]` present, plain `warder build` publishes the Component set.
`--component` asserts that choice and `--core` overrides it:

```bash
warder build
warder build --component
warder build --core
```

Warder validates and transactionally publishes this set together with the lock
update:

```text
dist/my-project-0.1.0.component.wasm
dist/my-project-0.1.0.component.wit
dist/my-project-0.1.0.component.toml
```

The metadata separates the semantic interface hash from exact Component and
WIT content hashes. A failed rebuild preserves the previous complete set.
Schema v2 adds root-only resources, value-only capture once-callables, and
explicit generic function instances.

The emitted `.component.toml` is artifact metadata, not project
configuration. A future edition may distinguish package-visible `pub` from
Component-intent `export`; today the source-public plus `[component]` double
gate remains required.

## Run And Test

Run the built program through an installed WASM runtime:

```bash
warder run
warder run -- arg1 arg2
```

`warder run` builds first, then looks for the generated
`dist/<name>-<version>.wasm`. It currently forces the Core profile when the
project declares `[component]`.

Run tests:

```bash
warder test
warder test bonus
```

Warder compiles and executes source-public, synchronous, non-generic,
zero-argument functions whose names begin with `test_` or end with `_test` and
whose explicit result type is `Boolean`. `true` passes; `false`, a trap, a
compile error, or a missing export fails. The optional filter matches function
names. This is a convention over ordinary functions, not a dedicated test
declaration syntax. Tests can import application modules through the reserved
namespace, for example `import project.main.{calculate_bonus}`.

## Publish Preflight

```bash
warder publish
warder publish --registry https://example.com/registry
```

For v0.0.1, `warder publish` performs a release-style preflight build and
metadata validation. Registry upload, authentication, and signing remain
experimental and no package is uploaded. The current preflight validates the
Core Cage profile.

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

Foreign Wasm wrapping is experimental in v0.0.1. `wrap --wit` stores the
caller-supplied WIT text without parsing, type-checking, matching it to the Core
module, or componentizing the module. It is separate from
`warder build --component`, which generates and validates the Component
artifact set from a Restrict package selection.

## Doctor

Check the current project:

```bash
warder doctor
```

`warder doctor` validates the root `warder.toml`, checks that the entry source
exists, loads `warder.lock` when present, checks for a Restrict compiler, and
reports whether a WASM runtime such as Wasmtime or Wasmer is available.
