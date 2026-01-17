<div align="center">
  <img src="assets/logo.svg" alt="Restrict Language Logo" width="200" height="200">
  
  # Restrict Language
  
  **A functional programming language with affine types for WebAssembly**
  
  [![CI](https://github.com/restrict-lang/restrict_lang/actions/workflows/ci.yml/badge.svg)](https://github.com/restrict-lang/restrict_lang/actions/workflows/ci.yml)
  [![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
  [![WASM](https://img.shields.io/badge/target-WebAssembly-orange.svg)](https://webassembly.org/)
  [![Documentation](https://img.shields.io/badge/docs-mdBook-green.svg)](https://restrict-lang.github.io/restrict_lang/)
  [![Playground](https://img.shields.io/badge/playground-try%20online-brightgreen.svg)](https://ischca.github.io/restrict_lang/)
</div>

---

A statically-typed functional programming language that compiles to WebAssembly, featuring an affine type system, pattern matching, lambda expressions with closures, and arena-based memory management.

## 🎮 Try it Online

Experience Restrict Language directly in your browser without any installation:

**[🚀 Launch Playground](https://ischca.github.io/restrict_lang/)**

The online playground allows you to:
- ✍️ Write and execute Restrict Language code instantly
- 🧪 Explore language features interactively  
- 🔗 Share code snippets with others
- 📚 Learn from built-in examples

---

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
- **🧬 Generics**: Parametric polymorphism with type inference and monomorphization
- **🧠 Arena Memory Management**: No garbage collection, deterministic memory usage with arena allocation
- **🎯 Pattern Matching**: Exhaustive pattern matching with type safety for Option, List, and Record types
- **🌟 Lambda Expressions**: First-class functions with closure capture and bidirectional type inference
- **⚡ WebAssembly Target**: Compiles to efficient WebAssembly with WASI support
- **📝 OSV Syntax**: Object-Subject-Verb syntax for natural function composition
- **🔌 Context Binding**: Implicit parameter passing via algebraic effect-like contexts
- **🧩 Scope Composition**: Combine multiple contexts with nested `with` blocks
- **📦 Module System**: Import/export with file-based module resolution
- **💬 Comments**: Full support for single-line (`//`) and multi-line (`/* */`) comments
- **✂️ Semicolon-Free**: Kotlin-style newline-based statement termination (semicolons optional)
- **🎨 Concise Syntax**: Parameter-less functions like `fun main = { ... }` without explicit `(): ()`

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

// Semicolons are optional - use newlines to separate statements
val a = 1
val b = 2
val c = a + b
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
val result = maybe_value match {
    Some(value) => { value * 2 }
    None => { 0 }
}

// List pattern matching
val numbers = [1, 2, 3, 4]
val description = numbers match {
    [] => { "empty" }
    [head | tail] => { "head: " + head }
    [a, b] => { "exactly two" }
    _ => { "other" }
}

// Record pattern matching
record Point { x: Int y: Int }
val point = Point { x: 10 y: 20 }
val sum = point match {
    Point { x: 0 y: 0 } => { "origin" }
    Point { x y } => { x + y }  // Destructure both fields
    _ => { "unknown" }
}
```

### Generics

```rust
// Generic function
fun identity<T>: (x: T) -> T = {
    x
}

// Generic record
record Box<T> {
    value: T
}

// Usage - types are inferred
val a = 42 identity          // T = Int
val b = "hello" identity     // T = String
val box = Box { value = 42 } // Box<Int>
```

### Records and Methods

```rust
// Record definition (comma-separated fields)
record Person {
    name: String,
    age: Int,
    email: String
}

// Method implementation
impl Person {
    fun greet: (self: Person) -> String = {
        "Hello, " + self.name + "!"
    }

    fun is_adult: (self: Person) -> Bool = {
        self.age >= 18
    }
}

// Usage
val alice = Person { name = "Alice", age = 30, email = "alice@example.com" }
val greeting = alice greet  // "Hello, Alice!"
val adult = alice is_adult  // true
```

### Arena Memory Management

```rust
// Arena-based memory - automatically freed when scope ends
fun main = {
    with Arena {
        val user = Person { name = "Bob", age = 25 }
        // Process data...
        // All memory automatically freed when leaving scope
    }
}
```

### Context Binding (Implicit Parameters)

```rust
// Define a context (like Reader monad / algebraic effects)
record Connection { id: Int }

context Database {
    val conn: Connection
}

// Function requires Database context
fun query: (sql: String) -> String with Database = {
    sql  // Would use conn from context in real code
}

fun main = {
    with Arena {
        val conn = Connection { id = 1 }
        // Provide the context
        with Database { conn = conn } {
            "SELECT * FROM users" query |> println
        }
    }
}
```

### Scope Composition (Multiple Contexts)

```rust
context Logging { val logger: Logger }
context Config { val config: AppConfig }

// Function requires BOTH contexts
fun log_with_config: () with Logging, Config = {
    "Logging with config" |> println
}

fun main = {
    with Arena {
        // Nested contexts compose automatically
        with Logging { logger = myLogger } {
            with Config { config = myConfig } {
                log_with_config
            }
        }
    }
}
```

### FizzBuzz Example

```rust
fun fizzbuzz: (n: Int) -> String = {
    n % 15 == 0 then { "FizzBuzz" } else {
        n % 3 == 0 then { "Fizz" } else {
            n % 5 == 0 then { "Buzz" } else {
                n int_to_string
            }
        }
    }
}

fun main = {
    mut val i = 1
    i <= 20 while {
        i fizzbuzz |> println
        i = i + 1
    }
}
```

### Semicolon-Free Syntax

Restrict Language uses Kotlin-style newline-based statement termination. Semicolons are optional in most cases:

```rust
// Statements separated by newlines (no semicolons needed)
fun main: () -> Int = {
    val x = 42
    val y = 10
    val result = x + y
    result
}

// Multiple statements on one line require semicolons
fun compact: () -> Int = { val a = 1; val b = 2; a + b }

// Line continuation: operators at end of line continue to next line
fun multiline: () -> Int = {
    val sum = 10 +
        20 +
        30

    val piped = 42 |>
        println

    sum
}

// Method chaining can span multiple lines
val result = obj
    .method1()
    .method2()
    .method3()
```

## 📚 Documentation

- **[Tutorial](TUTORIAL.md)** - Learn the language step by step
- **[Reference](REFERENCE.md)** - Complete language reference
- **[Examples](examples/)** - Sample programs and use cases
- **[Temporal Affine Types Guide](docs/TEMPORAL_DESIGN_GUIDE.md)** - Comprehensive TAT documentation
- **[TAT Implementation Status](docs/TAT_IMPLEMENTATION_STATUS.md)** - Current TAT implementation progress

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
- [x] Kotlin-style semicolon-free syntax
- [x] Conditional expressions (`then`/`else` syntax)
- [x] Context binding with `context` and `with` blocks
- [x] Scope composition (multiple contexts)
- [x] Concise syntax (`fun main = { ... }` without parameter list)
- [x] Return type inference from function body

### 🚧 In Progress

- [ ] Higher-order functions (map, filter, fold)
- [ ] String interpolation
- [ ] Module system
- [ ] Recursive functions (basic parsing works, but execution has limitations)

### 📋 Planned Features

- [ ] Async/await support
- [ ] Error handling with Result types
- [ ] Generics and parametric polymorphism
- [ ] SIMD operations
- [ ] WebGPU backend

### ⚠️ Current Limitations

- Recursive functions are not fully supported yet
- Complex pattern matching may not work in all cases
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