// Standard Library: Prelude reference surface
//
// This file is not the runtime implementation. The current compiler registers
// prelude symbols directly in the Rust type checker and WebAssembly codegen.
// Keep this file as a canonical, source-adjacent index for readers and tests.
//
// Current compiler-registered surface:
// - identity: <T>(T) -> T
// - map: generic container mapping builtin
// - filter: generic container filtering builtin
// - fold: generic List reduction builtin
// - not: (Boolean) -> Boolean
// - and: (Boolean, Boolean) -> Boolean
// - or: (Boolean, Boolean) -> Boolean
// - assert: (Boolean, String) -> ()
// - panic: (String) -> ()
//
// Canonical call shapes:
// - value |> identity
// - value |> mapper |> next_mapper
// - condition |> not
// - (left, right) and
// - (condition, "expected condition to hold") assert
//
// Helpers such as xor, eq, ne, when, and debug_assert are absent from the
// compiler-registered v0.0.1 surface.
