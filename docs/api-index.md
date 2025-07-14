# Restrict Language API Documentation

This is the entry point for the Restrict Language API documentation. The documentation is generated using `rustdoc` and provides detailed information about all public modules, types, and functions.

## Generating API Documentation

To generate the API documentation locally:

```bash
# Generate documentation
cargo doc --all-features --no-deps

# Generate and open in browser
cargo doc --all-features --no-deps --open

# Generate with private items (for contributors)
cargo doc --all-features --no-deps --document-private-items
```

## Online Documentation

The latest API documentation is available at:
- [https://docs.restrict-lang.org/api/](https://docs.restrict-lang.org/api/)

## Core Modules

### Compiler Pipeline

- **[restrict_lang::lexer](./restrict_lang/lexer/index.html)** - Tokenization and lexical analysis
- **[restrict_lang::parser](./restrict_lang/parser/index.html)** - Parsing OSV syntax into AST
- **[restrict_lang::ast](./restrict_lang/ast/index.html)** - Abstract Syntax Tree definitions
- **[restrict_lang::type_checker](./restrict_lang/type_checker/index.html)** - Affine type system implementation
- **[restrict_lang::codegen](./restrict_lang/codegen/index.html)** - WebAssembly code generation

### Language Features

- **[restrict_lang::stdlib](./restrict_lang/stdlib/index.html)** - Standard library definitions
- **[restrict_lang::module](./restrict_lang/module/index.html)** - Module system and imports

### Developer Tools

- **[restrict_lang::lsp](./restrict_lang/lsp/index.html)** - Language Server Protocol implementation
- **[restrict_lang::debug_visualizer](./restrict_lang/debug_visualizer/index.html)** - AST visualization tools
- **[restrict_lang::test_framework](./restrict_lang/test_framework/index.html)** - Testing utilities

## Key Types

### AST Nodes

- [`Program`](./restrict_lang/ast/struct.Program.html) - Root AST node
- [`Expr`](./restrict_lang/ast/enum.Expr.html) - Expression types
- [`Pattern`](./restrict_lang/ast/enum.Pattern.html) - Pattern matching constructs
- [`Type`](./restrict_lang/ast/enum.Type.html) - Type representations

### Errors

- [`TypeError`](./restrict_lang/type_checker/enum.TypeError.html) - Type checking errors
- [`CodeGenError`](./restrict_lang/codegen/enum.CodeGenError.html) - Code generation errors
- [`ParseError`](./restrict_lang/parser/enum.ParseError.html) - Parsing errors

## Example Usage

```rust
use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};

// Parse source code
let source = r#"
    fn main() {
        "Hello, World!" |> println;
    }
"#;

let program = parse_program(source)?;

// Type check
let mut checker = TypeChecker::new();
let typed_program = checker.check_program(&program)?;

// Generate WebAssembly
let mut codegen = WasmCodeGen::new();
let wat_code = codegen.generate(&typed_program)?;
```

## Contributing

When adding new public APIs:

1. Add comprehensive doc comments with examples
2. Use `#[doc(hidden)]` for implementation details
3. Include doctests for examples
4. Run `cargo test --doc` to verify doctests

## Documentation Style Guide

### Module Documentation

```rust
//! # Module Name
//! 
//! Brief description of the module's purpose.
//! 
//! ## Overview
//! 
//! More detailed explanation...
//! 
//! ## Example
//! 
//! ```rust
//! // Example code
//! ```
```

### Function Documentation

```rust
/// Brief description of the function.
/// 
/// More detailed explanation if needed.
/// 
/// # Arguments
/// 
/// * `arg1` - Description of first argument
/// * `arg2` - Description of second argument
/// 
/// # Returns
/// 
/// Description of return value
/// 
/// # Errors
/// 
/// Description of possible errors
/// 
/// # Example
/// 
/// ```rust
/// let result = function(arg1, arg2)?;
/// ```
pub fn function(arg1: Type1, arg2: Type2) -> Result<ReturnType, Error> {
    // Implementation
}
```

## Related Documentation

- [Language Guide](./guide/) - User-facing language documentation
- [Getting Started](./getting-started/) - Tutorials and examples
- [Reference](./reference/) - Language reference

## License

The API documentation is available under the same license as Restrict Language itself.