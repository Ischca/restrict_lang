// Standard Library: List reference surface
//
// This file is not the runtime implementation. The current compiler registers
// list symbols directly in the Rust type checker and WebAssembly codegen.
// Keep this file as a canonical v0.0.1, source-adjacent index for readers and
// tests.
//
// Current compiler-registered surface:
// - list_is_empty: <T>(List<T>) -> Boolean
// - list_head: <T>(List<T>) -> Option<T>
// - list_tail: <T>(List<T>) -> Option<List<T>>
// - list_reverse: <T>(List<T>) -> List<T>
// - list_prepend: <T>(T, List<T>) -> List<T>
// - list_append: <T>(List<T>, T) -> List<T>
// - list_concat: <T>(List<T>, List<T>) -> List<T>
// - list_count: <T>(List<T>) -> Int32
//
// Compiler list builtins that are also source-callable:
// - list_length: <T>(List<T>) -> Int32
// - list_get: <T>(List<T>, Int32) -> T
//
// Canonical call shapes:
// - values |> list_is_empty
// - values |> list_head
// - values |> list_tail
// - values |> list_reverse
// - (item, values) list_prepend
// - (values, item) list_append
// - (left, right) list_concat
// - values |> list_count
// - values |> list_length
// - (values, index) list_get
//
// Higher-order helpers such as list_map, list_filter, and list_fold_left are
// intentionally absent. The current compiler-registered v0.0.1 surface exposes
// generic container map, filter, and fold through the prelude instead.
