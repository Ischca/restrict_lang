# Introduction

Welcome to **Restrict Language** — a modern programming language designed for WebAssembly with an affine type system that ensures memory safety and optimal performance.

## What is Restrict Language?

Restrict Language is a statically-typed, compiled language that brings together unique features:

- **OSV (Object-Subject-Verb) word order** inspired by Japanese syntax
- **Affine type system** ensuring each value is used at most once
- **Prototype-based records** with `freeze` and `clone` operations
- **WebAssembly-first** design with no garbage collector
- **Interoperability** through WebAssembly Component Model and WIT

## Core Philosophy

### 名前1つ = 実体1つ (One Name = One Entity)

In Restrict Language, every binding represents a single, unique entity. This principle drives our affine type system, ensuring predictable resource management and preventing common programming errors like use-after-move and data races.

```restrict
// Each value has a single owner
let x = 42
let y = x  // x is moved to y
// x is no longer accessible here
```

### Memory Safety Without GC

By leveraging affine types and explicit memory management, Restrict Language achieves memory safety without the overhead of garbage collection. This makes it ideal for:

- High-performance WebAssembly applications
- Resource-constrained environments
- Real-time systems
- Blockchain and smart contracts

## Key Features

### 1. OSV Syntax

Inspired by Japanese grammar, Restrict Language uses Object-Subject-Verb ordering:

```restrict
// Traditional: subject.verb(object)
// Restrict: object subject.verb

"Hello, World!" |> println
list |> iter |> map(x => x * 2)
```

### 2. Affine Types

Each value can be referenced at most once, preventing aliasing bugs:

```restrict
fn consume(x: String) {
    x |> println
    // x is consumed here
}

let msg = "Hello"
msg |> consume
// msg cannot be used here
```

### 3. Prototype-Based Inheritance

Instead of classes, Restrict uses prototypes with explicit cloning and freezing:

```restrict
let base_car = {
    wheels: 4,
    drive: fn() { "Driving..." |> println }
}

// Create a new car by cloning
let my_car = base_car |> clone
my_car.color = "red"

// Freeze to prevent modifications
let frozen_car = my_car |> freeze
```

### 4. WebAssembly Integration

Restrict compiles directly to WebAssembly with first-class support for:

- WASI (WebAssembly System Interface)
- Component Model
- WIT (WebAssembly Interface Types)
- Cross-language interoperability

## Getting Started

Ready to dive in? Start with:

1. [Installation](./getting-started/installation.md) - Set up your development environment
2. [Hello World](./getting-started/hello-world.md) - Write your first Restrict program
3. [Warder Package Manager](./getting-started/warder.md) - Learn about package management

## Example Program

Here's a taste of Restrict Language:

```restrict
// Define a function with affine types
fn greet(name: String) -> String {
    let greeting = "Hello, " + name + "!"
    greeting  // Return ownership
}

// Main entry point
fn main() {
    let name = "World"
    name |> greet |> println
    
    // Working with lists
    [1, 2, 3, 4, 5]
        |> map(x => x * x)
        |> filter(x => x > 10)
        |> fold(0, (acc, x) => acc + x)
        |> println
}
```

## Why Restrict Language?

- **Performance**: Zero-cost abstractions and no GC overhead
- **Safety**: Affine types prevent common memory bugs at compile time
- **Simplicity**: Clear syntax inspired by functional programming
- **Interoperability**: Seamless integration with existing WebAssembly ecosystem
- **Modern Tooling**: Built-in package manager, LSP support, and VS Code extension

Join us in building the future of WebAssembly programming!