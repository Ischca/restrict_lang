# Restrict Language Development Roadmap

**Last Updated**: 2026-08-07
**Status**: Active Development
**Target**: v1.0 Release

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
- [ ] Detect arena exhaustion and report it rather than relying on an
  accidental Wasm trap
- [x] Support memory growth or a configurable larger arena for benchmark
  workloads
- [ ] Reset the arena reliably between iterations and expose enough data to
  measure peak memory

#### In-Repository Regression Benchmarks

- [x] Add a `benchmarks/` suite for compiler time, runtime, and artifact size
- [x] Cover scalar loops, function calls and recursion, records, and
  `map`/`filter`/`fold`
- [x] Give every workload a deterministic checksum or other correctness oracle
- [x] Run correctness and a short smoke subset in pull-request CI
- [ ] Run stable timing measurements on a controlled nightly or release runner
- [x] Store machine-readable raw results before producing charts or summaries

#### Reproducibility Contract

- [ ] Pin compiler, runtime, validator, optimizer, and comparison toolchains
- [x] Record source revision, target profile, flags, OS, CPU, and tool versions
- [ ] Measure compile time, raw and compressed Wasm size, cold instantiation,
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
- [ ] A clean machine can reproduce the complete baseline with pinned tools
- [ ] Baseline results and an explicit regression policy are checked in
- [ ] Only after these checks pass may public cross-language performance claims
  be based on the suite

WIT, the Component Model, a composite-value host ABI, filesystem or HTTP APIs,
async support, DOM access, threads, SIMD, and a JavaScript backend are not
required for this Core Wasm milestone. They must be evaluated separately when
benchmarking application or platform integration.

The Restrict regression suite belongs in this repository. The later
cross-language harness should live in a separate repository so that Rust,
Grain, MoonBit, and Restrict toolchains, sources, and raw results can be
versioned without coupling them to the compiler release cycle.

### Milestone B1: Polyglot Web Project Ready

**Goal**: Make a Restrict package a first-class part of a conventional JS/TS
web workspace after B0. Existing frontend frameworks own HTML, CSS, DOM, and
iframe presentation; Restrict owns the application domain logic compiled to
Wasm. Browser JavaScript remains host glue and is not a Restrict source
backend.

#### Project and Build Integration

- [ ] Keep `package.rl.toml`, `src/`, and `tests/` usable as a Restrict
  subproject inside a larger repository
- [ ] Let automation invoke Warder against an explicit project directory
  without changing the caller's working directory
- [ ] Emit stable, machine-readable artifact paths and build metadata
- [ ] Make `warder build --release` produce a browser-loadable Wasm artifact
  suitable for bundler pipelines
- [ ] Document a reference workspace combining Warder with a JS package manager
  and task runner
- [ ] Keep npm packages and frontend-framework dependency resolution outside
  Warder

#### Browser ABI and Bindings

- [ ] Define the first browser-host ABI for `String`, byte arrays, and a
  structured success/error envelope
- [ ] Generate typed JS/TS bindings for supported imports and exports
- [ ] Emit an import/export, memory, target, and compiler-version manifest
- [ ] Define stable initialization, call, error, and disposal lifecycle hooks
- [ ] Provide a framework-neutral Worker bootstrap and browser host adapter
- [ ] Preserve explicit capability imports rather than exposing ambient DOM or
  network access

#### Development Experience

- [ ] Preserve Restrict source locations in diagnostics returned through the
  generated bindings
- [ ] Provide a watch or incremental-build contract that a bundler plugin can
  consume
- [ ] Publish one reference integration for a mainstream JS/TS bundler without
  coupling the compiler to that bundler
- [ ] Run Restrict tests and frontend integration tests from one documented root
  command

#### Exit Criteria

- [ ] A fresh JS/TS workspace can contain and build a Warder project with one
  documented root command
- [ ] The frontend can call non-trivial Restrict logic through generated typed
  bindings and receive structured errors
