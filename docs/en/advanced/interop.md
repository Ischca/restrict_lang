# Language Interoperability

Restrict interoperability is currently WebAssembly-centered. v0.0.1 keeps the
host boundary intentionally small so generated modules are predictable while
the generic and composite ABI design remains open.

## Calling Restrict From A Host

Expose host-callable functions with scalar monomorphic `pub fun` or
`export fun` declarations:

```restrict
fun adjust: (value: Int32) -> Int32 = {
    value + 1
}

export fun exported_adjust: (value: Int32) -> Int32 = {
    value |> adjust
}
```

The exported function can be called from a WebAssembly host using the normal
scalar ABI. Keep records, strings, lists, `Option`, and `Result` inside the
Restrict implementation or convert them to scalar summaries.

## Program Execution

For program-style execution, define `main`:

```restrict
fun main: () -> Int32 = {
    42
}
```

The generated module exports `_start` as a no-result wrapper around `$main`.
That wrapper initializes the default arena, calls `$main`, drops any returned
value, and resets the arena. If a host needs the result, add a separate scalar
export.

## Browser Compiler

The online compiler uses `wasm-pack` and `wasm-bindgen` to expose compiler entry
points to JavaScript:

```bash
wasm-pack build --target web --out-dir web/pkg
```

The web UI can compile a single source string and display WAT, tokens, AST, and
diagnostics. It is an interop example for the compiler itself, not a promise
that every Restrict program has a JavaScript ABI for composite values.

## Imports

Source imports are for Restrict modules:

```restrict
import examples.math.{double}
```

String import paths, aliases, re-exports, host module declarations, WIT
bindings, and Component Model adapter generation are outside v0.0.1.

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
    val reading = Reading { value: value, limit: limit }
    reading |> is_over_limit
}
```

This lets internal Restrict code use records while the host sees only scalar
parameters and results.
