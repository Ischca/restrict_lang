# WebAssembly Integration

Restrict lowers source programs to WebAssembly without depending on a garbage
collector. The current v0.0.1 integration is deliberately small: it is strong
enough for executable examples, scalar host exports, and the browser compiler,
while leaving generic and composite host ABI decisions for later design work.

## One Backend, Multiple Hosts

WebAssembly is Restrict's sole code-generation target. A host profile selects
imports, exports, adapters, and packaging without changing Restrict source
semantics:

```text
Restrict source
    |
    v
Core WebAssembly
    |
    +-- native WASI runtime
    +-- Component Model host       (planned)
    +-- browser adapter
    +-- cloud or edge adapter      (planned)
```

Generated JavaScript may currently load a module or provide browser and cloud
APIs. That code is host glue, not a JavaScript backend. Restrict does not need
to compile source to JavaScript in order to run Wasm from JavaScript.

The current execution boundary is:

| Path | Status |
| --- | --- |
| Import-free `wasm-core` compute modules | Current |
| `wasip1` with basic WASI Preview 1 program imports | Current and default |
| Browser execution through the playground's JavaScript WASI bridge | Current |
| General WASI arguments, filesystem, clocks, randomness, networking, and HTTP | Future |
| WIT and WebAssembly Component Model output | Future |
| Generated browser and cloud platform adapters | Future |
| Direct portable DOM access from Wasm | Not currently standardized |

Native WASI runtimes can execute a Wasm application without JavaScript. The
browser case is different: the current WebAssembly Web API does not give a
module direct access to the DOM or native browser UI, so a host adapter remains
necessary. Restrict keeps that adapter separate so a future standardized host
interface can replace it without introducing a new language backend.

## Build Outputs

The native compiler emits text or validated binary WebAssembly directly:

```bash
restrict_lang --target wasip1 --emit wat app.rl
restrict_lang --target wasip1 --emit wasm app.rl
restrict_lang --target wasm-core --emit wasm compute.rl
```

`wasm-core` emits no imports and rejects host output. `wasip1` supplies the
current `fd_write`-based output surface. Arena capacity defaults to 4096 bytes;
larger allocation-heavy workloads can select an explicit capacity such as
`--arena-bytes 1048576`.

Warder emits both text and binary Wasm. A default Warder project build also
includes a local cage artifact:

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
    val score = Score { base: 30, bonus: 12 };
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

The playground also provides the generated program's WASI Preview 1 imports
from JavaScript so it can capture stdout and stderr. This is a browser host
adapter around a Wasm program, not a second Restrict code-generation backend.

## Deployment Environments

A WASI artifact should remain independent of its launcher and packaging:

- Wasmtime and other native WASI runtimes are the JavaScript-free executable
  path for CLI, batch, server, plugin, and edge programs.
- Docker, containerd shims, and runwasi are possible OCI packaging and execution
  layers. They do not define a Docker-specific Restrict ABI.
- Cloudflare Workers can execute Wasm, but its general documented integration
  currently uses V8 and platform bindings. Restrict should use a generated
  adapter instead of treating Cloudflare as a portable WASI contract.
- Browsers require a host adapter for DOM, events, Fetch, and other Web APIs
  until a portable direct Wasm interface is standardized and implemented.

Platform behavior evolves independently from the language. See the official
[WASI releases](https://wasi.dev/releases),
[Cloudflare Workers Wasm documentation](https://developers.cloudflare.com/workers/runtime-apis/webassembly/),
[Docker alternative runtime documentation](https://docs.docker.com/engine/daemon/alternative-runtimes/),
and [WebAssembly Web API](https://webassembly.github.io/spec/web-api/) for the
current host surfaces.

## Current Limits

These are intentional v0.0.1 boundaries, not accidental omissions:

- exported generic functions are not host-visible
- exported composite values do not receive a direct host ABI
- current form dispatch is statically selected and monomorphized; there are no
  runtime form objects or dynamic dispatch ABI
- forms and concrete record adoptions are source-module features and do not add
  a composite host ABI
- Temporal Affine Types are outside the default release gate
- WebAssembly Component Model and WIT integration are future interop work
- general WASI filesystem, network, HTTP, and async bindings are future work
- direct browser DOM access is not part of the current WebAssembly host surface

See the [v0.0.1 Release Surface](../reference/release-surface.md) for the
normative release-facing table.
