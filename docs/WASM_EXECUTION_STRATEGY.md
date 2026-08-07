# WebAssembly Execution Strategy

**Status**: Accepted direction
**Decision date**: 2026-08-07
**Scope**: Compiler targets, host integration, and application deployment

This document records product and architecture direction. It does not widen the
current language or host ABI release surface. `LANGUAGE_SPECIFICATION.md`
remains authoritative for implemented language behavior.

## Decision

WebAssembly is Restrict's sole code-generation target. Browser, edge, CLI,
server, and container environments are different hosts for the same Wasm
backend; they are not separate JavaScript or native language backends.

Restrict will separate three layers:

```text
Restrict source and semantics
        |
        v
Core WebAssembly code generation
        |
        v
Host profile and generated adapter
        |
        +-- native WASI runtime
        +-- WebAssembly Component host
        +-- browser host
        +-- cloud or edge host
```

A generated JavaScript entry point may be used where a platform currently
requires one. It is packaging and host glue, not a JavaScript backend: Restrict
source still compiles to WebAssembly and application logic remains in Wasm.

## Current Reality

The current compiler emits Core WebAssembly text. Warder can package both WAT
and binary Wasm. Program code imports the WASI Preview 1 `fd_write` and
`proc_exit` functions, and a zero-argument `main` receives an exported `_start`
wrapper. This is enough for the current output path and small command-style
programs, but it is not a complete WASI application platform.

The browser playground supplies the required program imports through a small
JavaScript WASI bridge. The bridge is necessary because browsers do not
currently expose WASI or the DOM directly to Core Wasm modules.

The stable v0.0.1 host ABI remains scalar-only. `String`, `List`, `Array`,
records, `Option`, `Result`, and user-defined enums do not yet have a stable
general host ABI. The experimental `flat-record-v1` profile does not generate
WIT and is not part of the v0.0.1 compatibility boundary.

## Target Host Profiles

The names below describe architectural destinations, not claims that every
profile is implemented today.

### Core Wasm

A host-neutral computation module with explicit imports and exports. Core
language semantics must not depend on JavaScript objects, DOM APIs, a particular
cloud vendor, or a container runtime.

### WASI Preview 1

The near-term executable path for command-style programs. Complete support
requires direct binary emission plus documented bindings for arguments,
standard streams, environment values, filesystem capabilities, clocks, and
randomness. Availability of an API must remain capability-based.

### WASI Component

The forward-looking interoperability path. WIT and the Component Model should
describe strings, lists, records, variants, results, resources, streams, and
other host-visible values without exposing Restrict's arena layout.

### Web and Edge Adapters

Browsers and platforms such as Cloudflare Workers may require a generated
JavaScript or platform-specific entry point. Restrict should generate or ship
that adapter while continuing to compile user logic only to Wasm. A future host
that exposes Web or DOM capabilities directly should be usable by replacing the
adapter, without adding a new language backend.

### OCI and Container Runtimes

Docker, containerd shims, and runwasi are packaging and execution options for a
WASI artifact. They must not define a Docker-specific Restrict ABI. The bundled
Docker Desktop Wasm feature is currently deprecated, while Docker Engine can
use separately installed containerd Wasm shims; the durable compiler contract
is therefore WASI or the Component Model rather than Docker itself.

## Platform Assumptions

- Native WASI runtimes can provide a JavaScript-free execution environment for
  CLI, batch, server, plugin, and edge workloads.
- Cloudflare Workers executes Wasm, but its documented general integration
  currently uses the V8 JavaScript runtime and platform bindings. Cloudflare
  support should therefore be an adapter, not evidence of a universal WASI ABI.
- The current WebAssembly Web API gives Wasm no direct access to the DOM or
  native browser UI. Direct browser host interfaces may evolve, but Restrict
  must not promise them before a standard and implementations exist.

Current platform references:

- [WASI releases and interfaces](https://wasi.dev/releases)
- [Cloudflare Workers WebAssembly](https://developers.cloudflare.com/workers/runtime-apis/webassembly/)
- [Docker alternative runtimes](https://docs.docker.com/engine/daemon/alternative-runtimes/)
- [WebAssembly Web API](https://webassembly.github.io/spec/web-api/)

## Design Consequences

1. Host operations are modeled as explicit imported capabilities, not as
   privileged syntax or hard-coded JavaScript globals.
2. Internal arena layouts remain compiler-owned. A host ABI or Canonical ABI
   adapter performs lifting and lowering for composite values.
3. Web, Cloudflare, Docker, and individual runtimes do not become compiler
   backends. They consume a Wasm module or component and optional generated
   adapter.
4. Resource-like host values such as files, sockets, HTTP bodies, streams, and
   handles should integrate with Restrict's affine and scoped resource model.
5. Async host interfaces must be designed together with resource cleanup and
   temporal escape rules rather than introduced as an environment-specific
   exception.
6. Each host profile requires conformance tests that compile the same Restrict
   semantics and vary only the host imports, exports, adapters, and packaging.

## Milestone: Core Wasm Benchmark Ready

Restrict should not publish performance comparisons with Rust, Grain, MoonBit,
or other Wasm languages until its own execution path is stable enough that the
measurement describes the language implementation rather than incidental debug
code, unused imports, or a fixed test-sized allocator.

This milestone requires:

1. Distinct `wasm-core` and `wasip1` profiles, with host-neutral workloads free
   of unused WASI imports.
2. Direct binary Wasm output, validation, and a pinned reference runtime.
3. A documented release optimization pipeline that removes unreachable code
   and unused runtime helpers, while reporting both raw and optimized sizes.
4. A frozen benchmark language subset whose scalar control flow, calls,
   recursion, records, collections, closures, and selected `map`/`filter`/`fold`
   paths pass semantic and release-Wasm execution checks.
5. Defined behavior beyond the current fixed 4 KiB arena, including explicit
   exhaustion handling, reset between iterations, and peak-memory measurement.
6. An in-repository regression suite covering compiler time, artifact size,
   cold instantiation, warm execution, and representative language features.
7. A deterministic correctness oracle for every workload and machine-readable
   raw output for every run.
8. Pinned toolchains and one documented command that reproduces the Restrict
   baseline on a clean machine.

The completion gate is behavioral: a non-I/O Core Wasm workload must run
without JavaScript or unnecessary WASI imports; release output must
demonstrably exclude unused runtime code; workloads must not accidentally trap
at the former arena boundary; and repeated runs must verify identical results.
The detailed checklist is maintained in `ROADMAP.md`.

The regression benchmarks belong in the Restrict repository because they guard
compiler changes. A public cross-language harness should be created separately
after this milestone and must pin each toolchain, implement equivalent workload
semantics, retain every source and raw result, and report unfavorable results as
well as favorable ones.

WIT, the Component Model, composite host values, the full WASI API, async host
operations, browser DOM access, threads, and SIMD are not prerequisites for
Core Wasm compute benchmarks. They become required when a comparison claims to
measure application interoperability or a specific platform workload.

## Polyglot Web Project Boundary

After the benchmark-ready milestone, Restrict should support being a first-class
package inside a conventional JS/TS web workspace. This does not require
Restrict to parse HTML or CSS, manage npm dependencies, implement a frontend
framework, or add a JavaScript code-generation backend.

Warder owns deterministic Restrict compilation, tests, direct Wasm artifacts,
and artifact metadata. The web toolchain owns HTML, CSS, bundling, DOM rendering,
iframe construction, and framework dependencies. A generated binding and thin
host adapter connect the two layers.

The first browser integration needs a deliberately narrow contract:

- lifting and lowering for `String`, byte arrays, and a structured
  success/error envelope;
- typed JS/TS imports and exports plus target, memory, capability, and compiler
  metadata;
- initialization, call, error, and disposal lifecycle hooks;
- a Worker-compatible loader independent of the selected UI framework; and
- source-mapped diagnostics suitable for editors and build tools.

This boundary enables a separate embeddable sandbox showcase. Its GUI may use
an established JS/TS framework, HTML, CSS, and iframe APIs. Restrict should own
substantial session, execution-policy, capability, and result-processing logic,
while the fixed host adapter owns DOM and Worker mechanics. The Rust-compiled
compiler Wasm and each generated user-program Wasm remain separate modules with
separate trust and resource boundaries.

## Delivery Sequence

1. Complete the Core Wasm Benchmark Ready milestone: target separation, direct
   artifacts, a stable benchmark language subset, release optimization, defined
   memory behavior, and reproducible in-repository regression measurements.
2. Establish a separate, auditable Rust, Grain, MoonBit, and Restrict comparison
   harness only after the milestone gate passes.
3. Complete Polyglot Web Project Ready: Warder subproject integration, stable
   artifacts, narrow browser ABI, typed JS/TS bindings, and a
   framework-neutral Worker loader.
4. Build the embeddable Restrict sandbox as a separate polyglot showcase
   repository with Restrict-owned domain logic and a JS/TS GUI host.
5. Make native WASI command execution reproducible and add a
   capability-oriented WASI standard library.
6. Extend stable lifting and lowering from the browser subset to lists, records,
   `Option`, and `Result`.
7. Generate WIT and Component Model adapters for the supported ABI surface.
8. Generate thin Web and cloud adapters without introducing a JavaScript
   source backend.
9. Adopt direct browser host interfaces when they become standardized and
   sufficiently portable.

## Non-Goals

- A JavaScript code-generation backend solely because browsers or Workers use
  JavaScript host APIs.
- Bundling npm dependency resolution or a particular frontend framework into
  the compiler or Warder.
- A Docker-specific target or ABI.
- DOM operations as intrinsic language syntax.
- Claims that current Restrict programs already have full WASI, Component
  Model, browser DOM, or rich composite-host interoperability.
