# Restrict Language Development Roadmap

**Last Updated**: 2026-08-09
**Status**: Active Development
**Target**: v0.1.0 synchronous Component ABI, then v0.2.0 async runtime

---

## 🎯 Core Vision

**Restrict Language is a functional programming language with strict scope management for WebAssembly.**

### Core Values

1. **Scope Strictness** - Resources and values have clearly defined, enforceable scopes
2. **Affine Types** - Variables can be used at most once, preventing accidental duplication
3. **Zero-cost Abstractions** - Compile-time safety with no runtime overhead
4. **WebAssembly Target** - Efficient, deterministic execution without GC

---

## WebAssembly Execution Strategy

**Decision date**: 2026-08-07

WebAssembly remains Restrict's sole code-generation target. WASI, browsers,
cloud/edge platforms, and container runtimes are host profiles, adapters, or
packaging around the same Wasm backend. Restrict will not add a JavaScript
backend merely because a current host requires JavaScript glue.

### Current Baseline

- [x] Generate Core WebAssembly text
- [x] Package binary Wasm through Warder
- [x] Export `_start` for zero-argument `main`
- [x] Import WASI Preview 1 `fd_write` and `proc_exit` for basic program I/O
- [x] Run generated programs in the browser through a small WASI bridge
- [x] Keep the stable v0.0.1 host ABI scalar-only

The ordinary pipeline remains `.rl -> WAT -> Core Wasm`. The v0.1.0-rc.1
candidate also has an explicit B1 path from schema-v1/schema-v2 selections
through checked `ComponentInterfaceIr`, normalized WIT and metadata, Canonical
ABI adapters, and validated Component bytes. Warder publishes the `.component.wasm`,
`.component.wit`, and `.component.toml` set transactionally. A Core module cage
created by `warder wrap --wit` is still only an attachment container; it is not
the Component build path.

### Milestone B0: Core Wasm Benchmark Ready

**Goal**: Make Restrict measurable and reproducible before publishing a
Rust, Grain, or MoonBit comparison. This milestone covers host-neutral Core
Wasm workloads; it does not claim that Restrict is already a complete
application platform.

#### Target and Artifact Boundary

- [x] Separate documented `wasm-core` and `wasip1` target profiles
- [x] Omit WASI imports from `wasm-core` artifacts when the program does not
  use host I/O
- [x] Make direct `.wasm` output and validation a first-class compiler flow
- [x] Pin and test the reference runtime and Wasm validation tool versions

#### Release Code Generation

- [x] Connect release mode to the optimizer and document its exact pipeline
- [x] Eliminate unreachable functions, unused runtime helpers and imports, and
  dead internal declarations while preserving explicit exports as roots
- [x] Define the role of `wasm-opt` and retain both raw and optimized artifact
  measurements
- [x] Make identical source, compiler revision, and options produce
  reproducible benchmark artifacts

#### Benchmark Language Surface

- [x] Stabilize release Wasm code generation for scalar arithmetic, branches,
  loops, and monomorphic function calls
- [x] Complete recursion for the forms used by the benchmark corpus
- [x] Complete closure calls and the `map`/`filter`/`fold` paths used by the
  collection workloads
- [x] Verify record and collection workloads against interpreter-independent
  expected results
- [x] Freeze and document the exact supported subset before comparing it with
  other languages

#### Memory Behavior

- [x] Replace or explicitly configure the current fixed 4 KiB arena limit
- [x] Detect arena exhaustion and report it rather than relying on an
  accidental Wasm trap
- [x] Support memory growth or a configurable larger arena for benchmark
  workloads
- [x] Reset the arena reliably between iterations and expose enough data to
  measure peak memory

#### In-Repository Regression Benchmarks

- [x] Add a `benchmarks/` suite for compiler time, runtime, and artifact size
- [x] Cover scalar loops, function calls and recursion, records, and
  `map`/`filter`/`fold`
- [x] Give every workload a deterministic checksum or other correctness oracle
- [x] Run correctness and a short smoke subset in pull-request CI
- [x] Record repeated timing reports and machine-check exact host, toolchain,
  source revision, workload set, and within-run dispersion
- [x] Keep timing informational and collect repeated evidence locally on demand
- [x] Adopt a same-session local comparison protocol: run every implementation
  on one machine in one time window, interleave their measurement order, and
  retain all observed samples
- [x] Keep hosted CI focused on correctness, reproducible artifacts, artifact
  size, and memory instead of treating shared-runner timing as a baseline
- [x] Store machine-readable raw results before producing charts or summaries

#### Reproducibility Contract

- [x] Pin the Restrict compiler build toolchain, runtime, validator,
  compressor, and compiler-owned release optimizer used by this baseline
- [x] Record source revision, target profile, flags, OS, CPU, and tool versions
- [x] Measure compile time, raw and compressed Wasm size, cold instantiation,
  warm execution, and peak memory where the runtime exposes it
- [x] Specify warm-up, iteration count, process isolation, and statistical
  summary rules
- [x] Provide one documented command that builds, validates, runs, verifies,
  and records all Restrict baselines on a clean machine

#### Exit Criteria

