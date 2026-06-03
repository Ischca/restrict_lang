// Standard Library: root reference surface
//
// This file is not the runtime implementation. The current compiler registers
// std symbols directly in the Rust type checker and WebAssembly codegen.
// Keep this file as a canonical v0.0.1, source-adjacent index for readers and
// tests.
//
// Current checked-in reference modules:
// - prelude: automatically available compiler-registered helpers
// - io: compiler-registered console output helpers
// - string: source-level string operations and lowered runtime helpers
// - math: compiler-registered numeric helpers
// - list: compiler-registered list helpers
// - option: compiler-registered Option helpers
//
// The compiler-registered v0.0.1 surface has no parseable source import/export
// aggregator. This file is an index of the current reference modules.
