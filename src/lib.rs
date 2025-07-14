//! # Restrict Language Compiler
//! 
//! A modern programming language for WebAssembly with affine types and OSV (Object-Subject-Verb) syntax.
//!
//! ## Overview
//! 
//! Restrict Language brings together unique features:
//! - **OSV (Object-Subject-Verb) word order** inspired by Japanese syntax
//! - **Affine type system** ensuring each value is used at most once
//! - **Prototype-based records** with `freeze` and `clone` operations
//! - **WebAssembly-first** design with no garbage collector
//! - **Pattern matching** with exhaustive checking
//! - **Generic functions** with monomorphization
//!
//! ## Quick Example
//!
//! ```rust,ignore
//! // OSV syntax with pipe operator
//! "Hello, World!" |> println;
//! 
//! // Affine types ensure memory safety
//! let data = getData();
//! data |> process;  // data is consumed
//! // data |> process;  // Error: already used
//! 
//! // Pattern matching
//! result |> match {
//!     Ok(value) => value |> handleSuccess,
//!     Err(error) => error |> handleError
//! };
//! 
//! // Generic functions
//! fn identity<T>(value: T) -> T {
//!     value
//! }
//! ```
//!
//! ## Architecture
//! 
//! The compiler follows a traditional pipeline:
//! 1. **Lexing** ([`lexer`]) - Source code → Tokens
//! 2. **Parsing** ([`parser`]) - Tokens → AST
//! 3. **Type Checking** ([`type_checker`]) - AST → Typed AST (with affine type validation)
//! 4. **Code Generation** ([`codegen`]) - Typed AST → WebAssembly
//!
//! ## Key Modules
//!
//! - [`lexer`] - Tokenization and lexical analysis using nom parser combinators
//! - [`ast`] - Abstract Syntax Tree definitions with support for generics
//! - [`parser`] - Parsing Restrict Language's OSV syntax
//! - [`type_checker`] - Type checking with affine types and generic inference
//! - [`codegen`] - WebAssembly code generation with monomorphization
//! - [`module`] - Module system for managing imports/exports
//! - [`lsp`] - Language Server Protocol implementation for IDE support

#![doc(html_logo_url = "https://restrict-lang.org/logo.svg")]
#![doc(html_favicon_url = "https://restrict-lang.org/favicon.ico")]
#![doc(html_playground_url = "https://play.restrict-lang.org")]

/// Lexical analysis module for tokenizing Restrict Language source code
pub mod lexer;

/// Abstract Syntax Tree module containing all AST node definitions
pub mod ast;

/// Parser module for converting tokens into an Abstract Syntax Tree
pub mod parser;

/// Type checking module implementing the affine type system
pub mod type_checker;

/// Code generation module for producing WebAssembly output
pub mod codegen;

/// Test framework for property-based testing
pub mod test_framework;

/// Debug visualizer for AST and type information
pub mod debug_visualizer;

/// Module system for managing imports and exports
pub mod module;

/// Development tools for debugging and analysis (non-WASM only)
#[cfg(not(target_arch = "wasm32"))]
pub mod dev_tools;

/// Language Server Protocol implementation for IDE integration (non-WASM only)
#[cfg(not(target_arch = "wasm32"))]
pub mod lsp;

/// WebAssembly bindings for browser integration
pub mod web;

// Re-exports for convenience
pub use lexer::*;
pub use ast::*;
pub use parser::*;
pub use type_checker::*;
pub use codegen::{WasmCodeGen, CodeGenError};

/// Legacy convenience function for tests
/// 
/// Generates WebAssembly text format from a parsed program.
/// 
/// # Example
/// ```rust,ignore
/// let program = parse("fun main = { 42 }").unwrap();
/// let wat = generate(&program).unwrap();
/// ```
pub fn generate(program: &Program) -> Result<String, CodeGenError> {
    let mut codegen = WasmCodeGen::new();
    codegen.generate(program)
}