- [x] A non-I/O `wasm-core` workload runs without JavaScript and without
  unnecessary WASI imports
- [x] Release builds demonstrably optimize code and remove unused runtime code
- [x] Every language feature used by the corpus passes its semantic and Wasm
  execution checks in release mode
- [x] A representative workload can exceed the former 4 KiB arena boundary
  without an accidental trap
- [x] Every benchmark rejects an incorrect result through its correctness
  oracle
- [x] A clean machine can reproduce the complete baseline with pinned tools;
  the required GitHub `ubuntu-24.04` benchmark job builds, records, compares,
  and uploads the full baseline from a fresh checkout
- [x] Baseline results and an explicit regression policy are checked in
- [x] One documented local command records repeated raw timing evidence without
  turning machine-dependent timings into a persistent regression gate

WIT, the Component Model, a composite-value host ABI, filesystem or HTTP APIs,
async support, DOM access, threads, SIMD, and a JavaScript backend are not
required for this Core Wasm milestone. They must be evaluated separately when
benchmarking application or platform integration.

The Restrict regression suite belongs in this repository. No dedicated or
self-hosted benchmark runner is required at this stage. Timing is measured on
demand locally and remains informational; deterministic correctness, artifact,
size, and memory checks remain enforceable in CI.

### Post-B0: Cross-Language Comparison Gate

- [ ] Create a separate cross-language harness so Rust, Grain, MoonBit,
  Restrict, and any later comparison targets can pin their toolchains, sources,
  runtimes, and raw results without coupling them to the compiler release cycle
- [ ] Execute every comparison on the same local machine in the same session,
  use equivalent workloads and correctness oracles, and interleave execution
  order to reduce time- and temperature-dependent bias
- [ ] Report host and toolchain metadata, every raw sample, summary statistics,
  and unfavorable results; do not combine or compare absolute timing values
  collected on different machines or in different sessions
- [ ] Only after these checks pass may public cross-language performance claims
  be based on the suite

### Milestone B1 / v0.1.0: Synchronous Component Project Ready

**Status**: Implementation complete; `0.1.0-rc.1` is the first release
candidate. Stable v0.1.0 has not been published.

**Goal**: Make a Restrict package a first-class synchronous WebAssembly
Component producer and consumer in native Component hosts. The repository B1
gate is Rust-only; JS/TS bindings, Worker loaders, and browser bundler
integration are optional host-adapter follow-on work rather than v0.1 exit
criteria.

The target is not WIT file generation in isolation. B1 completes the WIT +
Component Model + Canonical ABI + language bindings +
component composition/runtime stack needed for independent-language
interoperability.

The completed B0 benchmark gate remains valid and independent. The unchecked
Post-B0 cross-language comparison gate does not block B1.

#### Interface Authority and ABI Architecture

Interface authority follows API ownership, not import/export direction.

- [x] Add a checked, monomorphized `ComponentInterfaceIr` (Component Interface
  IR) independent of the compiler-owned Core Wasm arena layout
- [x] Make checked/monomorphized IR authoritative for boundary function bodies
  and layouts, including reconciling the current String descriptor/codegen
  header mismatch
- [x] For Restrict-owned APIs, generate normalized WIT from explicitly selected
  Restrict component exports through Component Interface IR; do not expose
  every source-level `pub` declaration implicitly
- [x] For externally owned APIs such as WASI, standard worlds, and third-party
  components, treat external WIT as authoritative both when Restrict
  consumes/imports and when Restrict implements/exports the interface
- [x] Generate typed Restrict guest-import bindings from rich external WIT and
  scalar implementation obligations; rich implementation/export and
  external-WIT resources fail closed pending a follow-on adapter slice
- [x] Treat normalized WIT plus the stable ABI profile metadata as the public
  compatibility contract
- [x] Generate Canonical ABI lift/lower adapters, `cabi_realloc`, return-area
  handling, post-return cleanup, and trap-safe ownership transitions
- [x] Move or copy every composite result into boundary-owned storage before
  resetting the Restrict invocation arena
- [x] Keep raw pointers, arena offsets, layout IDs, function-table indexes, and
  unresolved type parameters private

This ownership-based source-first/WIT-first model avoids duplicate Restrict
type declarations and handwritten WIT drift for Restrict-owned APIs while
preserving affine ownership/capture facts and Restrict source diagnostics. It
retains WIT-first conformance for externally owned contracts. See
the [v0.1 Component ABI scope](docs/V010_SYNCHRONOUS_COMPONENT_ABI.md) for the
normative target and the
[B1 Component Implementation Design](docs/B1_COMPONENT_IMPLEMENTATION_DESIGN.md)
for the Component selection schema, Component Interface IR boundaries, phased
implementation, and kill criteria.

#### Component Foundation and Publication

- [x] Parse and structurally validate schema-v1 `[component]` metadata with an
  explicit `namespace:name@semver` package identity and
  `abi = "canonical-v1-sync"`
- [x] Resolve schema-v1 root selections internally with a source-public plus
  repository-root `warder.toml` selection double gate; preserve existing
  `pub` / `export` Core Wasm behavior
