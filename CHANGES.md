# Recent Changes Summary

## Completed Changes

### 1. Removed Traditional Function Call Syntax
- Removed support for `func(args)` syntax from the parser
- Only OSV (Object-Subject-Verb) syntax is now supported
- `(21) double` works correctly as OSV syntax where `(21)` is a parenthesized expression

### 2. Option Type Syntax Reform  
- Added support for lowercase `none` in addition to `None` in the lexer
- Modified parser to support `None<T>` syntax for typed None values
- Implemented tagged union representation for Option types:
  - Tag 0 for None
  - Tag 1 for Some with value at offset 4
- Added `some` as a built-in function for creating Some values

### 3. Improved Compiler Error Reporting
- Enhanced error messages to show parsing context when parsing fails
- Shows 40 characters before and after the failure point
- Helps developers understand exactly where parsing stopped

### 4. Fixed Code Generation Issues
- Fixed `drop` instruction placement in match expression code generation
- Implemented `generate_block_as_expression` to properly handle match arms
- Match expressions now correctly use `(if (result i32))` format

## Test Status

### Passing Tests
- Basic compiler tests (arithmetic, function calls, conditionals, etc.)
- Core library tests (lexer, parser, type checker)
- `(21) double` syntax works correctly

### Failing Tests
- Pattern matching tests - written for features not yet fully implemented
- Method tests - method syntax not fully implemented
- Some Option type tests - need updating for new syntax

## Notes

- The "regression" with `(21) double` was a misunderstanding - it works correctly as OSV syntax
- Pattern matching tests were created but assume features like `Int?` type syntax which isn't implemented
- The compiler successfully parses and generates code for basic programs with the new syntax