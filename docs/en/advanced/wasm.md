# WebAssembly Integration

Restrict lowers source programs to WebAssembly without depending on a garbage
collector. The ordinary v0.0.1 path provides executable programs and scalar
Core exports. The explicit B1 path additionally provides synchronous rich
Components without changing that default ABI.

## Build Outputs

The compiler emits WebAssembly text or validated binary output directly:

```bash
restrict_lang --target wasip1 --emit wat app.rl
restrict_lang --target wasip1 --emit wasm app.rl
restrict_lang --target wasm-core --emit wasm compute.rl
restrict_lang --target wasm-core --emit wasm --release compute.rl
```

`wasm-core` emits no imports and rejects host output. `wasip1` is the default
and supplies the current `fd_write`-based output surface. Arena capacity
defaults to 4096 bytes; allocation-heavy workloads can select a larger
multiple-of-four capacity with `--arena-bytes`.

The default output is deliberately raw and retains the complete generated
module for debugging. `--release` runs this deterministic pipeline:

1. lower the release-validated program to WAT using Checked IR for the current
   ABI authority;
2. root reachability at function exports and the start entry;
3. follow direct calls transitively, retaining table elements only when a
   reachable indirect call needs them;
4. remove unreachable functions and function imports, then unused named types,
   globals, tables, and element segments; and
5. encode and validate the selected WAT or binary Wasm output.

This pass is dead-code elimination, not an instruction optimizer. It does not
yet perform inlining, constant folding on production bodies, or invoke an
external `wasm-opt`. If Binaryen is evaluated later, its result will be an
additional downstream artifact; the raw and compiler-release artifacts remain
available so size and runtime effects are attributable.

When building through Warder, the default project output also includes a local
cage artifact. Warder requests the compiler release pass when
`build.optimize = true` in the repository-root `warder.toml` (the default) or when
`warder build --release` is used:

```text
dist/<package-name>-<package-version>.wat
dist/<package-name>-<package-version>.wasm
dist/<package-name>-<package-version>.rgc
```

With an explicit `[component]` selection:

```bash
warder build --component
```

publishes a transactional Component set:

```text
dist/<package-name>-<package-version>.component.wasm
dist/<package-name>-<package-version>.component.wit
dist/<package-name>-<package-version>.component.toml
```

The emitted `.component.toml` is artifact metadata, not another project
configuration file. The current Component path still requires a source-public
declaration plus explicit `[component]` selection in `warder.toml`; a future
`pub`/`export` source-first distinction is not implemented.

For the repository itself, the browser compiler is packaged with `wasm-pack`:

```bash
wasm-pack build --target web --out-dir web/pkg
```

The Pages assembly step copies that bundle to `/compiler/pkg/` beside the
mdBook output and blog.

## Program Entry

`main` is the source-level program entry point:

```restrict
fun main: () -> Int32 = {
    40 + 2
}
```

Generated WebAssembly keeps `$main` as a normal function with its source result
type. A no-result wrapper named `$__restrict_start` initializes the default
arena, calls `$main`, drops any returned value, resets the arena, and is exported
as `_start`.

That split matters:

- Restrict source keeps `main` type-correct.
- WASI-style program execution gets a conventional no-result `_start`.
- Tests and host wrappers can still reason about the real source result.

Use a separate scalar export when the host should call a function and observe a
return value:

```restrict
fun compute_score: () -> Int32 = {
    42
}

export fun exported_score: () -> Int32 = {
    () compute_score
}
```

## Host ABI Surface

The v0.0.1 host-visible ABI supports concrete scalar values:

| Restrict type | Host ABI |
| --- | --- |
| `Int32` | `i32` |
| `Int64` | `i64` |
| `Float64` | `f64` |
| `Boolean` | `i32` boolean |
| `Char` | `i32` code point |
| `()` | no result or parameter payload |

Public or exported functions should be monomorphic at the host boundary. A
function that exposes `String`, records, lists, `Option`, `Result`, or a generic
type parameter directly is rejected by v0.0.1 release-surface validation instead
of receiving an unstable ad hoc ABI.

Composite values are still valid inside Restrict programs:

```restrict
record Score {
    base: Int32,
    bonus: Int32
}

fun total: (score: Score) -> Int32 = {
    score.base + score.bonus
}

export fun exported_total: () -> Int32 = {
    val score = Score { base: 30, bonus: 12 }
    score |> total
}
```

The exported function is scalar even though the implementation uses a record.

## Versioned Interoperability Direction

