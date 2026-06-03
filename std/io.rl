// Standard Library: IO reference surface
//
// This file is not the runtime implementation. The current compiler registers
// IO symbols directly in the Rust type checker and WebAssembly codegen.
// Keep this file as a canonical, source-adjacent index for readers and tests.
//
// Current compiler-registered surface:
// - println: (String) -> ()
// - print: (String) -> ()
// - print_int: (Int32) -> ()
// - print_float: (Float64) -> ()
// - eprint: (String) -> ()
// - eprintln: (String) -> ()
//
// Canonical call shapes:
// - "hello" |> println
// - "hello" |> print
// - 42 |> print_int
// - 3.14 |> print_float
// - "error" |> eprintln
//
// Input-reading and file APIs are absent from the compiler-registered v0.0.1 surface.