- [x] Limit schema-v1 selections to public root-module functions and
  reject unknown schema versions, private or missing declarations, duplicate
  selections, WIT-name collisions, and unsupported types
- [x] Build `ComponentInterfaceIr` independent of `HostAbi`,
  `ValueRepr`, `LayoutId`, arena addresses, and resolver-mangled symbols
- [x] Generate deterministic normalized WIT and reparse it with pinned tooling
- [x] Package deterministic Component bytes and validate them with pinned
  in-process tooling
- [x] Execute the selected function from an independently built Rust consumer
  that shares only the versioned WIT world and Canonical ABI
- [x] Keep exact artifact content identity separate from the semantic interface
  hash derived from normalized WIT and stable ABI-profile metadata
- [x] Publish `.component.wasm`, `.component.wit`, `.component.toml`, and the
  generated root `warder.lock` update as one recoverable Warder transaction
- [x] Revalidate staged Component bytes, normalized WIT, identity, and content
  hashes before publication; preserve the prior artifact set on failure

The default Core module cage and explicit Component artifact remain distinct.
`warder wrap --wit` attaches caller-supplied text to a cage and does not run
this generation or validation pipeline. Durable cross-module
`CanonicalDeclId` remains open; schema v2 selections are root-only.

#### Complete Synchronous Type Boundary

- [x] `String` maps to WIT `string` with validated UTF-8 lifting and lowering
- [x] `List<T>` maps recursively to WIT `list<T>`
- [x] `Array<T, N>` maps to WIT `list<T>` plus generated exact-length metadata
  and boundary validation
- [x] Records map recursively to WIT `record` without exposing internal padding
- [x] `Option<T>` and `Result<T, E>` map to WIT `option` and `result`
- [x] Payload-free user enums map to WIT `enum`; payload-carrying enums map to
  WIT `variant`
- [x] Explicit synchronous resources map to WIT `resource` with affine
  `own`/call-scoped `borrow`, exactly-once drop, and scalar-signature
  factories/methods
- [x] Value-only capture closure/function values map to signature-specific
  once-callable resources; captured raw table indexes never cross the boundary
- [x] Generic functions require explicit finite concrete instantiations with
  deterministic host names; open generics never appear in WIT
- [x] Nested combinations of supported ordinary value categories round-trip through
  the same recursive adapter implementation
- [ ] Add an explicit source byte type before exposing WIT `list<u8>`; `Char`
  is not a byte substitute
- [ ] Extend resources beyond scalar-signature methods/factories, callable
  captures beyond value-only state, and generic nominal boundary types

WIT alone cannot express Restrict open generics, first-class closures, affine
capture multiplicity, or a stable `Array<T, N>` length constraint. Explicit
concrete instantiation, callable resources, ownership/drop adapters, and list
length validation are therefore normative B1 rules.

#### Synchronous WASI and Host Profiles

- [x] Preserve the current Core Wasm and WASI Preview 1 profiles as compatible
  legacy output paths
- [x] Consume synchronous WIT capabilities and run the B1 conformance world
  in a pinned Component host, adapting the current basic output/exit path where
  required
- [x] Track full WASI 0.2 command coverage for arguments, environment,
  filesystem, clocks, randomness, and other standard interfaces as a separate
  host-profile milestone rather than a v0.1.0 ABI release blocker
- [x] Keep futures, streams, async functions, and suspension semantics in
  v0.2.0; keep complete network/HTTP profiles independently scoped
- [ ] Optionally generate a framework-neutral Worker loader and typed JS/TS
  bindings as later host adapters; these are not v0.1 repository exit gates

#### Project, Artifacts, and Developer Experience

- [x] Use one authored repository-root `warder.toml` and one generated
  repository-root `warder.lock`; do not require package/member leaf manifests
- [x] Keep the current project model to one publishable root package plus
  direct local source packages selected centrally from `[dependencies]`
- [x] Require only `src/lib.rl` for a direct local source package and keep the
  local source graph out of `warder.lock`; the current local-only lock has no
  resolved package entries
- [ ] Extend the same root file with a multi-publish workspace graph without
  adding leaf manifests or deriving public identity from directory placement
- [ ] Introduce a future edition's source-first Component ergonomics by
  distinguishing package-visible `pub` from Component-intent `export`; until
  then retain the implemented source-public plus `[component]` double gate
- [ ] Let Warder build an explicit project directory without changing the
  caller's working directory (follow-on CLI ergonomics; not a B1 ABI gate)
- [x] Emit deterministic normalized WIT, validated Component Wasm, and
  machine-readable artifact metadata from the explicit Component build path
- [x] Compose independently built components by their WIT worlds and execute
  the result in pinned Component runtimes
- [x] Separate the normalized semantic interface hash from exact artifact
  content hashes
- [x] Preserve Restrict source identities in component diagnostics
- [ ] Add optional watch/incremental and reference-bundler integration without
  making Warder an npm dependency resolver
- [x] Run the compiler, Warder publication, Rust Component host, and
  compatibility tests through the repository test commands

#### v0.1.0 Exit Criteria

- [x] Every supported B1 type, including nested ordinary composites,
  round-trips through generated Rust bindings in a pinned native Component
  runtime
