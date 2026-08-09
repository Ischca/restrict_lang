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

The current implementation has one publishable root package. Direct local
source packages are selected centrally from this root file; they do not have
leaf manifests or leaf locks. A future multi-publish workspace graph will
extend this same root `warder.toml` rather than add a workspace file or member
manifests. Public package and Component identities are explicit configuration,
not values derived from directory names.

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

An explicit `build` table controls the target name, whether optimization is
requested, and the output directory when the defaults are overridden.

## Dependencies

Add a local dependency:

```bash
warder add local_utils --path ../local-utils
```

The referenced local source package needs only its library root:

```text
local-utils/
└── src/
    ├── lib.rl
    └── numbers.rl
```

A minimal `src/lib.rl` might export a function:

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

The root configuration binding mounts the dependency's `src/` directory under
its alias:

| Source import | Dependency file |
|---------------|-----------------|
| `import local_utils.{score}` | `../local-utils/src/lib.rl` |
| `import local_utils` | `../local-utils/src/lib.rl` |
| `import local_utils.numbers.{double}` | `../local-utils/src/numbers.rl` |

An unqualified import inside a dependency stays package-local. For example,
`import numbers.{double}` inside `local-utils/src/lib.rl` resolves to that
package's `src/numbers.rl`, not an application module.

This is a configuration-to-compiler namespace binding. It does not add source-level
`import ... as` aliases or re-exports; both remain unsupported.

On a successful build, Warder writes one generated `warder.lock` beside the
root configuration. The lock records only dependency choices that cannot be
reconstructed deterministically from repository sources. Direct local source
packages are therefore not lock entries. Registry, Git, foreign WASM, and
transitive resolution currently fail closed, so a local-only lock has no
resolved package entries. Commit `warder.lock` when Warder generates it; do not
edit it by hand.

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
the root configuration's `build.output` field.

The build target is read from `warder.toml`, not from a command-line flag.

### Component build

An explicit Component build requires a schema-v1 or schema-v2 `[component]`
selection in the root `warder.toml`. Ordinary `pub` and `export` declarations
are not published implicitly. For example, schema v1 selects a public root
function:

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

Build it with:

```bash
warder build --component
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

The `.component.toml` file above is generated artifact metadata under `dist/`,
not another project configuration file. Source-first Component publication is
also future work: a later edition may distinguish package-visible `pub` from
Component-intent `export`, but the current implementation still requires the
source-public plus `[component]` double gate.

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
exists, and validates `warder.lock` when present. It also checks for a Restrict
compiler and reports whether a WASM runtime such as Wasmtime or Wasmer is
available.
