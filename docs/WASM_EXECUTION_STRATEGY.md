# WebAssembly Execution Strategy

**Status**: Accepted direction
**Decision date**: 2026-08-07
**Release-scope amendment**: 2026-08-09
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

The current compiler emits Core WebAssembly text or validated binary Wasm.
Raw output preserves the complete lowered module. Explicit `--release` output
applies deterministic reachability-based dead-code elimination to functions,
function imports, named types, globals, tables, and element segments. Warder
can package WAT and binary Core Wasm artifacts in a cage; that is not Component
Model packaging. Program code imports the WASI Preview 1 `fd_write` and
`proc_exit` functions, and a zero-argument `main` receives an exported `_start`
wrapper. This is enough for the current output path and small command-style
programs, but it is not a complete WASI application platform.

The browser playground supplies the required program imports through a small
JavaScript WASI bridge. The bridge is necessary because browsers do not
currently expose WASI or the DOM directly to Core Wasm modules.

The stable default v0.0.1 host ABI remains scalar-only, and the experimental
`flat-record-v1` profile remains outside that compatibility boundary. The
ordinary path is still `.rl -> WAT -> Core Wasm`.

The v0.1.0-rc.1 candidate additionally implements the explicit B1 Component
path. Schema-v1 and schema-v2 selections in the repository-root `warder.toml`
become checked `ComponentInterfaceIr`, normalized WIT and stable ABI metadata, Canonical ABI
adapters, and validated Component bytes.
Warder `build --component` transactionally publishes
`<name>-<version>.component.wasm`, `.component.wit`, and `.component.toml`.
The rich external-WIT guest-import path is exercised by an independent Rust
producer, and scalar external implementation/export conformance is checked.
Rich external implementation/export and external-WIT resources still fail
closed. `warder wrap --wit` remains unverified WIT attachment to a Core module
cage and is not Component generation.

## Accepted Version Boundary

The next two product releases have an explicit interoperability split:

- **v0.1.0 / B1** is the synchronous Component Model implementation included
  in the v0.1.0-rc.1 candidate. It covers `String`, `List`, `Array`, records,
  `Option`, `Result`, user enums/variants, synchronous resources with scalar-signature
  factories and methods, value-capturing once-callable resources, and explicit
  concrete generic function instances. It generates and consumes WIT, emits
  Canonical ABI adapters and validated components, classifies compatibility,
  composes independently built components, and executes them through generated
  Rust bindings in a pinned runtime.
- **v0.2.0** adds language and Component Model async: async functions,
  `await`, structured `spawn`, futures, streams, cancellation, backpressure,
  and deterministic cleanup across suspension.

The repository is versioned `0.1.0-rc.1` for its first release candidate;
stable v0.1.0 has not been published. The complete B1 contract and intentional
limits are recorded in
`V010_SYNCHRONOUS_COMPONENT_ABI.md`.

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

The v0.1.0 interoperability path. WIT and the Component Model describe
strings, byte lists, general lists, fixed-length-validated arrays, records,
options, results, enums/variants, resources, callable handles, and concrete
generic specializations without exposing Restrict's arena layout. Streams,
futures, and async functions enter this profile in v0.2.0.

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
7. Interface authority follows API ownership, not import/export direction. A
   Restrict-owned API is selected by an explicit component export, normalized
   through `ComponentInterfaceIr` (Component Interface IR), and emitted as
   WIT; ordinary source-level `pub` is not an implicit Component ABI.
8. WASI, standard worlds, and third-party APIs are external-WIT-first whether
   Restrict consumes/imports them or implements/exports them. The same external
   WIT generates bindings or implementation obligations and is used to verify
   conformance.
9. The normalized WIT plus stable ABI-profile metadata is the final
   compatibility authority for both ownership paths. Generated adapters,
   bindings, packages, and runtime wiring are derived artifacts.

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

The current B0 memory slice makes item 5 observable without contaminating the
timed artifact. Every generated module records a machine-readable arena error
code, requested allocation size, and configured capacity before a deliberate
allocation trap. The benchmark runner compiles a separate
`--instrument-memory` artifact, verifies two identical host iterations each
finish with one arena reset and zero live bytes, and records peak live bytes and
allocation count. The ordinary `--release` artifact remains the runtime timing
subject. Nested-arena aggregate accounting remains outside the frozen B0
workload subset.