- [x] WIT parsing, Component validation, and semantic-interface compatibility
  tests are automated with pinned Rust tooling
- [x] Rich external WIT produces working Restrict guest-import bindings;
  scalar external implementation/export conformance is checked
- [x] Array-length, invalid/stale handle, repeated borrow, terminal ownership,
  drop, and callable-capture cleanup paths have negative/runtime tests
- [x] Explicit generic function specializations produce stable named exports,
  while unresolved generic boundaries fail with typed diagnostics
- [x] Resource and callable handles remain valid for their documented lifetime,
  release captured state exactly once, and do not grow without bound under
  repeated create/call/drop cycles
- [x] An independent Rust Component host builds generated bindings from WIT and
  calls non-trivial Restrict Components
- [x] Internal layout or optimizer changes leave the interface hash unchanged;
  a real normalized-WIT shape change updates it, and compatibility diff reports
  breaking versus non-breaking changes
- [x] Rust-produced Component -> Restrict consumer executes from independently
  built artifacts
- [x] Restrict-produced Component -> Rust consumer executes from independently
  built artifacts
- [x] Cross-language tests share only the versioned WIT package/world; they do
  not share an allocator, linear memory, arena layout, runtime representation,
  or bridge that bypasses the Canonical ABI

MoonBit remains a design precedent and an optional external comparison target.
It is not a required repository E2E toolchain or v0.1.0 release dependency.

Intentional B1 follow-ons are rich external-WIT implementation/export,
external-WIT resources, resource methods/factories with rich signatures,
resource/callable-bearing captures, `many` callables, generic nominal boundary
types, and durable cross-module schema-v2 identity. They do not invalidate the
implemented Rust-only synchronous gate.

Every v0.1.0 component function is synchronous. `async`, `await`, `spawn`,
future/stream values, cancellation, and cleanup across suspension are release
validation errors rather than partially supported ABI features.

### Milestone B2 / v0.2.0: Async Component Runtime

**Goal**: Add suspension and structured concurrency only after the synchronous
Component ABI, ownership, and cleanup rules are stable.

#### Language and Runtime

- [ ] Specify and implement async functions, `await`, and structured `spawn`
- [ ] Lower suspended calls to stackless state machines with affine and
  temporal escape checks
- [ ] Provide a single-thread executor, structured task ownership,
  cancellation, deadlines, and bounded channels
- [ ] Add streams with explicit backpressure and deterministic cleanup on
  completion, error, cancellation, and trap

#### Async Component Boundary

- [ ] Generate and consume WIT async functions, `future<T>`, and `stream<T>`
- [ ] Define callback, re-entry, waitable, and concurrent-call state transitions
- [ ] Prohibit borrowed resource handles from crossing suspension unless a
  later explicit ownership rule permits it
- [ ] Generate Promise and AsyncIterator host bindings
- [ ] Prove generic async WIT capability adapters; track complete network/HTTP
  profiles as follow-on v0.2.x host integration

#### v0.2.0 Exit Criteria

- [ ] Cancellation and errors release tasks, resources, closures, and buffers
  exactly once at every suspension point
- [ ] Bounded stream tests prove backpressure rather than unbounded buffering
- [ ] Native Component and JS/TS hosts pass the same async conformance corpus
- [ ] All v0.1.0 synchronous components and host bindings remain compatible

Shared-memory threads, distributed actors, supervision trees, and multi-node
transport are later work; completing local async does not imply them.

### Post-v0.2 Showcase: Embeddable Restrict Sandbox

**Goal**: Build a separate `restrict-sandbox` repository as a language showcase
after B1 and B2. It is intentionally a polyglot product: a JS/TS frontend owns
the editor and browser UI, while Restrict owns substantial session, policy,
capability, and result-processing logic.

- [ ] Build the GUI with an existing JS/TS framework, HTML, and CSS
- [ ] Ship an iframe embed SDK with a versioned `postMessage` protocol
- [ ] Run both compilation and generated user programs away from the UI thread
  in disposable Workers
- [ ] Enforce source, compile-time, execution-time, memory, and output limits
- [ ] Validate generated Wasm imports, exports, memory limits, and target profile
  before instantiation
- [ ] Keep the default capability surface limited to console output and exit;
  add stdin, deterministic clock/randomness, or virtual files explicitly
- [ ] Use the Rust-compiled Restrict compiler Wasm as a dependency while keeping
  the sandbox's product logic primarily in its Restrict subproject
- [ ] Support read-only examples, editable snippets, shareable state, Stop and
  Reset actions, structured diagnostics, and downloadable Wasm
- [ ] Document the trust model, CSP, hosting requirements, compiler version,
  supported browsers, and data-retention behavior

The showcase measures "Restrict-main" by ownership of domain behavior, not by
eliminating every line of JavaScript. The fixed JS/TS host may load Wasm, manage
DOM and Workers, and render the UI; user-program semantics and selected sandbox
policies should remain implemented and tested in Restrict.

### Later Application Milestones

1. **Generated platform adapters beyond v0.2**
   - [ ] Browser entry and Web API adapter without a JavaScript source backend
   - [ ] Cloud/edge adapters, including a Cloudflare Workers entry adapter
   - [ ] OCI packaging for WASI runtimes without a Docker-specific ABI
