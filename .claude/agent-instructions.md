# Agent Instructions - MUST READ

## Critical Requirements for ALL Agents

**MANDATORY**: Before making ANY changes to the Restrict Language codebase:

1. **ALWAYS review the language specification** at `/LANGUAGE_SPECIFICATION.md`
2. **ALWAYS verify syntax rules**:
   - Use `val` for variable declarations (NOT `let`)
   - Use OSV (Object-Subject-Verb) word order
   - Field assignments use `:` (NOT `=`)
   - Newlines are whitespace; non-callable values naturally start a new
     expression, while `;` ends an expression before a callable-shaped value
     that would otherwise extend the same OSV chain
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

## WebAssembly Direction

- WebAssembly is Restrict's sole code-generation target.
- WASI, browser, cloud/edge, and container support are host profiles or
  generated adapters, not JavaScript or native language backends.
- Keep platform APIs behind explicit imports and capabilities. Do not add DOM,
  Cloudflare, Docker, or runtime-specific behavior to core language semantics.
- Current browser JavaScript is host glue around Wasm. Do not infer that a
  JavaScript backend exists or is required.
- Treat WIT, the Component Model, composite host ABI, broad WASI support, and
  direct browser DOM access as future work unless the release surface says
  otherwise.
- Review `/docs/WASM_EXECUTION_STRATEGY.md` before changing code generation,
  ABI lowering, host imports, runtime integration, or deployment packaging.

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
