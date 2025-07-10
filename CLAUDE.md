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
4. **Type Checker** (TODO) - Implements affine type checking and phase checking
5. **Code Generator** (TODO) - Generates WASM output

## Language Features

- **OSV word order**: Object-Subject-Verb syntax (e.g., `obj subj.verb`)
- **Affine types**: Each binding can be referenced 0-1 times
- **Prototype-based records**: Use `clone` and `freeze` for inheritance
- **Context binding**: Resource management with `with` blocks
- **Pipe operators**: `|>` for immutable binding, `|>>` for mutable

## Important Notes

- The language compiles to WASM without GC
- Comments are not yet fully implemented in the lexer
- Binary operators for arithmetic are defined but not yet integrated into the parser