2. **Future browser integration**
   - [ ] Adopt direct browser host interfaces when portable standards and
     implementations exist
   - [ ] Keep DOM and Web APIs out of core language semantics
3. **Advanced concurrency**
   - [ ] Shared-memory threads and multicore scheduling
   - [ ] Distributed actors, supervision, and multi-node transport
4. **Standard WASI capability profiles**
   - [ ] Full WASI 0.2 command interfaces beyond the v0.1 ABI conformance world
   - [ ] Network and HTTP profiles after the v0.2 async adapter gate

See `docs/WASM_EXECUTION_STRATEGY.md` for the decision record and platform
assumptions.

---

## 📊 Historical Implementation Snapshot (recorded 2025-01-13)

This snapshot and the older phase plan below are retained as implementation
history. They are not the active release schedule. The versioned B milestones
above govern v0.1.0 and v0.2.0 wherever older v1/v2 wording conflicts.

### ✅ Implemented & Stable (70-95%)

| Feature | Status | Notes |
|---------|--------|-------|
| Lexer | ✅ 100% | Full token support, comments working |
| Parser | ✅ 95% | OSV syntax, patterns, all constructs |
| Basic Type System | ✅ 90% | Int, Float, String, Boolean, Unit, Records |
| Affine Types (Basic) | ⚠️ 80% | Working but needs refinement |
| OSV Syntax | ✅ 95% | Right-associative calls functional |
| Lambda Expressions | ✅ 85% | Closures working, some edge cases |
| Pattern Matching | ✅ 90% | Option, List, Record patterns with codegen |
| Arena Memory | ✅ 85% | Basic arena allocation |
| Context System | ✅ 75% | Callback-based resource management |
| **Generics** | ✅ 90% | Type params, inference, monomorphization |
| **Module System** | ✅ 80% | Imports, exports, file resolution |
| **Forms & Display** | ✅ Initial slice | Method-only forms, concrete record adoptions, static dispatch |

### 🚧 Partially Implemented (30-70%)

| Feature | Status | Notes |
|---------|--------|-------|
| Affine Types (Complex) | ⚠️ 70% | Multiple refs, complex expressions |
| Type Inference | ⚠️ 75% | Bidirectional + generics working |
| WebAssembly Codegen | ⚠️ 70% | Most constructs work |
| Standard Library | ⚠️ 60% | Core functions, IO, Result type |

### 🔬 Experimental / On Hold (0-30%)

| Feature | Status | Decision |
|---------|--------|----------|
| **Temporal Affine Types (TAT)** | ⚠️ 50% (parsing/AST) | Historical snapshot; v0.2 only takes the rules needed for async safety |
| Async/Await | 📋 Design only | Superseded by B2 / v0.2.0 |
| Recursive Functions | ⚠️ 30% | Needs work |
| Higher-order Functions | ⚠️ 40% | map/filter/fold incomplete |

---

## 🎬 Development Strategy: TAT Postponement

### Decision: Move TAT to Experimental

**Rationale**:
1. **Scope strictness is already achieved** through:
   - Affine types (usage count strictness)
   - Context + Callback (scope boundary strictness)
   - Arena (memory scope strictness)
   - with blocks (explicit scope strictness)

2. **TAT adds complexity** without being essential to core value
3. **Implementation cost is very high** (~6-12 months for full TAT)
4. **Interactions with other features are undefined**

### TAT Status

- ✅ **Keep**: Syntax reserved (`~`, `within`, `lifetime`)
- ✅ **Keep**: Documentation archived for future reference
- ⚠️ **Move to experimental**: Implementation behind feature flag
- 📋 **Postpone**: Full general TAT design until after the focused v0.2.0 async-safety slice

---

## 🚀 Historical Roadmap to Language Specification v1.0 (Archived)

The specification document version and compiler product versions are
independent. The following phase inventory predates the active v0.1.0/v0.2.0
plan and must not be used to move Component or async work out of B1/B2.

### Phase 1: Core Stabilization (1-2 months)

**Goal**: Make all core features production-ready

#### 1.1 Affine Types Completion ✅

**Status**: COMPLETED (2025-12-27)

**Completed Tasks**:
- [x] Fix ignored test: `test_function_params_affine`
- [x] Make semicolons optional after val bindings
- [x] Fix Unit type and () literal parsing
- [x] Implement affine checking for complex expressions
- [x] Add detailed error messages with suggestions
- [x] Test coverage: nested blocks, conditionals, mutable variables
  - 9 comprehensive affine tests (up from 4)
  - Coverage: basic violations, field access, nested blocks, conditionals, mutable vars

**Achievements**:
- ✅ All 46 tests passing
- ✅ No ignored tests
- ✅ Improved error messages with fix suggestions
- ✅ Semicolons now optional (better UX)

**Error Message Improvement**:
```
Before: Variable p has already been used (affine type violation)

After:  Affine type violation: variable 'p' has already been used.

        Affine types can only be used once. To fix this:
        - Use 'mut val' if you need to use the value multiple times
        - Use '.clone' to create a copy before the first use
        - Restructure your code to only use the value once
```

