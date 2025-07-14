# Introduction

Welcome to **Restrict Language** - a modern programming language designed for WebAssembly with a unique blend of functional programming, affine types, and Japanese-inspired syntax.

## What is Restrict Language?

Restrict Language brings together innovative features rarely seen in a single language:

- **OSV (Object-Subject-Verb) word order** inspired by Japanese grammar
- **Affine type system** ensuring each value is used at most once
- **Prototype-based inheritance** with `freeze` and `clone` operations  
- **WebAssembly-first** design with no garbage collector
- **Pattern matching** with exhaustive checking
- **Context system** for dependency injection

## Your First Program

<!-- include: ../code-examples/hello-world.rl -->

This simple program demonstrates Restrict's distinctive OSV (Object-Subject-Verb) syntax. Instead of `println("Hello, World!")`, we write `"Hello, World!" |> println`, creating a natural flow of data from left to right.

## Why Restrict Language?

### Memory Safety Without GC

<!-- include: ../code-examples/affine-types.rl -->

### Elegant Data Flow

<!-- include: ../code-examples/osv-basic.rl lines: 2-8 -->

### WebAssembly Native

Restrict compiles directly to WebAssembly, providing:
- Near-native performance
- Small binary size
- No runtime overhead
- Seamless web integration

## Getting Started

Ready to dive in? Head to the [Installation Guide](./getting-started/installation.md) to set up Restrict Language on your system.

## Community

- GitHub: [github.com/restrict-lang/restrict_lang](https://github.com/restrict-lang/restrict_lang)
- Discord: [Join our community](https://discord.gg/restrict-lang)
- Forum: [discuss.restrict-lang.org](https://discuss.restrict-lang.org)

Welcome to the Restrict Language community! 🦀