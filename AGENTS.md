# AGENTS.md

This file provides guidance to Codex (Codex.ai/code) when working with code in this repository.

## CRITICAL: Always Reference Language Specification

**IMPORTANT**: Before making ANY changes to the codebase, ALWAYS consult the official language specification at `/LANGUAGE_SPECIFICATION.md`. This document is the single source of truth for:
- Syntax rules (val not let, OSV word order, etc.)
- Type system (affine types, temporal types)
- Operators and keywords
- Language philosophy and design decisions

When working with agents, ALWAYS include in your prompt:
"Please review the language specification at /LANGUAGE_SPECIFICATION.md and follow the instructions in /.Codex/agent-instructions.md before making changes. Note: /.Codex/agent-instructions.md may be absent; use .claude/agent-instructions.md if so."

## Agent Instructions

All agents MUST follow the instructions in `/.Codex/agent-instructions.md`, or `.claude/agent-instructions.md` when the Codex path is absent. These include:
- Language specification requirements
- Syntax rules (val not let, OSV word order, etc.)
- Commit message rules
- Project command usage

## Commands

**IMPORTANT**: This project uses `mise` for environment management. All cargo commands should be prefixed with `mise exec --` to ensure proper environment setup.

### Build the project
```
mise exec -- cargo build
# or
mise run build
```

### Run the compiler
```
mise exec -- cargo run --bin restrict_lang <source_file.rl>
```

### Run tests
```
mise exec -- cargo test
# or
mise run test
```

### Debug lexer
```
mise exec -- cargo run --bin debug_lex
```

### Available mise tasks
```
mise tasks  # List all available tasks
mise run test-one TEST=test_name  # Run specific test
mise run fmt  # Format code
mise run lint  # Run clippy
mise run ci  # Run full CI pipeline
```

## Architecture

The Restrict Language compiler is structured as follows:

1. **Lexer** (`src/lexer.rs`) - Tokenizes source code using nom parser combinators
2. **AST** (`src/ast.rs`) - Defines the Abstract Syntax Tree structures
3. **Parser** (`src/parser.rs`) - Parses tokens into AST using nom
4. **Type Checker** (`src/type_checker.rs`) - Implements affine type checking with bidirectional type inference
5. **Code Generator** (`src/codegen.rs`) - Generates WebAssembly (WAT) output

## Language Features

- **OSV word order**: Object-Subject-Verb syntax (e.g., `(args) function` or `value |> function`)
- **Affine types**: Each binding can be referenced 0-1 times
- **Prototype-based records**: Use `clone` and `freeze` for inheritance
- **Context binding**: Resource management with `with` blocks
- **Pipe operator**: `|>` for OSV function composition

## Important Notes

- The language compiles to WASM without GC
- Comments are fully implemented: `//` for single-line and `/* */` for multi-line
- Binary operators for arithmetic are defined but not yet integrated into the parser

## Commit Message Rules

**IMPORTANT**: Follow these rules for ALL git commits:

1. **NO emoji** in commit messages
2. **NO "Generated with Codex"** or similar AI signatures
3. **NO Co-Authored-By** for AI tools
4. Use conventional commit format: `type: subject`
5. Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`
6. Keep subject under 50 characters
7. Use imperative mood ("Add" not "Added")
8. No period at end of subject line

Example:
```
fix: Restore WebAssembly code generation pipeline

All 8 WAT generation tests were failing due to parser not handling
function declarations correctly. Fixed by supporting both complex
and simple syntax styles.
```