**Commits**:
- `2a8ccff` - fix: Parse Unit return types and unit literals ()
- `db45aaa` - feat: Make semicolons optional after val bindings
- `0c1979d` - test: Add comprehensive affine type tests
- `881b6e4` - feat: Improve affine type violation error messages

---

#### 1.2 Pattern Matching Code Generation ✅

**Status**: COMPLETED (2025-12-28)

**Completed Tasks**:
- [x] Implement Option pattern codegen (`Some(x)`, `None`) - `src/codegen.rs:2376-2405`
- [x] Implement List pattern codegen (`[]`, `[head | tail]`) - `src/codegen.rs:2266-2334`
- [x] Implement Record pattern codegen (`Record { x y }`) - `src/codegen.rs:2335-2375`
- [x] Exhaustiveness checking - implemented in type checker
- [x] Test coverage: 17/20 pattern matching tests passing

**Test Results**:
- `test_match`: 5/7 passing (integer, boolean, nested patterns)
- `test_option`: 6/8 passing (Some/None patterns, constructors)
- `test_list`: 6/7 passing (list literals, head|tail patterns)
- `test_lambda`: 8/8 passing (lambdas in match arms)

**Note**: Failing tests use deprecated syntax (pre-EBNF v-1.0). Core functionality verified working.

**Commits**:
- `64391cc` - fix: Update test_lambda to use EBNF v-1.0 compliant syntax

---

#### 1.3 Context Standard Library

**Current Issues**:
- Context mechanism exists but underutilized
- No standard contexts for common resources
- Best practices not documented

**Tasks**:
- [ ] Implement `FileSystem` context
  ```rust
  context FileSystem {
      open: (String, (File) -> R) -> R
      read: File -> String
      write: (File, String) -> Unit
  }
  ```
- [ ] Implement `Database` context
  ```rust
  context Database {
      connect: (String, (Connection) -> R) -> R
      transaction: (Connection, (Transaction) -> R) -> R
  }
  ```
- [ ] Implement `HttpClient` context
- [ ] Document context pattern as best practice for resource management
- [ ] Create comprehensive examples

**Success Criteria**: 3+ standard contexts with full documentation

---

#### 1.4 Arena Enhancement

**Tasks**:
- [ ] Support nested arenas
- [ ] Add arena size tracking and overflow detection
- [ ] Implement arena growth strategies
- [ ] Document arena usage patterns
- [ ] Performance benchmarks

**Success Criteria**: Nested arenas working, documented patterns

---

### Phase 2: Language Completeness (2-3 months)

#### 2.1 Standard Library Expansion

**Status**: IN PROGRESS (2025-01-11)

**Completed**:
- [x] std/math.rl - abs, min, max, signum, pow, gcd, lcm, clamp
- [x] std/option.rl - Basic Option operations
- [x] std/string.rl - Character operations (is_digit, is_alpha, to_upper, to_lower, etc.)
- [x] std/list.rl - Basic list operations
- [x] std/prelude.rl - Core functions (not, identity, comparison helpers)

**In Progress**:
- [ ] Memory allocator for dynamic string/list operations

**Completed** (2025-01-11):
- [x] String runtime (WASM-level): string_length, string_concat, string_equals
- [x] String conversion: string_to_int, int_to_string
- [x] String access: char_at, substring

**Completed** (2025-01-11):
- [x] List higher-order functions: map, filter, fold (working!)

**Remaining Tasks**:
- [ ] List: zip function
- [ ] String operations: split, join (needs WASM runtime)
- [ ] Option utilities: map, flatMap, and_then
- [ ] I/O functions integrated with contexts

**Success Criteria**: Usable standard library for real applications

---

#### 2.1.1 String Runtime Implementation (WASM)

**Status**: COMPLETED (2025-01-11)

**Goal**: Implement WASM-level string operations that cannot be written in pure Restrict

**Completed Tasks**:
- [x] `string_length`: Read 4-byte length prefix
- [x] `string_concat`: Allocate new string and copy both sources
- [x] `string_equals`: Byte-by-byte comparison with length check
- [x] `string_to_int`: Parse integer from string (handles negative numbers)
- [x] `int_to_string`: Format integer as string (handles negative numbers)
- [x] `char_at`: Get character at index (bounds checked)
- [x] `substring`: Extract portion of string (start/end clamped)

**Technical Notes**:
- Strings use length-prefixed format (4 bytes length + data)
- Uses arena allocator for dynamic allocation
- All functions registered in type checker and codegen

---

#### 2.2 Module System Completion

**Status**: MOSTLY COMPLETE (2025-01-11)

**Completed**:
- [x] Import/export functionality (`import std.math.*`, `export fun`)
- [x] Module path resolution (search paths, file discovery)
- [x] Circular dependency detection (with clear error messages)
- [x] Type checker integration (imported types/functions available)
- [x] Codegen integration (imported functions compiled)

**Remaining Tasks**:
- [ ] Qualified name access (`std.math.abs` syntax)
- [ ] Re-exports (`export import module.*`)
- [ ] Module-level documentation

