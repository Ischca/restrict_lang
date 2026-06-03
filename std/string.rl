// Standard Library: String reference surface
//
// This file is not the runtime implementation. The current compiler handles
// core string operations directly in Rust codegen, including concatenation and
// content equality. Keep this file as a canonical, source-adjacent index for
// readers and tests.
//
// Current source-level string operations:
// - a + b: concatenate two String values
// - a == b: compare String contents
// - a != b: compare String contents and negate the result
//
// Lowered runtime helpers:
// - string_concat: (String, String) -> String
// - string_eq: (String, String) -> Boolean
//
// Canonical expression shapes:
// - first + second
// - first == second
// - first != second
//
// Length, parsing, and formatting helpers are absent from the
// compiler-registered v0.0.1 surface.