The ordinary v0.0.1 compatibility boundary remains scalar-only. The
v0.1.0-rc.1 candidate implements a synchronous Component ABI
for `String`, `List`, `Array`, records, `Option`, `Result`, user enums/variants,
scalar-signature resources, value-capturing once-callables, and explicit
concrete generic function instances. It generates or consumes WIT, performs
Canonical ABI lift/lower, validates and composes Components, classifies
compatibility, and executes through Rust bindings in a pinned runtime.

This is a release candidate; stable v0.1.0 has not been published.

- v0.2.0 adds the asynchronous boundary: async Component functions,
  `future`/`stream`, cancellation, and cleanup across suspension.

Neither version boundary makes Restrict's arena representation public. See the
[v0.1.0 Synchronous Component ABI scope](../../V010_SYNCHRONOUS_COMPONENT_ABI.md)
for the implemented contract and intentional limits.

The ordinary artifact path remains `.rl -> WAT -> Core Wasm`. With an explicit
schema-v1/schema-v2 selection, `warder build --component` transactionally
publishes validated `.component.wasm`, normalized `.component.wit`, and
`.component.toml` metadata. `wrap --wit` only attaches unverified
caller-supplied WIT to a Core module cage and is not Component generation.

Interface authority follows API ownership. WASI, standard worlds, and
third-party contracts are external-WIT-first whether Restrict consumes/imports
or implements/exports them. Rich guest imports and scalar external
implementation/export are implemented; rich external implementations and
external-WIT resources fail closed. Explicit Restrict-owned Component exports
are checked through `ComponentInterfaceIr` and generate normalized WIT; ordinary
`pub` declarations are not implicit host exports. WIT output alone is not
completion. The repository E2E gate covers
Rust-produced Component -> Restrict consumer and
Restrict-produced Component -> Rust consumer through WIT only. MoonBit remains
a reference and optional external comparison, not a repository gate.

## Memory Model

Restrict uses arena-oriented lowering for heap-backed values. The generated
start wrapper initializes the default arena before calling `main` and resets it
after the call. That gives examples a deterministic lifetime for program-local
allocations without a WebAssembly GC dependency.

Arena exhaustion remains a deliberate, non-recoverable Wasm trap. Before the
trap, the module records a machine-readable cause in these compiler-owned
exports:

| Export | Meaning |
| --- | --- |
| `__restrict_arena_error_code` | `0` for none, `1` for exhaustion, `2` for allocation without an active arena |
| `__restrict_arena_error_requested_bytes` | size of the allocation that failed |
| `__restrict_arena_capacity_bytes` | capacity selected by `--arena-bytes` |

A host may inspect the globals after a trap to classify the failure. It must
not treat them as a source-level `Result` or resume the trapped call.

For benchmark diagnostics, `--instrument-memory` emits a separate instrumented
module with `__restrict_arena_peak_bytes`, `__restrict_arena_live_bytes`,
`__restrict_arena_allocation_count`, `__restrict_arena_reset_count`, and the
`__restrict_memory_metrics_reset` function. Peak and live counts exclude the
arena header. The option adds bookkeeping, so performance measurements should
time the ordinary `--release` artifact and use the instrumented artifact only
for memory observations. These counters currently cover the B0 single-entry
arena path, not aggregate nested-arena usage.

Treat the memory layout as compiler-owned implementation detail. Core hosts
must not reach into record, string, list, `Option`, or `Result`
representations. Use scalar Core wrappers or the explicit Component path.

## Browser Compiler

The Pages site hosts the online compiler under `/compiler/`. The compiler is a
`wasm-pack` web bundle backed by the same Rust crate:

```text
site/dist/
├── docs/       mdBook output
├── compiler/   browser compiler UI and wasm-pack bundle
└── blog/       implementation notes
```

The compiler page accepts a `?code=` query parameter, so docs and blog posts can
open a source example directly in the browser. The mdBook theme adds "Try in
Playground" buttons to complete `restrict` code blocks that contain `fun main`.

## Current Limits

These are intentional v0.0.1 boundaries, not accidental omissions:

- exported generic functions are not host-visible
- exported composite values do not receive a direct host ABI
- current form dispatch is statically selected and monomorphized; there are no
  runtime form objects or dynamic dispatch ABI
- forms and concrete record adoptions are source-module features and do not add
  a composite host ABI
- Temporal Affine Types are outside the default release gate
- WebAssembly Component Model and WIT are available only through the explicit
  B1 path; they do not widen ordinary v0.0.1 Core exports

See the [v0.0.1 Release Surface](../reference/release-surface.md) for the
normative release-facing table.
