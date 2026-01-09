# Temporal Types Design Exploration Archive

This directory contains historical design documents from the development of Restrict Language's Temporal Affine Types (TAT) feature.

## Purpose

These documents represent the **design exploration phase** where multiple syntax options and implementation approaches were considered. They are preserved for:

1. **Historical reference** - Understanding the design rationale
2. **Alternative approaches** - Reference for future design decisions
3. **Educational value** - Learning from the design process

## Status: ARCHIVED

⚠️ **These documents use outdated syntax and are not authoritative.**

The final design decision was to use **tilde `~` syntax** for temporal type variables.

## Files in This Archive

### Syntax Exploration

- **TEMPORAL_BACKTICK_SYNTAX.md** - Exploration of backtick `` `t `` syntax
  - **Status**: Rejected in favor of `~t`
  - **Reason**: Poor visibility, markdown conflicts

- **TEMPORAL_SYNTAX_ALTERNATIVES.md** - Comprehensive comparison of 10 syntax options
  - **Alternatives considered**: `~t`, `` `t ``, `@t`, `$t`, `#t`, `%t`, `!t`, `_t`, lowercase-only, keywords
  - **Final decision**: Tilde `~t` chosen for visibility and wave metaphor

- **TEMPORAL_SYNTAX_DESIGN.md** - Earlier design iteration
  - **Syntax used**: `'t` (single quote, like Rust)
  - **Issue**: Conflicts with character literals (`'a'`)
  - **Status**: Superseded by tilde syntax

### Design Philosophy

- **TEMPORAL_AFFINE_TYPES.md** - Comprehensive TAT overview
  - **Syntax used**: `'t` (outdated)
  - **Value**: Excellent conceptual explanations and examples
  - **Note**: Content is valuable but syntax needs updating to `~t`

- **TEMPORAL_TYPES_RETHINK.md** - Philosophical rethinking
  - **Key insight**: Temporal types should be about types, not just blocks
  - **Status**: Valuable conceptual foundation but uses `'t` syntax

- **TEMPORAL_MINIMAL_DESIGN.md** - Minimalist approach
  - **Philosophy**: Minimal new syntax, progressive complexity
  - **Status**: Good design principles but uses `'t` syntax

### Naming Conventions

- **LIFETIME_NAMING_V2.md** - Second iteration of naming exploration
  - **Alternatives**: "life", "valid", "span", "lifetime"
  - **Status**: Incomplete, contains non-English content
  - **Note**: See `docs/LIFETIME_NAMING.md` for first iteration

## What to Use Instead

### Current Authoritative Documentation

For up-to-date TAT documentation, refer to:

1. **[TEMPORAL_TYPES_FINAL_DESIGN.md](../../TEMPORAL_TYPES_FINAL_DESIGN.md)** - Official specification
2. **[TEMPORAL_CONSTRAINT_RULES.md](../../TEMPORAL_CONSTRAINT_RULES.md)** - Formal constraint rules
3. **[TEMPORAL_ASYNC_ROADMAP.md](../../TEMPORAL_ASYNC_ROADMAP.md)** - Implementation roadmap
4. **[TEMPORAL_ASYNC_THEORY.md](../../TEMPORAL_ASYNC_THEORY.md)** - Theoretical foundation
5. **[TAT_IMPLEMENTATION_STATUS.md](../../TAT_IMPLEMENTATION_STATUS.md)** - Current implementation status

### Correct Syntax

```rust
// ✅ CORRECT: Use tilde ~ for temporal type variables
record File<~f> {
    handle: FileHandle
}

record Transaction<~tx, ~db> where ~tx within ~db {
    db: Database<~db>
    txId: Int32
}

// ❌ INCORRECT: Do not use single quote '
record File<'f> { ... }  // Old syntax, archived

// ❌ INCORRECT: Do not use backtick `
record File<`f> { ... }  // Rejected alternative
```

## Design Timeline

1. **Phase 1**: Initial exploration (files in this archive)
   - Evaluated syntax alternatives
   - Explored design philosophies
   - Considered Rust-like `'t` syntax

2. **Phase 2**: Syntax decision
   - Identified conflict with character literals
   - Evaluated 10+ alternatives
   - Selected tilde `~t` for final design

3. **Phase 3**: Implementation (current)
   - Lexer, parser, AST use `~t` syntax
   - Type checker implements temporal constraints
   - Code generation in progress

## Key Lessons Learned

### Why Not `'t` (Rust-style)?
- Character literals use single quotes: `'a'`, `'x'`
- Parser ambiguity between `'t` (temporal) and `'t'` (char)
- Would require complex lookahead

### Why `~t` (Tilde)?
- ✅ No conflicts with existing syntax
- ✅ Visually distinct from type parameters (`T`)
- ✅ Wave metaphor suggests temporality
- ✅ Single character, easy to type
- ✅ No markdown/documentation conflicts

### Design Principles Preserved
- Temporal types integrate with existing type system
- Both implicit (type-driven) and explicit (scope-driven) approaches supported
- Zero runtime overhead (compile-time only)
- Natural OSV syntax integration

## Contributing

If you're working on temporal types or related features:

1. **DO**: Read the current authoritative docs (listed above)
2. **DO**: Use `~t` syntax in all examples and code
3. **DO**: Refer to this archive for historical context only
4. **DON'T**: Use `'t` or `` `t `` syntax in new code/docs

## Questions?

For questions about temporal types, see:
- Implementation status: `docs/TAT_IMPLEMENTATION_STATUS.md`
- Current specification: `docs/TEMPORAL_TYPES_FINAL_DESIGN.md`
- GitHub issues: Tag with `temporal-types`

---

**Archive Date**: 2025-12-27
**Archived By**: Claude Code documentation cleanup
**Reason**: Syntax migration from `'t` to `~t`