The completion gate is behavioral: a non-I/O Core Wasm workload must run
without JavaScript or unnecessary WASI imports; release output must
demonstrably exclude unused runtime code; workloads must not accidentally trap
at the former arena boundary; and repeated runs must verify identical results.
The detailed checklist is maintained in `ROADMAP.md`.

The regression benchmarks belong in the Restrict repository because they guard
compiler changes. A public cross-language harness should be created separately
after this milestone and must pin each toolchain, implement equivalent workload
semantics, retain every source and raw result, and report unfavorable results as
well as favorable ones. Timing comparisons are run locally on one machine in a
single session, with implementations interleaved to reduce changing host-load
and temperature bias. Absolute timings from different machines or sessions are
not combined. A dedicated or self-hosted benchmark runner is not required for
the current milestone.

WIT, the Component Model, composite host values, the full WASI API, async host
operations, browser DOM access, threads, and SIMD are not prerequisites for
Core Wasm compute benchmarks. They become required when a comparison claims to
measure application interoperability or a specific platform workload.

## v0.1.0 Synchronous Component Project Boundary

After the benchmark-ready milestone, B1 establishes Restrict as a synchronous
Component producer and consumer. The repository interoperability gate is
Rust-only: it deliberately does not require JavaScript/TypeScript bindings, a
Worker loader, HTML, CSS, npm, or a frontend framework. Those are later host-
adapter deliverables and do not expand the language backend.

The v0.1.0 boundary is a complete synchronous Component contract rather than a
temporary browser-only ABI:

- recursive lifting and lowering for `String`, `List`, `Array`,
  records, `Option`, `Result`, and user enums/variants;
- WIT resources with affine `own`, call-scoped `borrow`, exactly-once drop,
  scalar-signature factories and methods, and trap-safe cleanup;
- value-only capture closures exposed as signature-specific once-callable
  resources, never raw function-table indexes; resource/callable-bearing
  captures and `many` multiplicity fail closed;
- generic functions exposed only as explicitly named concrete
  monomorphizations, because open type parameters are not a WIT boundary;
- generated and validated WIT, Component Wasm, stable interface and content
  metadata, compatibility diffing, and Rust bindings; and
- synchronous WIT capability imports exercised in a pinned Component host,
  while full WASI 0.2 command-interface coverage remains a separate host
  profile rather than a v0.1.0 ABI release blocker.

WIT generation alone does not complete this boundary. Component packaging and
validation, Canonical ABI lifting/lowering, language bindings,
component composition, resource lifecycle, compatibility diffing, and runtime
execution are one interoperability stack.

Component Interface IR sits after type checking and monomorphization and before
Core Wasm adapters. It is the single recursive type graph used to generate WIT,
Canonical ABI lift/lower code, host bindings, metadata, compatibility hashes,
and diagnostics. Internal arena layouts remain independently evolvable.

The implemented Restrict-owned selection surface is rooted in the repository's
single `warder.toml`. Schema v1 uses a complete
`component.package = "namespace:name@semver"`, a selected `world`,
`abi = "canonical-v1-sync"`, and explicit interface/source/external-name
entries. A declaration must be source-public under the existing `pub` /
`export` surface and separately selected by `[component]`. Neither source
modifier alone creates a Component ABI, and their current Core behavior does
not change. Warder implements structural validation and translation to a
compiler-neutral selection, and the compiler enforces the double gate. Schema
v2 adds resources, once-callables, and explicit generic function instances.
Both schemas reach Component IR, adapters, validation, Warder publication, and
Rust runtime E2E. Schema v2 stays root-module-only; durable cross-module
declaration identity remains follow-on work. Detailed diagnostics, hash
separation, and limitations are recorded in the
[B1 Component Implementation Design](B1_COMPONENT_IMPLEMENTATION_DESIGN.md).

Warder uses one authored `warder.toml` and one generated `warder.lock` at the
repository root. The current project model contains one publishable root
package and direct local source packages selected from `[dependencies]`.
Those local packages need only `src/lib.rl`; they have no leaf configuration
or lock, and their deterministic source graph is not duplicated into
`warder.lock`. The currently unsupported registry, Git, foreign-Wasm, and
transitive resolvers fail closed, so a local-only lock has no resolved package
entries. A future multi-publish workspace graph must extend this same root file
without making directory placement a public identity or introducing leaf
manifests.

