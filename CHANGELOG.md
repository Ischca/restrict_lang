# Changelog

All notable changes to the Restrict Language compiler will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

## [0.1.0] - 2026-03-11

### Added
- Complete OSV (Object-Subject-Verb) syntax
- Affine type system with use-at-most-once semantics
- Pattern matching for Option, Result, List, and Record types
- Generic functions with monomorphization
- Prototype-based records with `clone` and `freeze`
- Context-based resource management (`with` blocks)
- Arena allocator with bounds checking and nested arena support
- Module system with imports and exports
- Standard library: Option, Result, List, String, I/O
- FileSystem context for safe file operations
- WebAssembly (WAT) code generation
- LSP server with diagnostics, document symbols, and hover
- WASM playground for browser-based editing
- Pipe operators (`|>` for immutable, `|>>` for mutable binding)
- Single-line (`//`) and multi-line (`/* */`) comments

### Known Limitations
- Float64 fields in record pattern matching assume 4-byte offsets
- Closures do not capture free variables from outer scope
- While loop code generation is not complete
- Pattern guards are not yet implemented
- `it` implicit parameter defaults to Int32 type
- Temporal Affine Types (TAT) are experimental and deferred to v2.0
