# Language Guide Overview

Welcome to the Restrict Language guide. These pages describe the release-facing
v0.0.1 syntax and the design rules that shape it.

## What You'll Learn

- **[OSV Word Order](./osv-order.md)** - Calls put arguments before functions
- **[Syntax Reference](./syntax.md)** - Current declarations, expressions, and literals
- **[Type System](./types.md)** - Built-in types, generics, and affine ownership
- **[Ownership](./ownership.md)** - Memory management without GC
- **[Pattern Matching](./patterns.md)** - Exhaustive `match` expressions
- **[Warder Package Manager](./warder.md)** - Managing dependencies

## Quick Example

```restrict
record ReleaseSignal {
    score: Int32,
    passing: Boolean
}

fun summarize_release: (signal: ReleaseSignal) -> String = {
    signal match {
        ReleaseSignal { passing: true, score } => {
            "ready"
        }
        ReleaseSignal { passing: false, ..._ } => {
            "blocked"
        }
    }
}

fun main: () -> String = {
    val signal = ReleaseSignal { score: 92, passing: true }
    signal |> summarize_release
}
```

## Key Concepts

### OSV Syntax
Restrict uses Object-Subject-Verb call order. Arguments come first:
`value |> function`, `(left, right) function`, and `() function`.

### Affine Types
Heap-backed values such as strings, lists, records, and function values are
affine by default. Copyable primitives such as `Int32`, `Boolean`, `Float64`,
`Char`, and `()` can be reused.

### Type-Directed Impl Functions
`impl` blocks define functions selected by the receiver type, but calls remain
OSV. Use `(receiver, args...) function_name`, not object-style method calls.

### Pattern Matching
`match` is also OSV: the value being matched appears before the `match`
keyword, and each branch body is wrapped in braces.

### Zero GC
Restrict targets deterministic memory management through compile-time checks
and WASM-friendly data layouts.