A future edition may give `export` Component-publication intent distinct from
package-visible `pub` and infer ordinary concrete Component exports from typed
source. That source-first surface is not implemented by B1: the explicit
source-public plus `[component]` double gate remains the current contract.

WASI, standard worlds, third-party components, and other externally owned APIs
supply WIT first. This is true both when Restrict consumes their imports and
when Restrict implements and exports their contracts. For explicitly selected
Restrict-owned exports, the compiler generates WIT from Component Interface IR.
The current implementation covers rich guest imports and scalar external
implementation/export conformance. Rich external implementations and
external-WIT resources fail closed pending a later adapter slice.
MoonBit and other WIT-first toolchains remain design references only. They are
not repository dependencies or interoperability gates. Restrict keeps the
hybrid ownership rule to avoid duplicating an interface the compiler owns,
losing affine/capture facts, or weakening source diagnostics.

WIT alone cannot express Restrict open generics, first-class closures, affine
capture multiplicity, or a stable `Array<T, N>` length constraint. Explicit
concrete instantiation, callable resources, ownership/drop adapters, and list
length validation are therefore normative v0.1.0 adapter rules.

After the v0.2.0 async boundary, this enables a separate embeddable sandbox
showcase. Its GUI may use
an established JS/TS framework, HTML, CSS, and iframe APIs. Restrict should own
substantial session, execution-policy, capability, and result-processing logic,
while the fixed host adapter owns DOM and Worker mechanics. The Rust-compiled
compiler Wasm and each generated user-program Wasm remain separate modules with
separate trust and resource boundaries.

## Delivery Sequence

1. Complete the Core Wasm Benchmark Ready milestone: target separation, direct
   artifacts, a stable benchmark language subset, release optimization, defined
   memory behavior, and reproducible in-repository regression measurements.
2. Optionally establish a separate, auditable Rust, Grain, MoonBit, and Restrict
   comparison harness after the milestone gate passes. This is a non-blocking
   parallel track, not a prerequisite for B1 or v0.1.0.
3. The B1 / v0.1.0 implementation in the release candidate completes one
   synchronous interoperability gate: Component Interface IR, recursive rich-value
   lifting/lowering, resources, once-callable closures, concrete generic
   function instances, WIT generation and rich import consumption, validated
   Component packaging, compatibility diffing, and a pinned native Component
   conformance host. The Rust-only repository gate includes independently
   compiled
   Rust-produced Component -> Restrict consumer and independently compiled
   Restrict-produced Component -> Rust consumer calls. They connect only
   through the versioned WIT world and Canonical ABI, without a shared allocator
   or internal layout. MoonBit remains a design precedent and optional external
   comparison, not a required repository E2E toolchain. JS/TS bindings and a
   Worker loader remain optional host-adapter work rather than B1 exit gates.
4. Complete B2 / v0.2.0: async/await/spawn, state-machine lowering,
   future/stream adapters, cancellation, backpressure, async resource cleanup,
   and generic async WIT capability adapters. Complete network/HTTP host
   profiles remain independently scoped follow-on work.
5. Build the embeddable Restrict sandbox as a separate polyglot showcase
   repository with Restrict-owned domain logic and a JS/TS GUI host.
6. Generate thin Web and cloud adapters without introducing a JavaScript
   source backend.
7. Adopt direct browser host interfaces when they become standardized and
   sufficiently portable.

## Non-Goals

- A JavaScript code-generation backend solely because browsers or Workers use
  JavaScript host APIs.
- Bundling npm dependency resolution or a particular frontend framework into
  the compiler or Warder.
- Package/member leaf manifests or a second workspace configuration file;
  future workspace support extends the repository-root `warder.toml`.
- A Docker-specific target or ABI.
- DOM operations as intrinsic language syntax.
- Equating ordinary Core output with the explicit B1 Component path, or
  claiming full WASI 0.2, browser DOM, or unrestricted composite-host support.
- Treating WIT text output or WIT attachment to a Core module cage as complete
  Component Model interoperability.
- Async functions, futures, streams, cancellation, or async resource cleanup in
  v0.1.0; those form the v0.2.0 boundary.
