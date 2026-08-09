# Changelog

All notable changes to the Restrict Language compiler are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

- No changes yet.

## [0.1.0-rc.1] - 2026-08-09

### Component interoperability

- Add the explicit synchronous WebAssembly Component path from checked
  `ComponentInterfaceIr` through normalized WIT, Canonical ABI adapters,
  validated packaging, compatibility diffing, composition, and runtime
  execution.
- Support recursive `String`, `List`, fixed-length-validated `Array`, records,
  `Option`, `Result`, and user enum/variant values at the Component boundary.
- Add affine resources with `own`, call-scoped `borrow`, and exactly-once drop;
  value-capturing once-callable resources; and explicitly selected concrete
  generic function instances.
- Verify independent Rust-produced Component to Restrict consumer and
  Restrict-produced Component to Rust consumer calls through WIT and the
  Canonical ABI without sharing allocator or internal layout.

### Warder project model

- Use one authored repository-root `warder.toml` and one generated
  repository-root `warder.lock`, with no package or dependency leaf manifests.
- Build schema-v1 and schema-v2 Component selections transactionally into
  `.component.wasm`, `.component.wit`, and `.component.toml` artifact sets.

### Release candidate boundaries

- Restore the browser compiler's `wasm32-unknown-unknown` build and check that
  target in pull-request CI before documentation deployment.
- Add a reviewed v0.1.0-rc.1 deterministic Core Wasm benchmark baseline while
  retaining the v0.0.1 baseline as historical evidence.
- Keep the ordinary scalar Core Wasm compatibility profile from v0.0.1 and
  require explicit `[component]` selection for rich Component exports.
- Keep async functions, futures, streams, cancellation, backpressure, and
  cleanup across suspension in the v0.2.0 scope.
- Continue to fail closed for rich external-WIT implementation exports,
  external-WIT resources, rich resource signatures, `many` callables, and
  generic nominal boundary types.

## [0.0.1] - 2026-08-06

### Language

- OSV-only calls through `value |> function`, `(args) function`, and `() function`
- Typed `context` declarations and composable `with Context { bindings } { body }` lexical scopes
- Affine binding checks with `val` and `mut val`
- Bidirectional type inference for generics, lambdas, branches, records, and containers
- Records, prototype-style `clone` and `freeze`, and record destructuring
- Closed user-defined enums with qualified OSV construction and exhaustive matching
- Qualified `Option::Some`, `Option::None`, `Result::Ok`, and `Result::Err` value construction
- Pattern matching for user enums, `Option`, `Result`, `List`, and record values
- Method-only `form` contracts, concrete record `takes`, and `<T of A + B>` bounds with static monomorphized dispatch
- Compiler-provided and explicit record `Display` adoptions for polymorphic `display`, `print`, and `println`
- Source modules with dotted imports and scalar WebAssembly exports

### Compiler and runtime

- WebAssembly text and binary generation with arena-backed internal values
- Checked IR facts for inferred expression types, function signatures, layouts, and host ABI lowering
- Executable runtime coverage for compiler examples and browser samples
- An experimental opt-in `flat-record-v1` host adapter for concrete scalar-field records

### Tools and distribution

- Browser compilation and execution with token, AST, Wasm, output, and diagnostic views
- Language Server Protocol support over stdio
- Warder project creation, checking, building, running, testing, and diagnostics
- Deterministic direct local dependencies with source hashes, immutable staging snapshots, and recoverable artifact publication
- Cross-platform compiler and Warder archives with SHA-256 checksums

### Release boundaries

- The default host ABI is limited to concrete scalar parameters, results, and literal constants
- User enums and forms are source-level and statically lowered; they do not expose dynamic host objects
- Temporal affine types, WIT and Component Model output, dynamic form dispatch, generic or recursive enums, and postfix `?` remain future work
- Registry, Git, foreign-Wasm, and transitive package dependency graphs remain outside the direct-local package slice
- Warder registry publishing performs local validation only and uploads nothing
- Homebrew and VS Code Marketplace distribution are not part of v0.0.1

[Unreleased]: https://github.com/Ischca/restrict_lang/compare/v0.1.0-rc.1...HEAD
[0.1.0-rc.1]: https://github.com/Ischca/restrict_lang/compare/v0.0.1...v0.1.0-rc.1
[0.0.1]: https://github.com/Ischca/restrict_lang/releases/tag/v0.0.1
