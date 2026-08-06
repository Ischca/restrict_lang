// Standard Library: Option reference surface
//
// This file is not the runtime implementation. The current compiler registers
// option symbols directly in the Rust type checker and WebAssembly codegen.
// Keep this file as a canonical v0.0.1, source-adjacent index for readers and
// tests.
//
// Current compiler-registered surface:
// - option_is_some: <T>(Option<T>) -> Boolean
// - option_is_none: <T>(Option<T>) -> Boolean
// - option_unwrap_or: <T>(Option<T>, T) -> T
//
// Source-level constructors:
// - value Option::Some: Option<T> payload constructor syntax
// - () Option::None: Option<T> empty constructor syntax
// - value |> Option::Some: equivalent piped payload form
// Match patterns stay unqualified: Some(value) and None.
//
// Canonical call shapes:
// - maybe_value |> option_is_some
// - maybe_value |> option_is_none
// - (maybe_value, fallback) option_unwrap_or
//
// Higher-order helpers such as option_map, option_flatten, option_and_then,
// option_zip, and option_to_list are absent from the compiler-registered
// v0.0.1 surface.
