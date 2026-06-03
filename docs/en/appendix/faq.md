# FAQ

## Is Restrict production-ready?

Not yet. The documentation targets a focused v0.0.1 release surface. It is
useful for compiler development, language experiments, WebAssembly examples,
and release-surface validation, but several major design areas remain reserved.

## Why does Restrict reject `function(args)`?

Restrict is OSV-only. Put the value or argument tuple first:

```text
value |> function
(left, right) combine
() make_default
```

This rule is strict so examples, diagnostics, and generated code all use the
same call shape.

## Why does `main` return get dropped during execution?

`main` keeps its source result type in generated WebAssembly, but program-style
execution uses a no-result `_start` wrapper. The wrapper initializes the default
arena, calls `$main`, drops any returned value, and resets the arena.

If a host needs a return value, expose a separate scalar `pub fun` or
`export fun`.

## Can I export strings or records to JavaScript?

Not directly in v0.0.1. Host-visible exports are limited to scalar monomorphic
values: `Int32`, `Int64`, `Float64`, `Boolean`, `Char`, and `()`.

Use records, strings, lists, `Option`, and `Result` internally, then export a
scalar wrapper while the composite host ABI is still being designed.

## Are Temporal Affine Types included?

No. Temporal Affine Types are planned and reserved, but they are outside the
default v0.0.1 gate.

## Are user-defined enums supported?

No. `enum` is reserved, and built-in `Option<T>` and `Result<T, E>` are
supported, but user-defined ADT declarations are not implemented in the v0.0.1
surface.

## What should documentation examples use?

Use:

```text
val value = 42
mut val counter = 0
value |> function
(left, right) combine
RecordName { field: value }
```

Avoid:

```text
let value = 42
val mut counter = 0
function(value)
RecordName { field = value }
```

## Where should I start?

Read the [Quick Start](../getting-started/quick-start.md), then the
[Language Guide](../guide/README.md). Use the
[Online Compiler](../tools/online-compiler.md) for short examples and the local
CLI/Warder flow for project builds.
