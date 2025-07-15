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

A statically-typed functional programming language that compiles to WebAssembly, featuring an affine type system, pattern matching, lambda expressions with closures, and arena-based memory management.

## 🚀 Quick Start

### Using the Package Manager (Warder)

```bash
# Install warder (package manager)
# TODO: Add installation instructions

# Create a new project
warder new my-project
cd my-project

# Build and run
warder build
warder run
```

### Manual Compilation

```bash
# Clone the repository
git clone https://github.com/restrict-lang/restrict_lang
cd restrict_lang

# Build the compiler
cargo build --release

# Compile your first program
echo 'fun main = { "Hello, World!" |> println }' > hello.rl
./target/release/restrict_lang hello.rl

# Run the generated WebAssembly
wasmtime hello.wat
```

## ✨ Features

- **🔒 Affine Type System**: Variables can be used at most once, preventing accidental resource duplication
- **🧠 Arena Memory Management**: No garbage collection, deterministic memory usage with arena allocation
- **🎯 Pattern Matching**: Exhaustive pattern matching with type safety for Option, List, and Record types  
- **🌟 Lambda Expressions**: First-class functions with closure capture and bidirectional type inference
- **⚡ WebAssembly Target**: Compiles to efficient WebAssembly with WASI support
- **📝 OSV Syntax**: Object-Subject-Verb syntax for natural function composition
- **💬 Comments**: Full support for single-line (`//`) and multi-line (`/* */`) comments

## 📖 Language Overview

### Hello World

```rust
// hello.rl
fun main = {
    "Hello, Restrict Language!" |> println
}
```

### Basic Arithmetic

```rust
// arithmetic.rl
fun add = x:Int y:Int {
    x + y
}

fun main = {
    val result = (10, 20) add
    "Result: " |> println
    result |> print_int
}
```

### Variables and Affine Types

```rust
// Variables can only be used once (affine types)
val x = 42
val y = x    // x is consumed here
// val z = x // Error: x already used!

// Mutable variables can be reused
mut val counter = 0
counter = counter + 1  // OK
counter = counter + 1  // OK
```

### Functions and OSV Syntax

```rust
// Function definition
fun add = x:Int y:Int { x + y }

// OSV (Object-Subject-Verb) function calls
val result = (5, 10) add      // Multiple arguments

// Pipe operator for single arguments
val doubled = 21 |> double
```

### Lambda Expressions and Closures

```rust
// Simple lambda
val double = |x| x * 2
val result = 21 |> double  // Returns 42

// Closures capture variables (advanced feature - may have limitations)
fun make_adder = n:Int {
    |x| x + n  // Captures 'n'
}

val add5 = 5 |> make_adder
val result = 10 |> add5  // Returns 15
```

### Pattern Matching

```rust
// Option pattern matching
val maybe_value: Option<Int> = Some(42)
maybe_value match {
    Some(value) => { value * 2 }
    None => { 0 }
}

// List pattern matching
val numbers = [1, 2, 3, 4]
numbers match {
    [] => { "empty" }
    [head | tail] => { "head: " + head }
    [a, b] => { "exactly two" }
    _ => { "other" }
}
```

### Records and Methods

```rust
// Record definition
record Person {
    name: String,
    age: Int,
    email: String,
}

// Method implementation
impl Person {
    fun greet = self:Person {
        "Hello, " + self.name + "!"
    }
    
    fun is_adult = self:Person {
        self.age >= 18
    }
}

// Usage
val alice = Person { name: "Alice", age: 30, email: "alice@example.com" }
val greeting = alice.greet()  // "Hello, Alice!"
val adult = alice.is_adult()  // true
```

### Arena Memory Management

```rust
// Create an arena for memory management
val arena = new_arena(1024)  // 1KB arena

arena {
    val big_list = [1, 2, 3, /* many elements */]
    val user = Person { name: "Bob", age: 25, email: "bob@test.com" }
    
    // Process data...
    // All memory automatically freed when leaving scope
}
```

## 📚 Documentation

- **[Tutorial](TUTORIAL.md)** - Learn the language step by step
- **[Reference](REFERENCE.md)** - Complete language reference
- **[Examples](examples/)** - Sample programs and use cases

## 🏗️ Implementation Status

### ✅ Completed Features

- [x] Lexer with comment support
- [x] Parser with OSV syntax
- [x] Type system with affine types
- [x] Lambda expressions with closures
- [x] Pattern matching (Option, List, Record)
- [x] WebAssembly code generation
- [x] Arena memory management
- [x] Bidirectional type inference
- [x] Function tables for indirect calls

### 🚧 In Progress

- [ ] Higher-order functions (map, filter, fold)
- [ ] String interpolation
- [ ] Module system
- [ ] Conditional expressions (`then`/`else` syntax parsing works, but runtime issues remain)
- [ ] Recursive functions (basic parsing works, but execution has limitations)
- [ ] Complex affine type usage (multiple variable references in expressions)

### 📋 Planned Features

- [ ] Async/await support
- [ ] Error handling with Result types
- [ ] Generics and parametric polymorphism
- [ ] SIMD operations
- [ ] WebGPU backend

### ⚠️ Current Limitations

- Conditional expressions (`then`/`else`) have parsing support but runtime limitations
- Recursive functions are not fully supported yet
- Complex pattern matching may not work in all cases
- Some syntax features in examples may not be fully implemented
- Use `mut val` instead of `val mut` for mutable variables

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
cargo test

# Run specific test suites
cargo test lambda        # Lambda expression tests
cargo test pattern      # Pattern matching tests  
cargo test type_check    # Type checker tests
cargo test codegen       # Code generation tests
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
cargo build

# Run tests
cargo test

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