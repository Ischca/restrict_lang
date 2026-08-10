# Language Interoperability

Restrict interoperability is WebAssembly-centered. The ordinary v0.0.1 Core
boundary remains intentionally scalar. The explicit B1 path adds synchronous
rich Component producer and consumer interoperability without exposing the
compiler's arena layout.

The compiler's ordinary artifact path is `.rl -> WAT -> Core Wasm`. With a
schema-v1 or schema-v2 `[component]` selection in the repository-root
`warder.toml`, `warder build` (or the explicit `--component` assertion)
transactionally
publishes `.component.wit`, validated `.component.wasm`, and `.component.toml`
metadata. `wrap --wit` only attaches caller-supplied text to a Core module cage
and is not Component generation.

## Versioned Interoperability Direction

The ordinary v0.0.1 compatibility boundary remains scalar-only. Stable
v0.1.0 implements `String`, `List`, `Array`, records,
`Option`, `Result`, enums/variants, scalar-signature resources, value-capturing
once-callables, and concrete generic function instances. It generates and
consumes WIT, performs Canonical ABI lift/lower, validates and composes
Components, classifies compatibility, and executes through Rust bindings.

- v0.2.0 adds the asynchronous boundary: async component functions,
  `future`/`stream`, cancellation, and cleanup across suspension.

Neither version boundary makes Restrict's arena representation public. The
repository's v0.1 scope document records the implemented contract and its
intentional limits; this page summarizes the public boundary.

Interface authority follows API ownership. WASI, standard worlds, and
third-party contracts are external-WIT-first whether Restrict consumes/imports
or implements/exports them. Rich guest imports and scalar external
implementation/export are implemented; rich external implementations and
external-WIT resources fail closed. Explicit Restrict-owned component exports are
checked through `ComponentInterfaceIr` and generate normalized WIT; ordinary
`pub` declarations are not implicit host exports. Normalized WIT and stable ABI
metadata form the compatibility contract, but WIT output alone is not the B1
gate; adapters, validation, composition, lifecycle, diffing, and runtime calls
are included.

Repository E2E requires independently built
Rust-produced Component -> Restrict consumer and
Restrict-produced Component -> Rust consumer paths, connected only through WIT
and the Canonical ABI. MoonBit remains a design precedent and optional external
comparison, not a repository release toolchain.

JS/TS bindings and Worker loaders are optional host-adapter work, not B1 gates.
Full WASI 0.2 command/network/HTTP coverage and async/future/stream belong to
separate host-profile and v0.2 work.

## Host Profiles, Not Language Backends

Restrict source always compiles to WebAssembly. WASI, browser, cloud/edge, and
container environments provide different host imports and packaging around the
same backend. JavaScript emitted for a browser or Worker entry point is an
adapter; Restrict does not need a JavaScript code-generation backend.

Keep platform operations behind explicit imports and capabilities. A future
WIT or browser host interface should be able to replace today's adapter without
changing ordinary Restrict calls or the internal arena representation.

## Calling Restrict From A Host

For ordinary Core Wasm, expose host-callable functions with scalar monomorphic
`pub fun` or `export fun` declarations:

```restrict
fun adjust: (value: Int32) -> Int32 = {
    value + 1
}

export fun exported_adjust: (value: Int32) -> Int32 = {
    value |> adjust
}
```

The exported function can be called from a Core WebAssembly host using the
normal scalar ABI. Keep rich values internal or convert them to scalar
summaries on this path.

For rich interoperability, explicitly select the public function under
`[component]` in the root `warder.toml` and run `warder build` (or
`warder build --component` when the selection must be asserted).
Ordinary `pub` or `export` alone never publishes a Component ABI. A future
edition may distinguish package-visible `pub` from Component-intent `export`,
but that source-first surface is not implemented. Schema v2 is root-only and adds resources,
once-callables, and explicit generic function instances. The resulting WIT is
the only contract shared with an independent Rust consumer.

## Program Execution

For program-style execution, define `main`:

```restrict
fun main: () -> Int32 = {
    42
}
```

When `main` takes no parameters, the generated module exports `_start` as a
no-result wrapper around `$main`. That wrapper initializes the default arena,
calls `$main`, drops any returned value, and resets the arena. If a host needs
the result, add a separate scalar export.

## Browser Compiler

The online compiler uses `wasm-pack` and `wasm-bindgen` to expose compiler entry
points to JavaScript:

```bash
wasm-pack build --target web --out-dir web/pkg
```

The web UI can compile a single source string and display WAT, tokens, AST, and
diagnostics. It is an interop example for the compiler itself, not a promise
that every Restrict program has a JavaScript ABI for composite values.

The same distinction applies to other hosts. A Cloudflare Workers adapter may
translate request and response values to the platform runtime, while a native
WASI host may expose standard component interfaces. Neither path changes the
Restrict language backend.

## Imports

Source imports are for Restrict modules:

```restrict
import examples.math.{double}
```

String import paths, aliases, re-exports, and source-level host module
declarations are outside v0.0.1. External-WIT guest imports use the explicit B1
compiler path rather than this source-module syntax.

## Practical Boundary Pattern

Use scalar wrappers:

```restrict
record Reading {
    value: Int32,
    limit: Int32
}

fun is_over_limit: (reading: Reading) -> Boolean = {
    reading.value > reading.limit
}

export fun exported_over_limit: (value: Int32, limit: Int32) -> Boolean = {
    val reading = Reading { value: value, limit: limit };
    reading |> is_over_limit
}
```

This lets internal Restrict code use records while the host sees only scalar
parameters and results.
