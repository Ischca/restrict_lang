<div align="center">
  <img src="assets/logo.svg" alt="Restrict Language Logo" width="200" height="200">

  # Restrict Language

  **A functional programming language with affine types for WebAssembly**

  [![CI](https://github.com/Ischca/restrict_lang/actions/workflows/ci.yml/badge.svg)](https://github.com/Ischca/restrict_lang/actions/workflows/ci.yml)
  [![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
  [![WASM](https://img.shields.io/badge/target-WebAssembly-orange.svg)](https://webassembly.org/)
  [![Documentation](https://img.shields.io/badge/docs-mdBook-green.svg)](https://ischca.github.io/restrict_lang/docs/)

  [Website](https://ischca.github.io/restrict_lang/) · [Playground](https://ischca.github.io/restrict_lang/compiler/) · [Release surface](docs/public/en/reference/release-surface.md)
</div>

---

## 📖 Language Specification

**IMPORTANT**: The authoritative language specification is at [`LANGUAGE_SPECIFICATION.md`](LANGUAGE_SPECIFICATION.md). This document defines all syntax rules, type system behavior, and language semantics. Always refer to this document when implementing features or writing code.

A statically-typed functional programming language that compiles to WebAssembly, featuring an affine type system, pattern matching, lambda expressions with closures, and arena-based memory management.

## WebAssembly-First Execution Model

WebAssembly is Restrict's sole code-generation target. Native WASI runtimes,
browsers, cloud/edge platforms, and container runtimes are hosts for the same
Wasm backend rather than separate JavaScript or native language backends.

```text
Restrict source
    ↓
Core WebAssembly
    ↓
host profile or generated adapter
    ├── native WASI runtime
    ├── Component Model host
    ├── browser host
    └── cloud or edge host
```

The current compiler uses WASI Preview 1 imports for basic program output. The
browser playground implements those imports with a small JavaScript bridge;
Restrict source still compiles only to Wasm. The long-term direction is to add
capability-oriented WASI APIs, stable composite-value lowering, WIT and
Component Model output, and generated Web or cloud adapters without adding a
JavaScript backend.

Native WASI runtimes can provide a JavaScript-free application environment.
Browsers do not currently expose the DOM directly to Wasm, so Web applications
still need a host adapter. The compiler keeps that adapter separate so a future
standard browser host interface can replace it without changing the language
backend. Docker and containerd are treated as ways to run or package WASI
artifacts, not as Restrict targets or ABIs.

See [WebAssembly Integration](docs/public/en/advanced/wasm.md) for the public
boundary and [WebAssembly Execution Strategy](docs/WASM_EXECUTION_STRATEGY.md)
for the accepted architecture direction.

## 🚀 Quick Start

The browser playground runs without an installation at the
[Restrict playground](https://ischca.github.io/restrict_lang/compiler/).

### From Source

```bash
git clone https://github.com/Ischca/restrict_lang
cd restrict_lang

# Build the compiler
mise exec -- cargo build --release

# Compile your first program
echo 'fun main: () -> Int32 = { 42 }' > hello.rl
./target/release/restrict_lang --emit wasm hello.rl

# Run the generated WebAssembly
wasmtime hello.wasm
```

For import-free compute modules, select the host-neutral benchmark target:

```bash
./target/release/restrict_lang --target wasm-core --emit wasm compute.rl
```

`wasm-core` rejects host I/O. The default `wasip1` profile supports the current
`print` and `println` surface.

## ✨ Features

- **🔒 Affine Type System**: Variables can be used at most once, preventing accidental resource duplication
- **🧠 Arena Memory Management**: No garbage collection, deterministic memory usage with arena allocation
- **🎯 Pattern Matching**: Exhaustive pattern matching with type safety for closed user enums, Option, Result, List, and Record types
- **🌟 Lambda Expressions**: First-class functions with closure capture and bidirectional type inference
- **🧭 Scoped Verb Clauses**: Higher-order verbs open typed focus scopes such as `values map { it + 1 }`
- **⚡ WebAssembly Target**: Compiles to efficient WebAssembly with WASI support for the current concrete ABI surface
- **📝 OSV Syntax**: Object-Subject-Verb syntax for natural function composition (traditional function calls not supported)
- **🧩 Forms**: Explicit `form` / `takes` contracts with static, monomorphized dispatch
- **💬 Comments**: Full support for single-line (`//`) and multi-line (`/* */`) comments
- **🎪 Spread Destructuring**: Extract multiple record fields with rest patterns `{ field1, field2, ...rest }`
- **⏰ Temporal Affine Types**: Experimental and excluded from the default v0.0.1 gate

## Release Design Boundaries

The v0.0.1 release includes a deliberately small user-defined enum and static
form surface while retaining the ABI boundaries below.

- Closed, non-generic, non-recursive user-defined `enum` declarations are
  supported. Variants have zero or one payload, constructors use qualified
  `Type::Variant` names in OSV order, patterns use the same qualified names,
  and matches must be exhaustive. `pub enum` is source-module metadata only:
  user enums have no host WebAssembly ABI.
- The v0.0.1 compiler supports non-generic, method-only `form`
  contracts, concrete record `takes` declarations, and `<T of A + B>` generic
  bounds. Dispatch is static and monomorphized. Associated types, generic or
  conditional adoptions, defaults, enum adoptions, and dynamic dispatch remain
  future work. The current `Container` behavior used by `map` and `filter`
  remains compiler-internal.
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
- `Result<T, CustomError>` works with a closed user enum as its error type, but
  ergonomic `?` propagation syntax is not implemented yet.

## 📖 Language Overview

### Hello World

```restrict
// hello.rl
fun main: () -> () = {
    "Hello, Restrict Language!" println
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
    "Result: " println;
    result println;
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
mut val counter = 0;
counter = counter + 1;
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
val direct = 21 double        // Single argument: value function
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

Line breaks are whitespace, so a direct OSV call may span lines. A literal or
other value that cannot be a verb naturally starts a new expression:

```restrict
"first" println
"second" println
```

Use `;` when the next callable-shaped expression must not extend the current
OSV chain. This most often appears when a named intermediate is declared and
then used by the block's final expression:

```restrict
val message = "ready";
message println
```

That semicolon is intentional when `val` stages a named intermediate before a
separate identifier-started expression. If the name is only needed inside a
higher-order transformation, open a scoped verb clause and name its lambda
input instead:

```restrict
values map { |value|
    value + 1
}
```

This keeps the transformation semicolon-free without making line breaks carry
statement semantics.

### Lambda Expressions and Closures

```restrict
fun apply_int: (f: Int32 -> Int32, value: Int32) -> Int32 = {
    value |> f
}

fun main: () -> Int32 = {
    (|x| x * 2, 21) apply_int
}
```

### Scoped Verb Clauses

A higher-order verb can open its final function parameter as a lexical scope.
The complete clause becomes the object of the next verb, preserving
left-to-right clause-level OSV flow.

```restrict
fun main: () -> Int32 = {
    val values = [1, 2, 3]
    val shifted = values map {
        it + 1
    }
    (shifted, 0) fold { |total, value|
        total + value
    }
}
```

An unheaded unary scope receives the contextual `it` binding. Use an explicit
lambda-style header such as `{ |value| ... }` when naming the focus or handling
multiple parameters.

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

### Forms and Display

Forms describe behavior without runtime vtables. A record adopts a form
explicitly, and generic functions state the required forms with `of`.

```restrict
pub form Labelled {
    fun label: (self: Self) -> String
}

record Badge {
    text: String
}

Badge takes Labelled {
    fun label: (self: Badge) -> String = {
        self.text
    }
}

fun read_label: <T of Labelled>(value: T) -> String = {
    value label
}
```

The compiler provides `Display` for `String`, all scalar types, and `()`. User
records opt in explicitly. `print` and `println` accept any value that takes
`Display` and are called directly in OSV position; they are not first-class
function values in this initial slice. `print_int` and `print_float` remain
available for compatibility.

```restrict
record Notice {
    text: String
}

Notice takes Display {
    fun display: (self: Notice) -> String = {
        self.text
    }
}

fun main: () -> () = {
    42 print
    " · " print
    Notice { text: "records too" } println
}
```

Passing a non-Copy record as `self` consumes it under the ordinary affine
rules. Forms with associated types, generic or conditional `takes`, default
methods, enum adoptions, and dynamic dispatch are not in this initial slice.

### Arena Memory Management

```restrict
// Arena context with scoped temporary heap allocation
fun process_batch: () -> Int32 = {
    with Arena { } {
        val big_list = [1, 2, 3, 4, 5];
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

- **[Quick Start](docs/public/en/getting-started/quick-start.md)** - Build and run a first v0.0.1 project
- **[Language Guide](docs/public/en/guide/README.md)** - Current release-facing syntax and design rules
- **[Release Surface](docs/public/en/reference/release-surface.md)** - Current v0.0.1 language and host ABI boundaries
- **[Examples](examples/)** - Sample programs and use cases

## 🏗️ Implementation Status

### ✅ Completed Features

- [x] Lexer with comment support
- [x] Parser with OSV syntax (traditional function calls not supported)
- [x] Type system with affine types
- [x] Lambda expressions with closures
- [x] Pattern matching (user enum, Option, Result, List, Record)
- [x] Spread destructuring with `...rest` syntax
- [x] WebAssembly code generation
- [x] Arena memory management
- [x] Bidirectional type inference
- [x] Function tables for indirect calls
- [x] Higher-order functions (`map`, `filter`, `fold`) with typed function values
- [x] Generics and parametric polymorphism inside Restrict programs
- [x] Result types with expected-type inference
- [x] Closed user-defined enums with qualified constructors and exhaustive matching
- [x] Type-directed `impl` method dispatch through grouped OSV calls
- [x] Method-only forms, concrete record adoptions, `of` bounds, and static form dispatch
- [x] Display-polymorphic `print` and `println`
- [x] Source import resolution through the CLI
- [x] Direct local Warder dependencies with manifest-bound namespaces and deterministic lock hashes
- [x] Affine checking across complex expressions, OSV calls, field access, and branching
- [x] Conditional expressions with chained/nested runtime coverage
- [x] Recursive functions with direct and mutual runtime coverage

### 🚧 In Progress

- [ ] Temporal Affine Types (TAT) outside the default v0.0.1 gate
- [ ] Generic, recursive, and host-ABI user enum support beyond the current closed enum slice
- [ ] Registry, Git, foreign-WASM, and transitive Warder dependency resolution
- [ ] Source-level import aliases, re-exports, and std aggregators
- [ ] Direct WebAssembly ABI for exported generic functions and host-visible record values
- [ ] Associated types, generic/conditional form adoptions, and default form methods

### 📋 Planned Features

- [ ] String interpolation
- [ ] Async/await support
- [ ] Ergonomic error propagation syntax
- [ ] Capability-oriented WASI standard library
- [ ] WIT and WebAssembly Component Model output
- [ ] Generated Web and cloud host adapters
- [ ] SIMD operations
- [ ] WebGPU host integration

### ⚠️ Current Boundaries

- Pattern guards and tuple patterns are future/design gaps; v0.0.1 covers
  Option, Result, List, Record, nested, and spread record patterns
- Source-file imports and manifest-bound namespaces for direct local Warder
  dependencies are implemented. Source `import ... as`, re-exports, std
  aggregators, and non-local or transitive package graphs remain future work
- String interpolation is not part of the v0.0.1 syntax; use concatenation today
- Closed user-defined enums are supported after v0.0.1, including use as the
  error type in `Result<T, CustomError>`. Generic and recursive user enums,
  direct host enum ABI, and `?` propagation syntax remain future work
- Source forms are intentionally narrow: non-generic method contracts and
  concrete non-generic record adoptions only. `takes` is not independently
  public, and associated types, generic/conditional adoptions, defaults,
  dynamic dispatch, and enum adoptions remain future work
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
Host Profile / Generated Adapter
    ↓
WebAssembly Runtime (WASI, Component host, browser, edge, etc.)
```

### Type System

- **Affine Types**: Each variable can be used at most once
- **Arena Allocation**: Memory management without garbage collection
- **Static Type Checking**: Catch errors at compile time
- **Type Inference**: Bidirectional type checking for lambdas

### WebAssembly Backend

- WebAssembly is the sole language backend
- The native compiler emits WAT; Warder also packages binary Wasm
- Current program output uses WASI Preview 1 for basic I/O operations
- Browser and cloud integration belongs in generated host adapters
- WIT, the Component Model, and composite host ABI are planned
- Function tables for lambda/closure calls
- Linear memory management with arenas

## 🧪 Testing

```bash
# Run fast local checks while developing
mise run test-fast

# Run default tests
mise run test

# Run slow release example CLI gates
mise run check

# Run the complete merge preflight in fail-fast order
mise run preflight

# Also rebuild and smoke-test the Pages artifact
mise run preflight-pages

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
git clone https://github.com/Ischca/restrict_lang
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

![GitHub Stars](https://img.shields.io/github/stars/Ischca/restrict_lang)
![GitHub Forks](https://img.shields.io/github/forks/Ischca/restrict_lang)
![GitHub Issues](https://img.shields.io/github/issues/Ischca/restrict_lang)
![License](https://img.shields.io/github/license/Ischca/restrict_lang)

---

**Restrict Language** - Making functional programming efficient and safe for WebAssembly 🚀
