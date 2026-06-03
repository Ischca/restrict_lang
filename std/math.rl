// Standard Library: Math reference surface
//
// This file is not the runtime implementation. The current compiler registers
// math symbols directly in the Rust type checker and WebAssembly codegen.
// Keep this file as a canonical v0.0.1, source-adjacent index for readers and
// tests.
//
// Current compiler-registered surface:
// - abs: (Int32) -> Int32
// - max: (Int32, Int32) -> Int32
// - min: (Int32, Int32) -> Int32
// - pow: (Int32, Int32) -> Int32
// - factorial: (Int32) -> Int32
// - abs_f: (Float64) -> Float64
// - max_f: (Float64, Float64) -> Float64
// - min_f: (Float64, Float64) -> Float64
//
// Canonical call shapes:
// - value |> abs
// - (left, right) max
// - (left, right) min
// - (base, exponent) pow
// - value |> factorial
// - value |> abs_f
// - (left, right) max_f
// - (left, right) min_f
//
// Additional numeric helpers are absent from the compiler-registered v0.0.1
// surface.