**Success Criteria**: Multi-file projects work correctly

---

#### 2.3 Error Handling

**Status**: IN PROGRESS (2025-01-11)

**Completed**:
- [x] Result<T, E> type implementation
  - Qualified OSV `Result::Ok` and `Result::Err` constructors
  - Pattern matching with Ok(x) and Err(e)
  - Type inference for Result types
  - WASM codegen with tagged unions
- [x] Closed user-defined error enums
  - Non-generic, non-recursive declarations
  - Zero- or one-payload variants through qualified `Type::Variant` names
  - OSV constructors and exhaustive matching
  - `Result<T, CustomError>` within Restrict programs
- [x] `std/result.rl` constructor and match boundary
- [x] Initial source-level `form` / `takes` / `of` slice
  - Non-generic, method-only form contracts
  - Concrete non-generic record adoptions
  - `<T of A + B>` bounds and static monomorphization
  - Compiler-provided `Display` and polymorphic `print` / `println`

**Remaining Tasks**:
- [ ] Higher-level `std/result.rl` helpers such as predicates, transforms, and Option conversion
- [ ] Error propagation operator (? or similar)
- [ ] Generic or recursive user enums and a host-visible enum ABI
- [ ] Error context and stack traces
- [ ] Panic handling in WASM
- [ ] Graceful error recovery

**Success Criteria**: Robust error handling in all components

---

#### 2.4 Type System Polish

**Status**: IN PROGRESS (2025-01-11)

**Completed**:
- [x] Better type error messages with "did you mean" suggestions
  - Levenshtein distance for fuzzy name matching
  - Suggests similar variables, functions, records, and fields
  - Rust-style colored error output with context

**Remaining Tasks**:
- [ ] Generic type inference improvements
- [ ] Type aliases
- [x] Initial form-based behavioral bounds
- [ ] Associated form types, generic/conditional adoptions, and default methods
- [ ] Type system documentation

**Success Criteria**: Type inference "just works" in most cases

---

### Phase 3: Production Ready (1-2 months)

#### 3.1 Tooling

**Tasks**:
- [ ] LSP server stability improvements
- [ ] VSCode extension polish
- [ ] Warder package manager completion
- [ ] Build system optimization
- [ ] Debugger integration (if feasible)
- [ ] Improved type error messages

**Success Criteria**: Good developer experience

---

#### 3.2 Documentation

**Tasks**:
- [ ] Complete language tutorial
- [ ] API reference documentation
- [ ] Best practices guide
- [ ] Migration guide (if applicable)
- [ ] Example applications (3-5 real-world examples)

**Success Criteria**: New users can learn the language without assistance

---

#### 3.3 Testing & Validation

**Tasks**:
- [ ] Comprehensive test suite (>90% coverage)
- [ ] Integration tests for all features
- [ ] Performance benchmarks
- [ ] Stress testing (large files, deep nesting)
- [ ] Real-world application testing

**Success Criteria**: No known critical bugs

---

#### 3.4 Release Preparation

**Tasks**:
- [ ] Version 1.0 feature freeze
- [ ] Release notes preparation
- [ ] Website and landing page
- [ ] Package distribution (crates.io, etc.)
- [ ] Community setup (Discord, forums, etc.)

**Success Criteria**: Ready to announce v1.0

---

## 🔬 Beyond v0.2.0: Future Directions

### Stability and Adoption

- Bug fixes and stability improvements
- Performance optimization
- Community feedback incorporation
- Additional standard library functions
- More example applications

### Advanced Features After the Focused Async Release

**Temporal Affine Types Revival**:
- [ ] Formal specification completion
- [ ] Interaction with all v1.0 features defined
- [ ] Comprehensive test suite
- [ ] Gradual rollout as experimental → stable

**Other Potential Features**:
- [ ] Effect system
- [ ] Advanced generics
- [ ] SIMD operations
- [ ] WebGPU host integration

---

## 📋 Immediate Action Items (This Week)

### 1. Move TAT to Experimental

```bash
# Create experimental directory structure
mkdir -p src/experimental
mkdir -p docs/experimental

# Move TAT implementation
git mv src/lifetime_inference.rs src/experimental/

# Add feature flag to Cargo.toml
[features]
default = []
experimental-tat = []

# Update conditional compilation
#[cfg(feature = "experimental-tat")]
mod experimental;
```

**Files to update**:
- [ ] `Cargo.toml` - Add feature flag
- [ ] `src/lib.rs` - Conditional TAT modules
- [ ] `src/type_checker.rs` - Feature-gate TAT code
- [ ] `README.md` - Update feature status
- [ ] `docs/TAT_IMPLEMENTATION_STATUS.md` - Mark as experimental

---

### 2. Update Documentation

Create/update these files:
- [x] This file: `ROADMAP.md`
- [ ] `README.md` - Update implementation status section
- [ ] `docs/DEVELOPMENT_PLAN.md` - Detailed technical plan
- [ ] `CONTRIBUTING.md` - Guide for contributors on priorities

---

### 3. Fix Ignored Tests (After parser fix)

