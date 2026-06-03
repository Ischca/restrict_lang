# WebAssembly Integration

Restrict lowers source programs to WebAssembly without depending on a garbage
collector. The current v0.0.1 integration is deliberately small: it is strong
enough for executable examples, scalar host exports, and the browser compiler,
while leaving generic and composite host ABI decisions for later design work.

## Build Outputs

The compiler can emit WebAssembly text (`.wat`) and binary (`.wasm`) output.
When building through Warder, the default project output also includes a local
cage artifact:

```text
dist/<package-name>-<package-version>.wat
dist/<package-name>-<package-version>.wasm
dist/<package-name>-<package-version>.rgc
```

For the repository itself, the browser compiler is packaged with `wasm-pack`:

```bash
wasm-pack build --target web --out-dir web/pkg
```

The Pages assembly step copies that bundle to `/compiler/pkg/` beside the
mdBook output and blog.

## Program Entry

Zero-argument `main` is the source-level program entry point:

```restrict
fun main: () -> Int32 = {
    40 + 2
}
```

Generated WebAssembly keeps `$main` as a normal function with its source result
type. When `main` takes no parameters, a no-result wrapper named
`$__restrict_start` initializes the default arena, calls `$main`, drops any
returned value, resets the arena, and is exported as `_start`. A parameterized
function named `main` is still a normal function and does not emit `_start`.

That split matters:

- Restrict source keeps `main` type-correct.
- WASI-style program execution gets a conventional no-result `_start` for
  zero-argument `main`.
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

## Memory Model

Restrict uses arena-oriented lowering for heap-backed values. The generated
start wrapper initializes the default arena before calling `main` and resets it
after the call. That gives examples a deterministic lifetime for program-local
allocations without a WebAssembly GC dependency.

For v0.0.1, treat the memory layout as compiler-owned implementation detail.
Host code should not reach into record, string, list, `Option`, or `Result`
representations directly. Use scalar wrapper functions while the composite host
ABI is still being designed.

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
- source-level `form`/`takes` declarations are reserved for later
- Temporal Affine Types are outside the default release gate
- WebAssembly Component Model and WIT integration are future interop work

See the [v0.0.1 Release Surface](../reference/release-surface.md) for the
normative release-facing table.
