# Introduction

Restrict is a small, WebAssembly-oriented programming language built around
object-subject-verb calls, affine ownership, and explicit release boundaries.
The current documentation tracks the v0.0.1 surface: what is implemented and
intended for public use, what is intentionally rejected, and what remains
reserved design space.

## What Restrict Is Today

Restrict is a statically checked language that compiles source programs to
WebAssembly text and binary output. Its current stable-facing shape is compact:

- OSV-only calls such as `value |> function`, `(left, right) combine`, and
  `() start`
- immutable `val` bindings and mutable `mut val` bindings
- affine ownership for heap-backed values and function values
- built-in `List`, `Array`, `Option`, `Result`, and `Range<Int32>` support
- record declarations and record values inside Restrict programs
- scalar monomorphic WebAssembly exports for the host boundary

Some keywords and design documents describe future work. Temporal Affine Types,
source-level `form`/`takes` declarations, user-defined ADTs, and exported
generic or composite host ABIs are outside the default v0.0.1 gate.

## Why OSV

Restrict calls keep the data first and the operation last. This makes pipeline
shape visible without switching back to function-first syntax:

```restrict
fun add_bonus: (score: Int32, bonus: Int32) -> Int32 = {
    score + bonus
}

fun clamp_score: (score: Int32) -> Int32 = {
    score > 100 then {
        100
    } else {
        score
    }
}

fun main: () -> Int32 = {
    (96, 7) add_bonus |> clamp_score
}
```

Traditional calls such as `add_bonus(96, 7)` are not Restrict syntax. The
compiler keeps that rule strict so examples, diagnostics, and generated code all
use the same word order.

## Ownership Model

Restrict uses affine ownership: a binding may be used at most once unless the
value is copyable or the binding is explicitly mutable. This keeps ownership
visible in source code and makes deterministic WebAssembly lowering easier.

```restrict
fun choose_label: (passing: Boolean) -> String = {
    passing then {
        "ready"
    } else {
        "blocked"
    }
}

fun main: () -> String = {
    val passing = true
    passing |> choose_label
}
```

Copyable primitives such as `Int32`, `Int64`, `Float64`, `Boolean`, `Char`, and
`()` may be reused. Strings, lists, records, and function values follow the
affine path unless the compiler has an explicit copy rule for the operation.

## WebAssembly Boundary

Restrict programs can compute with composite values internally, but the
v0.0.1 host ABI is intentionally narrow. Public host-visible functions should be
monomorphic and scalar:

```restrict
export fun exported_score: () -> Int32 = {
    42
}
```

The executable source entry point is zero-argument `main`. During code
generation, `main: () -> ...` is called by a host `_start` wrapper. If `main`
returns a value, the wrapper drops it for program-style execution; expose
host-callable scalar results through a separate `pub fun` or `export fun`.

## Documentation Map

Start with the [Quick Start](./getting-started/quick-start.md), then read the
[Language Guide](./guide/README.md). The [Type Inference](./guide/type-inference.md)
page explains the current bidirectional inference surface, and the
[v0.0.1 Release Surface](./reference/release-surface.md) page records the public
boundary used by tests and examples.

The GitHub Pages site also hosts the online compiler at `../compiler/` from the
site root. Code blocks in this book that define `fun main` can be opened in that
browser compiler through the "Try in Playground" button.
