# Future Consideration: Three-Layer Scope System

## Overview

This document outlines a potential future enhancement to Restrict Language's type system: the addition of spatial and capability scope layers to complement the existing temporal scope system.

## Current State

Restrict Language currently implements a **temporal scope system** with hierarchical lifetime management:

```rust
with lifetime<~outer> {
    with lifetime<~inner> where ~inner within ~outer {
        // Resources in ~inner are automatically cleaned up before ~outer
    }
}
```

This system is inspired by Kotlin's coroutine scopes and provides structured resource management through temporal constraints.

## Proposed Additional Scope Layers

### 1. Spatial Scope Layer (`@`)

**Purpose**: Control where values are allocated in memory

**Proposed Syntax**:
```rust
record SecureData<~t, @location> {
    data: String
}

with spatial<@secure_heap> {
    // Values allocated in secure memory region
    val sensitive = SecureData { data: "encrypted" };
}
```

**Potential Built-in Spatial Scopes**:
- `@heap` - Standard heap allocation
- `@stack` - Stack allocation (when possible)
- `@cache` - Cache-optimized allocation
- `@secure` - Security-sensitive allocation
- `@gpu` - GPU memory allocation

### 2. Capability Scope Layer (`%`)

**Purpose**: Control what operations are permitted on values

**Proposed Syntax**:
```rust
record ProtectedResource<~t, %cap> {
    data: String
}

with capability<%read_only> {
    // Only read operations allowed
    val resource = ProtectedResource { data: "immutable" };
    resource.data  // OK: reading
    // resource.data = "new"  // ERROR: write not permitted
}
```

**Potential Built-in Capabilities**:
- `%read` - Read-only access
- `%write` - Write access
- `%execute` - Execution permission
- `%send` - Can be sent across threads
- `%sync` - Can be shared across threads

## Integration Example

All three layers could work together:

```rust
fun process_secure_data<~session>() = {
    with lifetime<~session> {
        with spatial<@secure_heap> {
            with capability<%read_write> {
                // Temporal: Lives for session duration
                // Spatial: Allocated in secure heap
                // Capability: Full read/write access
                val data = SecureData { content: "sensitive" };
                process(data)
            }
        }
    }
}
```

## Benefits

1. **Fine-grained Resource Control**: Different aspects of resource management separated into orthogonal concerns
2. **Security Boundaries**: Explicit capability restrictions at compile time
3. **Performance Optimization**: Spatial hints for memory allocation strategies
4. **Clear Intent**: Code explicitly states temporal, spatial, and capability requirements

## Implementation Considerations

### Parser Changes
- Add `@` prefix for spatial parameters
- Add `%` prefix for capability parameters
- Extend `with` statement to support all three scope types

### Type System Changes
- Extend type parameters to include spatial and capability parameters
- Add constraint checking for each layer
- Ensure orthogonality between layers

### Code Generation
- Map spatial hints to WASM memory instructions
- Implement capability checks at compile time
- Optimize based on scope constraints

## Relationship to Existing Features

- **Affine Types**: Capability scopes could enforce affine constraints
- **Prototype System**: Spatial scopes could optimize prototype cloning
- **Pattern Matching**: Capabilities could restrict pattern matching operations

## Open Questions

1. Should spatial and capability parameters be optional or required?
2. How do these scopes interact with generic type parameters?
3. Can scopes be inherited through prototype chains?
4. Should we allow custom user-defined scopes?
5. How do these scopes interact with the module system?

## Prior Art

- **Rust**: Lifetime system (temporal only)
- **Linear Haskell**: Linear types (capability-like)
- **Cyclone**: Region-based memory management (spatial)
- **Effect Systems**: Capability-based effect tracking
- **Kotlin Coroutines**: Structured concurrency (temporal)

## Decision Status

**Status**: 🔍 Under Consideration

This is a future enhancement proposal. The current focus remains on:
1. Stabilizing the temporal scope system
2. Completing the affine type implementation
3. Improving error messages and developer experience

The three-layer scope system may be revisited once the core language features are mature and there is clear demand for this level of control.

## Related Documents

- `/tests/test_three_layer_scope_edge_cases.rs` - Edge case tests exploring the concept
- `LANGUAGE_SPECIFICATION.md` - Current language specification
- `src/type_checker.rs` - Current temporal scope implementation