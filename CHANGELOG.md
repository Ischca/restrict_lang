# Changelog

All notable changes to the Restrict Language compiler are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [Unreleased]

### Added

- Add closed, non-generic, non-recursive user-defined enums. Variants carry
  zero or one payload; constructors use qualified `Type::Variant` names in OSV
  order, patterns use the same qualified names, and matches are exhaustive,
  including when a custom enum is a `Result` error.
- Add end-to-end Warder builds and test checks for direct local path
  dependencies. Manifest dependency keys bind compiler package namespaces,
  package roots map to `src/lib.rl`, and submodules map below `src/`.
- Record each local dependency's manifest version and deterministic source
  SHA-256 in `restrict-lock.toml`.
- Compile application and dependency sources from immutable staging snapshots,
  serialize concurrent builds per project, and publish WAT, WASM, Cage, and
  lock updates as one recoverable artifact transaction.
- Diagnose missing, malformed, and source-stale direct-local lock files with
  `warder doctor`.
- Add the experimental, opt-in `--host-abi flat-record-v1` core-WebAssembly
  adapter for concrete non-temporal records with 1 to 16 direct scalar fields.
  Record parameters flatten in source field order and record results use
  multi-value returns, while generated wrappers keep internal pointers and
  layout identifiers private.

### Compatibility

- Keep `pub enum` source-module-only. User enums have no host-visible
  WebAssembly ABI; generic and recursive enums and `?` propagation remain
  outside the current slice.
- Keep registry, Git, foreign-WASM, and transitive package graphs outside the
  direct-local v0.0.1 slice. Warder rejects them before writing placeholder
  lock entries; source-level import aliases and re-exports remain unsupported.
- Keep the published v0.0.1 host ABI scalar-only by default. Strings, nested
  records, collections, sum types, generics, temporal values, composite
  globals, WIT, and Component Model output remain outside `flat-record-v1`.

### Fixed

- Keep one canonical declaration identity across split, direct, transitive,
  and diamond source imports; use collision-proof internal names and emit each
  dependency module once.
- Reject duplicate exports, duplicate normalized virtual modules, ambiguous
  explicit search roots, and complete import cycles without poisoning the
  resolver cache after a failed attempt.
- Reject overlapping application, package, and build roots; symlink escapes;
  non-portable artifact names; and non-deterministic dependency manifest order.

## [0.0.1] - 2026-08-04

### Added

- OSV-only calls through `value |> function`, `(args) function`, and `() function`
- Affine binding checks with `val` and `mut val`
- Bidirectional type inference for the supported generic, lambda, and container surface
- Pattern matching for built-in `Option`, `Result`, `List`, and record values
- Source modules with dotted imports and scalar WebAssembly exports
- WebAssembly text generation, arena allocation, and executable runtime examples
- Browser compiler with compile, tokenize, and parse inspection views
- Language Server Protocol support and the Warder project tool

### Release boundaries

- Host-visible exports are limited to concrete scalar parameters, results, and literal constants
- User-defined ADTs, source-level `form`/`takes`, temporal affine types, and direct composite host ABI are reserved for later design work
- Warder registry publishing performs local preflight validation only and uploads nothing
- Homebrew and VS Code Marketplace distribution are not part of this preview release

[Unreleased]: https://github.com/Ischca/restrict_lang/compare/v0.0.1...HEAD
[0.0.1]: https://github.com/Ischca/restrict_lang/releases/tag/v0.0.1
