// Standard Library: legacy IO reference surface
//
// This file is not the runtime implementation. The current compiler registers
// IO symbols directly in the Rust type checker and WebAssembly codegen.
// Keep this path as a compatibility reference index for readers and tests.
//
// Current compiler-registered surface:
// - println: (String) -> ()
// - print: (String) -> ()
// - print_int: (Int32) -> ()
// - print_float: (Float64) -> ()
// - eprint: (String) -> ()
// - eprintln: (String) -> ()
//
// Canonical OSV call shapes:
// - "hello" |> println
// - "hello" |> print
// - 42 |> print_int
// - 3.14 |> print_float
// - "error" |> eprintln
//
// Source modules under this directory are reference notes only. They must stay
// comment-only until the compiler exposes a parseable source stdlib format.
