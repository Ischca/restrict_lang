// Standard Library: Result reference surface
//
// This file is not the runtime implementation. The current compiler registers
// Result constructors and pattern matching directly in the Rust type checker
// and WebAssembly codegen.
// Keep this file as a canonical, source-adjacent index for readers and tests.
//
// Current compiler-registered surface:
// - value Result::Ok: Result<T, E> success constructor syntax
// - error Result::Err: Result<T, E> error constructor syntax
// - match arms over Ok(value) and Err(value)
//
// Canonical expression shapes:
// - 42 Result::Ok
// - 7 Result::Err
// - 42 |> Result::Ok
// - result match { Ok(value) => { value } Err(code) => { code } }
//
// Higher-order Result helpers are outside the compiler-registered v0.0.1
// surface.
