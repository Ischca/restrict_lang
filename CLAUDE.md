# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Commands

### Build the project
```
cargo build
```

### Run the compiler
```
cargo run --bin restrict_lang <source_file.rl>
```

### Run tests
```
cargo test
```

### Debug lexer
```
cargo run --bin debug_lex
```

## Architecture

The Restrict Language compiler is structured as follows:

1. **Lexer** (`src/lexer.rs`) - Tokenizes source code using nom parser combinators
2. **AST** (`src/ast.rs`) - Defines the Abstract Syntax Tree structures  
3. **Parser** (`src/parser.rs`) - Parses tokens into AST using nom
4. **Type Checker** (`src/type_checker.rs`) - Implements affine type checking with bidirectional type inference
5. **Code Generator** (`src/codegen.rs`) - Generates WebAssembly (WAT) output

## Language Features

- **OSV word order**: Object-Subject-Verb syntax (e.g., `obj subj.verb`)
- **Affine types**: Each binding can be referenced 0-1 times
- **Prototype-based records**: Use `clone` and `freeze` for inheritance
- **Context binding**: Resource management with `with` blocks
- **Pipe operators**: `|>` for immutable binding, `|>>` for mutable

## Compiler Development Principles

### 1. No Silent Fallbacks

**Never use default/fallback values when type information is missing or ambiguous.**

Bad:
```rust
// Don't do this - silently assumes Int32
} else {
    "Int32".to_string()
}
```

Good:
```rust
// Return an error when type cannot be determined
} else {
    return Err(CodeGenError::CannotInferType(
        format!("cannot infer return type for function '{}'", func.name)
    ));
}
```

### 2. Fail Early, Fail Loudly

- Type inference failures should be compile-time errors, not runtime surprises
- If information is missing, report it clearly to the user
- Error messages should indicate what was expected and what was found

### 3. Type Safety

- All expressions must have a determinable type at compile time
- The compiler should never guess or assume types
- When type annotation is optional, inference must be complete or fail explicitly

### 4. Error Message Quality

- Include source location (line, column) when possible
- Explain what the compiler expected vs. what it found
- Suggest possible fixes when applicable

### 5. Code Generation Invariants

- Never generate WASM code for expressions with unknown types
- All function signatures must be fully resolved before code generation
- Memory layout must be deterministic based on types

## Important Notes

- The language compiles to WASM without GC
- Comments are fully implemented: `//` for single-line and `/* */` for multi-line
- Binary operators for arithmetic are defined but not yet integrated into the parser