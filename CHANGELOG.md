# Changelog

All notable changes to the Restrict Language compiler are documented here.
The format follows [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

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

[0.0.1]: https://github.com/Ischca/restrict_lang/releases/tag/v0.0.1