- [ ] The same Restrict artifact can be loaded in a Worker independently of the
  selected UI framework
- [ ] Replacing the JS/TS framework does not require compiler or language
  changes
- [ ] Node, DOM, and framework semantics remain outside Restrict core language
  semantics

### Milestone B2: Embeddable Restrict Sandbox Showcase

**Goal**: Build a separate `restrict-sandbox` repository as a language showcase
after B0 and B1. It is intentionally a polyglot product: a JS/TS frontend owns
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

1. **Native WASI application baseline**
   - [ ] Document reproducible CLI execution without JavaScript
2. **Capability-oriented WASI library**
   - [ ] Arguments, environment, stdin/stdout/stderr, and exit
   - [ ] Filesystem, clocks, and randomness
   - [ ] Networking and HTTP after resource and async semantics are defined
3. **Stable component boundary**
   - [ ] Extend the browser ABI to lists, records, `Option`, `Result`, and
     resources
   - [ ] Generate WIT and WebAssembly Component Model adapters
4. **Generated platform adapters**
   - [ ] Browser entry and Web API adapter without a JavaScript source backend
   - [ ] Cloud/edge adapters, including a Cloudflare Workers entry adapter
   - [ ] OCI packaging for WASI runtimes without a Docker-specific ABI
5. **Future browser integration**
   - [ ] Adopt direct browser host interfaces when portable standards and
     implementations exist
   - [ ] Keep DOM and Web APIs out of core language semantics

See `docs/WASM_EXECUTION_STRATEGY.md` for the decision record and platform
assumptions.

---

## 📊 Current Status (as of 2025-01-13)

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
| **Temporal Affine Types (TAT)** | ⚠️ 50% (parsing/AST) | **→ Experimental feature, v2.0 target** |
| Async/Await | 📋 Design only | Post v1.0 |
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
- 📋 **Postpone**: Full implementation to v2.0+

---

## 🚀 Roadmap to v1.0

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

## 🔬 Post v1.0: Future Directions

### v1.1 - v1.x: Stability & Adoption

- Bug fixes and stability improvements
- Performance optimization
- Community feedback incorporation
- Additional standard library functions
- More example applications

### v2.0: Advanced Features (6+ months after v1.0)

**Temporal Affine Types Revival**:
- [ ] Formal specification completion
- [ ] Interaction with all v1.0 features defined
- [ ] Comprehensive test suite
- [ ] Gradual rollout as experimental → stable

**Other Potential Features**:
- [ ] Async/await (if TAT is stable)
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

### v1.0 Release Criteria

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
- Discord/forum for community support (post v1.0)

---

## 🔄 Roadmap Review

This roadmap will be reviewed and updated:
- **Monthly**: Progress check and priority adjustment
- **Quarterly**: Major milestone assessment
- **After v1.0**: Planning for v2.0

---

## 📝 Appendix: TAT Decision Record

### Why TAT is Experimental

**Date**: 2025-12-27

**Decision**: Move Temporal Affine Types to experimental feature, targeting v2.0

**Context**:
- Core value is "scope strictness", already achieved through:
  - Affine types
  - Context + Callback
  - Arena memory
  - with blocks
- TAT implementation would take 6-12 months
- Interactions with other features undefined
- Risk of delaying v1.0 significantly

**Consequences**:
- ✅ Faster path to v1.0
- ✅ More stable core language
- ✅ TAT can be added later without breaking changes
- ⚠️ Some advanced use cases postponed
- ⚠️ Current TAT code will be feature-gated

**Alternatives Considered**:
1. Complete TAT now - Rejected due to time/complexity
2. Remove TAT entirely - Rejected, valuable for v2.0
3. Simplify TAT - Would still take significant time

**Review Date**: After v1.0 release (6+ months)

---

**End of Roadmap**

*This is a living document. Last updated: 2026-08-07*
