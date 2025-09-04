<div align="center">
  <img src="assets/logo.svg" alt="Restrict Language Logo" width="200" height="200">
  
  # Restrict Language
  
  **A functional programming language with affine types for WebAssembly**
  
  [![CI](https://github.com/restrict-lang/restrict_lang/actions/workflows/ci.yml/badge.svg)](https://github.com/restrict-lang/restrict_lang/actions/workflows/ci.yml)
  [![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
  [![WASM](https://img.shields.io/badge/target-WebAssembly-orange.svg)](https://webassembly.org/)
  [![Documentation](https://img.shields.io/badge/docs-mdBook-green.svg)](https://restrict-lang.github.io/restrict_lang/)
</div>

---

## 📖 Language Specification

**IMPORTANT**: The authoritative language specification is at [`LANGUAGE_SPECIFICATION.md`](LANGUAGE_SPECIFICATION.md). This document defines all syntax rules, type system behavior, and language semantics. Always refer to this document when implementing features or writing code.

A statically-typed functional programming language that compiles to WebAssembly, featuring an affine type system, pattern matching, lambda expressions with closures, and arena-based memory management.

## 🚀 Quick Start

### From Source

```bash
git clone https://github.com/restrict-lang/restrict_lang
cd restrict_lang

# Build the compiler
mise exec -- cargo build --release

# Compile your first program
echo 'fun main: () -> Int32 = { 42 }' > hello.rl
./target/release/restrict_lang hello.rl

# Run the generated WebAssembly
wasmtime hello.wat
```

## ✨ Features

- **🔒 Affine Type System**: Variables can be used at most once, preventing accidental resource duplication
- **🧠 Arena Memory Management**: No garbage collection, deterministic memory usage with arena allocation
- **🎯 Pattern Matching**: Exhaustive pattern matching with type safety for Option, List, and Record types  
- **🌟 Lambda Expressions**: First-class functions with closure capture and bidirectional type inference
- **⚡ WebAssembly Target**: Compiles to efficient WebAssembly with WASI support for the current concrete ABI surface
- **📝 OSV Syntax**: Object-Subject-Verb syntax for natural function composition (traditional function calls not supported)
- **💬 Comments**: Full support for single-line (`//`) and multi-line (`/* */`) comments
- **🎪 Spread Destructuring**: Extract multiple record fields with rest patterns `{ field1, field2, ...rest }`
- **⏰ Temporal Affine Types**: Experimental and excluded from the default v0.0.1 gate

## v0.0.1 Design Boundaries

The v0.0.1 release intentionally keeps a few language-shaping decisions out of
the default support promise. These are not partially-complete user features:
they need explicit design decisions before implementation.

- User-defined `enum`/ADT declarations are reserved syntax only. Built-in
  `Option<T>` and `Result<T, E>` are supported for sum-type workflows today.
- User-defined `form`, `takes`, `of`, and associated-type declarations are
  future design work. The current `Container` behavior used by `map` and
  `filter` is a compiler-internal constraint for built-in `List` and `Option`.
- Exported generic functions do not yet have a stable WebAssembly ABI and are
  rejected by v0.0.1 release-surface validation before `--check` success or
  code generation. Exported records are source-level module metadata only; they
  do not create host-visible Wasm exports until a concrete record ABI is
  designed.
- Exported top-level constants are host-visible only when their literal value
  has a scalar ABI: `Int32`, `Int64`, `Float64`, `Boolean`, `Char`, or `()`.
  Composite global exports are rejected until that ABI is designed.
- Generic functions, records, `Option`, and `Result` remain supported inside
  Restrict programs. Records may cross source-module boundaries; generic
  functions may not cross the unresolved host export ABI boundary.

## 📖 Language Overview

### Hello World

```restrict
// hello.rl
fun main: () -> () = {
    "Hello, Restrict Language!" |> println
}
```

### Basic Arithmetic

```restrict
// arithmetic.rl
fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

fun main: () -> Int32 = {
    val result = (10, 20) add
    "Result: " |> println
    result |> print_int
    result
}
```

### Variables and Affine Types

```restrict
// Copyable primitives can be read more than once
val score: Int32 = 42
val doubled = score + score

// Heap-backed values move when used
val message = "hello"
val next = message

// Mutable bindings can be reassigned
mut val counter = 0
counter = counter + 1
counter = counter + 1
```

### Functions and OSV Syntax

Restrict Language **exclusively uses OSV (Object-Subject-Verb) syntax**. Function-first call syntax is **not supported**.

```restrict
// Function definition
fun add: (x: Int32, y: Int32) -> Int32 = { x + y }

fun double: (value: Int32) -> Int32 = { value * 2 }

fun say_hello: () -> String = { "hello" }

// OSV function calls (ONLY supported syntax)
val result = (5, 10) add      // Multiple arguments: (args) function
val doubled = 21 |> double    // Single argument: value |> function
val greeting = () say_hello   // No arguments: () function

// Pattern: Arguments come BEFORE the function name
val product = (2, 3) multiply             // Direct OSV with multiple args
val message = "Hello, " + "Restrict"      // Current string composition

// Function composition flows naturally left-to-right
val process_data = data 
    |> validate 
    |> transform 
    |> save_to_database
```

### Lambda Expressions and Closures

```restrict
fun apply_int: (f: Int32 -> Int32, value: Int32) -> Int32 = {
    value |> f
}

fun main: () -> Int32 = {
    (|x| x * 2, 21) apply_int
}
```

### Pattern Matching

```restrict
record Point { x: Int32, y: Int32 }

fun score_option: (maybe_value: Option<Int32>) -> Int32 = {
    maybe_value match {
        Some(value) => { value * 2 }
        None => { 0 }
    }
}

fun score_list: (numbers: List<Int32>) -> Int32 = {
    numbers match {
        [] => { 0 }
        [head | tail] => { head }
        [a, b] => { a + b }
        _ => { 1 }
    }
}

fun score_point: (point: Point) -> Int32 = {
    point match {
        Point { x: 0, y: 0 } => { 0 }
        Point { x, y } => { x + y }
        _ => { 1 }
    }
}
```

### Spread Destructuring

Spread destructuring allows you to extract specific fields from records while capturing the rest:

```restrict
record User {
    name: String,
    email: String,
    age: Int32,
    department: String,
    role: String
}

fun display_user: (user: User) -> String = {
    user match {
        User { name, role: "Manager", ..._ } => { name + " is a manager" }
        User { name, department: "Engineering", ..._ } => { name + " is an engineer" }
        User { name, ..._ } => { name + " works here" }
    }
}

// Practical example: User profile updates
record UserUpdates {
    name: Option<String>,
    email: Option<String>
}

fun choose_string: (candidate: Option<String>, fallback: String) -> String = {
    candidate match {
        Some(value) => { value }
        None => { fallback }
    }
}

fun update_profile: (updates: UserUpdates) -> User = {
    val current_user = () get_current_user
    val User { name: new_name, email: new_email } = updates
    val User { name, email, age, department, role } = current_user

    User {
        name: (new_name, name) choose_string,
        email: (new_email, email) choose_string,
        age: age,
        department: department,
        role: role
    }
}
```

### Records and Methods

```restrict
// Record definition
record Person {
    name: String,
    age: Int32,
    email: String
}

// Impl functions are type-directed, but calls remain OSV.
impl Person {
    fun is_adult: (self: Person) -> Boolean = {
        self.age >= 18
    }
}

fun greet: (self: Person) -> String = {
    "Hello, " + self.name + "!"
}

// Usage
val alice = Person { name: "Alice", age: 30, email: "alice@example.com" }
val greeting = alice |> greet
val bob = Person { name: "Bob", age: 17, email: "bob@example.com" }
val adult = (bob) is_adult
```

### Arena Memory Management

```restrict
// Arena context with scoped temporary heap allocation
fun process_batch: () -> Int32 = {
    with Arena { } {
        val big_list = [1, 2, 3, 4, 5]
        big_list |> list_count
    }
}
```

### Temporal Affine Types (TAT)

Temporal Affine Types are part of the long-term Restrict design, but they are
outside the default v0.0.1 quality gate. TAT-specific tests are available behind
the `tat` Cargo feature while the core language release focuses on OSV syntax,
affine checking, type inference, pattern matching, and WebAssembly codegen.

## 📚 Documentation

- **[Quick Start](docs/en/getting-started/quick-start.md)** - Build and run a first v0.0.1 project
- **[Language Guide](docs/en/guide/README.md)** - Release-facing v0.0.1 syntax and design rules
- **[Release Surface](docs/v001-release-surface.md)** - Supported, rejected, and experimental v0.0.1 boundaries
- **[Examples](examples/)** - Sample programs and use cases

## 🏗️ Implementation Status

### ✅ Completed Features

- [x] Lexer with comment support
- [x] Parser with OSV syntax (traditional function calls not supported)
- [x] Type system with affine types
- [x] Lambda expressions with closures
- [x] Pattern matching (Option, List, Record)
- [x] Spread destructuring with `...rest` syntax
- [x] WebAssembly code generation
- [x] Arena memory management
- [x] Bidirectional type inference
- [x] Function tables for indirect calls
- [x] Higher-order functions (`map`, `filter`, `fold`) with typed function values
- [x] Generics and parametric polymorphism inside Restrict programs
- [x] Result types with expected-type inference
- [x] Type-directed `impl` method dispatch through grouped OSV calls
- [x] Source import resolution through the CLI
- [x] Affine checking across complex expressions, OSV calls, field access, and branching
- [x] Conditional expressions with chained/nested runtime coverage
- [x] Recursive functions with direct and mutual runtime coverage

### 🚧 In Progress

- [ ] Temporal Affine Types (TAT) outside the default v0.0.1 gate
- [ ] User-defined enum/ADT declarations (`enum` is reserved; built-in `Option`/`Result` are supported)
- [ ] Package-level module aliases, re-exports, and std aggregators beyond source-file import resolution
- [ ] Direct WebAssembly ABI for exported generic functions and host-visible record values

### 📋 Planned Features

- [ ] String interpolation
- [ ] Async/await support
- [ ] Ergonomic error propagation syntax
- [ ] SIMD operations
- [ ] WebGPU backend

### ⚠️ Current Boundaries

- Pattern guards and tuple patterns are future/design gaps; v0.0.1 covers
  Option, Result, List, Record, nested, and spread record patterns
- Source-file imports are implemented; package-level module aliases, re-exports,
  and std aggregators still need a concrete design
- String interpolation is not part of the v0.0.1 syntax; use concatenation today
- User-defined `enum`/ADT declarations are an intentional v0.0.1 design gap;
  use built-in `Option` and `Result` today
- Exported generic functions require a concrete WebAssembly ABI design before
  codegen support; exported records are source-level only and emit no direct
  host-visible Wasm export
- TAT examples and tests are experimental and run outside the default test gate
- Some older examples are design sketches and may use syntax that is not in the v0.0.1 gate
- Mutable variables use `mut val`

## 🔧 Architecture

```
Source Code (.rl)
    ↓
Lexer → Tokens
    ↓  
Parser → AST
    ↓
Type Checker → Typed AST
    ↓
Code Generator → WebAssembly (.wat)
    ↓
WebAssembly Runtime (wasmtime, browser, etc.)
```

### Type System

- **Affine Types**: Each variable can be used at most once
- **Arena Allocation**: Memory management without garbage collection
- **Static Type Checking**: Catch errors at compile time
- **Type Inference**: Bidirectional type checking for lambdas

### WebAssembly Backend

- Compiles to WebAssembly Text Format (WAT)
- Supports WASI for I/O operations
- Function tables for lambda/closure calls
- Linear memory management with arenas

## 🧪 Testing

```bash
# Run all tests
mise exec -- cargo test

# Run specific test suites
mise exec -- cargo test lambda        # Lambda expression tests
mise exec -- cargo test pattern       # Pattern matching tests
mise exec -- cargo test type_check    # Type checker tests
mise exec -- cargo test codegen       # Code generation tests
```

## 🤝 Contributing

We welcome contributions! Please see our contributing guidelines:

1. **Fork** the repository
2. **Create** a feature branch (`git checkout -b feature/amazing-feature`)
3. **Commit** your changes (`git commit -m 'Add amazing feature'`)
4. **Push** to the branch (`git push origin feature/amazing-feature`)
5. **Open** a Pull Request

### Development Setup

```bash
# Install Rust (if not already installed)
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Clone and build
git clone https://github.com/restrict-lang/restrict_lang
cd restrict_lang
mise exec -- cargo build

# Run tests
mise exec -- cargo test

# Install WebAssembly runtime for testing
curl https://wasmtime.dev/install.sh -sSf | bash
```

## 📜 License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

## 🙏 Acknowledgments

- Inspired by Rust's affine type system
- WebAssembly community for excellent tooling
- Functional programming language research

## 📊 Project Stats

![GitHub Stars](https://img.shields.io/github/stars/restrict-lang/restrict_lang)
![GitHub Forks](https://img.shields.io/github/forks/restrict-lang/restrict_lang)
![GitHub Issues](https://img.shields.io/github/issues/restrict-lang/restrict_lang)
![License](https://img.shields.io/github/license/restrict-lang/restrict_lang)

---

**Restrict Language** - Making functional programming efficient and safe for WebAssembly 🚀