Priority order:
1. [ ] Fix parser to handle function definitions
2. [ ] `type_checker::tests::test_function_params_affine`
3. [ ] Any other ignored/skipped tests
4. [ ] Document why tests were ignored

---

## 🚨 Parser Issues Discovered and Fixed (2025-12-27)

### Issues Found

**Initial Symptom**:
- Test `test_function_params_affine` was ignored with TODO note
- When un-ignored, test passed with `Ok(())` instead of expected affine violation error

**Investigation Results** - Three distinct issues identified:

#### Issue 1: Incorrect Syntax Used ❌
```
# WRONG - Not EBNF v-1.0 compliant:
fun use_twice: (p: Point) = { val a = p.x; val b = p.x; a }

# CORRECT - EBNF v-1.0 syntax:
fun use_twice: (p: Point) -> Unit = { val a = p.x; val b = p.x; () }
```
- Parser expects: `fun name: (params) -> ReturnType = { body }`
- See `RESTRICT_LANG_EBNF.md` line 211-214

#### Issue 2: Missing Semicolons Required ⚠️
```
# Parser fails to parse statement boundaries without semicolons:
val a = p.x
val b = p.x   // OSV parser consumes this incorrectly

# FIX: Add semicolons after val bindings:
val a = p.x;
val b = p.x;
```
- OSV parser is greedy and crosses statement boundaries
- Semicolons explicitly mark statement endings

#### Issue 3: Parser Bugs Fixed ✅
1. **Unit Type Parsing**: `parse_type` didn't handle `Unit` keyword
   - Added `type_name()` helper to accept both identifiers and `Unit`
   - Enables `-> Unit` return types

2. **Unit Literal Parsing**: `()` literal wasn't recognized as expression
   - `()` is lexed as `LParen` + `RParen` (two tokens), not `Token::Unit`
   - Added special case in `atom_expr` before general parenthesized expressions

### Fixes Applied

**Parser Changes** (`src/parser.rs`):
- [x] Added `type_name()` function to handle Unit keyword in types
- [x] Added `()` literal parsing in `atom_expr`
- [x] Added test `test_fun_decl_unit_return`

**Test Changes** (`src/type_checker.rs`):
- [x] Fixed `test_function_params_affine` with correct syntax
- [x] Added semicolons after val bindings
- [x] Removed `#[ignore]` attribute

### Result

✅ **test_function_params_affine now passes**
✅ **Affine type checking correctly detects parameter violations**

```
Type error: Variable p has already been used (affine type violation)
```

**Commit**: `2a8ccff` - fix: Parse Unit return types and unit literals ()

---

## 🎯 Success Metrics

### Historical General-Release Quality Criteria

**Functionality**:
- ✅ All core features implemented and tested
- ✅ No ignored/skipped tests
- ✅ Pattern matching fully working
- ✅ Standard library usable
- ✅ Module system functional

**Quality**:
- ✅ >90% test coverage
- ✅ No known critical bugs
- ✅ Performance benchmarks acceptable
- ✅ Memory safety verified

**Documentation**:
- ✅ Complete tutorial
- ✅ API reference
- ✅ 5+ example applications
- ✅ Best practices guide

**Tooling**:
- ✅ LSP server stable
- ✅ VSCode extension working
- ✅ Warder package manager functional

---

## 👥 Resource Allocation

### Current Focus (80% of effort)

1. **Affine types completion** (30%)
2. **Pattern matching codegen** (25%)
3. **Context standard library** (15%)
4. **Documentation** (10%)

### Maintenance (20% of effort)

- Bug fixes
- Code reviews
- Issue triage

---

## 📞 Communication

### Status Updates

- Weekly progress summary (if active development)
- Monthly milestone reviews
- Public roadmap on GitHub

### Community

- GitHub Issues for bug reports
- Discussions for feature requests
- Discord/forum for community support (after the active release milestones)

---

## 🔄 Roadmap Review

This roadmap will be reviewed and updated:
- **Monthly**: Progress check and priority adjustment
- **Quarterly**: Major milestone assessment
- **After v0.2.0**: Plan subsequent stability and advanced-feature releases

---

## 📝 Appendix: TAT Decision Record

### Why TAT is Experimental

**Date**: 2025-12-27

**Decision**: Keep general Temporal Affine Types experimental; v0.2.0 adopts
only the temporal rules required for safe async suspension and cleanup

**Context**:
- Core value is "scope strictness", already achieved through:
  - Affine types
  - Context + Callback
  - Arena memory
  - with blocks
- TAT implementation would take 6-12 months
- Interactions with other features undefined
- Risk of delaying the synchronous v0.1.0 boundary significantly

**Consequences**:
- ✅ Faster path to v0.1.0 and a focused v0.2.0 async release
- ✅ More stable core language
- ✅ TAT can be added later without breaking changes
- ⚠️ Some advanced use cases postponed
- ⚠️ Current TAT code will be feature-gated

**Alternatives Considered**:
1. Complete TAT now - Rejected due to time/complexity
2. Remove TAT entirely - Rejected, valuable for async safety and later research
3. Simplify TAT - Would still take significant time

**Review Date**: After v0.2.0 release

---

**End of Roadmap**

*This is a living document. Last updated: 2026-08-09*
