# Agent Instructions - MUST READ

## Critical Requirements for ALL Agents

**MANDATORY**: Before making ANY changes to the Restrict Language codebase:

1. **ALWAYS review the language specification** at `/LANGUAGE_SPECIFICATION.md`
2. **ALWAYS verify syntax rules**:
   - Use `val` for variable declarations (NOT `let`)
   - Use OSV (Object-Subject-Verb) word order
   - Field assignments use `:` (NOT `=`)
   - No semicolons after top-level declarations
3. **ALWAYS follow the type system rules**:
   - Affine types (use at most once)
   - Temporal types for resource management
   - Copy semantics only for basic types

## Language Specification Quick Reference

### Variable Declaration
```restrict
val x = 42          // Immutable
mut val count = 42  // Mutable
```

### Record Syntax
```restrict
record Point {
    x: Int32,     // Use colon, not equals
    y: Int32
}

val p = Point { x: 10, y: 20 }
```

### Function Syntax
```restrict
fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

fun increment: (x: Int32) -> Int32 = {
    x + 1
}

// OSV style only: arguments come before the function name.
(5, 10) add
5 |> increment
```

Traditional calls such as `add(5, 10)` or `increment(5)` are not valid Restrict
syntax.

### Spread Patterns
```restrict
value match {
    User { name, email, ...rest } => { (name, email, rest) summarize_user }
    _ => { "unknown" }
}
```

## Project Commands

All commands use `mise`:
```sh
mise exec -- cargo build
mise exec -- cargo test
mise exec -- cargo run --bin restrict_lang <file.rl>
```

## When Uncertain

If you're uncertain about ANY syntax or semantic rule, ALWAYS:
1. Check `/LANGUAGE_SPECIFICATION.md` first
2. Look at working examples in `/examples/` directory
3. Verify with existing tests

## Commit Message Rules

- NO emoji
- NO "Generated with Claude" signatures
- Use conventional commit format: `type: subject`
- Types: `feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`
