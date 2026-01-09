# Temporal Affine Types: Design Guide

**Status**: Official Design Guide
**Syntax**: Tilde `~t` (Final)
**Last Updated**: 2025-12-27

This document provides comprehensive guidance on designing and using Temporal Affine Types (TAT) in Restrict Language.

---

## Table of Contents

1. [Core Concepts](#core-concepts)
2. [Syntax Overview](#syntax-overview)
3. [Resource Ownership Models](#resource-ownership-models)
4. [Context-Based API Design](#context-based-api-design)
5. [Naming Conventions](#naming-conventions)
6. [Design Patterns](#design-patterns)
7. [Best Practices](#best-practices)

---

## Core Concepts

### What Are Temporal Type Variables?

Temporal type variables (using `~` prefix) are **compile-time labels** that track *when* a resource is valid, not just *what type* it is.

```rust
// Regular type parameter: "File containing what type of data?"
record Container<T> { data: T }

// Temporal parameter: "File valid for how long?"
record File<~f> { handle: FileHandle }
```

### Mental Model: Types + Time

Think of temporal variables as type parameters that represent time:

| Category | Example | Meaning |
|----------|---------|---------|
| **Type variables** | `T`, `U`, `V` | "What type?" |
| **Temporal variables** | `~t`, `~u`, `~v` | "When valid?" |

```rust
// Combining both
record Cache<T, ~valid> {
    data: T              // What we're caching
    expiry: Time<~valid> // When it expires
}
```

### Key Principles

1. **Temporal variables are scope-bound** - They exist only within a specific block
2. **Automatic cleanup** - Resources are freed when their temporal scope ends
3. **Zero runtime cost** - All checks happen at compile time
4. **Composable** - Temporal types work with the existing type system

---

## Syntax Overview

### Official Syntax: Tilde `~`

After evaluating multiple alternatives (`'t`, `` `t ``, `@t`, etc.), **tilde `~`** was chosen as the final syntax.

#### Basic Declaration

```rust
// Temporal type parameter on a record
record File<~f> {
    handle: FileHandle
    path: String
}

// Multiple temporal parameters
record Transaction<~tx, ~db> where ~tx within ~db {
    db: Database<~db>
    txId: Int32
}
```

#### Function Signatures

```rust
// Function with temporal parameter
fun readFile<~io> = file: File<~io> -> String {
    file.path
}

// Temporal constraint
fun beginTransaction<~db, ~tx> = db: Database<~db> -> Transaction<~tx, ~db>
where ~tx within ~db {
    Transaction { db: db, txId: 42 }
}
```

#### Lifetime Blocks

```rust
// Explicit lifetime scope
with lifetime<~f> {
    val file = openFile("data.txt");  // file: File<~f>
    val content = file.read();
    content
}  // file automatically cleaned up here

// Nested lifetimes with constraints
with lifetime<~outer> {
    with lifetime<~inner> where ~inner within ~outer {
        // ~inner must end before ~outer
    }  // ~inner cleanup
}  // ~outer cleanup
```

#### Context Integration

```rust
// Context creates implicit temporal scope
context FileSystem<~fs> {
    open: (String, (File<~fs>) -> R) -> R
}

with FileSystem {
    FileSystem.open("data.txt") { file ->
        // file: File<~fs>
        file.read()
    }  // file cleaned up automatically
}
```

---

## Resource Ownership Models

There are three approaches to managing temporal resources. Choose based on your use case.

### Model 1: Record-Based Ownership

**When to use**: Resources with clear ownership, composable structures

```rust
record Connection<~conn> {
    socket: Socket
    buffer: Buffer
}

record Database<~db> {
    connection: Connection<~db>
    config: Config
}

// Direct ownership
fun processData = {
    val db = Database.connect("postgres://localhost");
    db.connection.query("SELECT * FROM users");
}  // db automatically dropped
```

**Pros:**
- ✅ Clear ownership model
- ✅ Composable (records contain records)
- ✅ Works naturally with type system
- ✅ Familiar to Rust developers

**Cons:**
- ⚠️ Lifetime parameters can proliferate
- ⚠️ More complex for simple use cases

### Model 2: Context-Based Management

**When to use**: Simple resource scoping, callback-style APIs

```rust
context Database {
    query: String -> Result
    execute: String -> Unit
}

// No lifetime annotations needed
with Database {
    query("SELECT * FROM users")
    execute("INSERT INTO logs ...")
}  // All resources cleaned up
```

**Pros:**
- ✅ Simple syntax
- ✅ No explicit lifetime annotations
- ✅ Natural scoping
- ✅ Matches existing `with` pattern

**Cons:**
- ⚠️ Can't pass resources between functions
- ⚠️ Limited composability
- ⚠️ Operations must stay in context block

### Model 3: Hybrid Approach (Recommended)

**When to use**: Most real-world scenarios

Combine contexts for resource acquisition with records for resource holding:

```rust
// Records hold resources with lifetimes
record Connection<~conn> {
    socket: Socket
}

record Transaction<~tx, ~conn> where ~tx within ~conn {
    connection: Connection<~conn>
    txId: Int32
}

// Contexts create and manage lifetimes
context DatabaseCtx<~db> {
    connect: ((Connection<~db>) -> R) -> R
}

// Usage: Context creates, records hold
with DatabaseCtx {
    DatabaseCtx.connect { conn ->       // conn: Connection<~db>
        conn.beginTx { tx ->            // tx: Transaction<~tx, ~db>
            tx.execute("UPDATE ...");
            tx.commit();
        }  // tx cleanup
    }  // conn cleanup
}
```

**Pros:**
- ✅ Best of both worlds
- ✅ Flexible and composable
- ✅ Natural resource hierarchies
- ✅ Clear ownership semantics

**Design principle**: **Context creates, record holds**

---

## Context-Based API Design

### Key Innovation: Callback Chains

Instead of returning resources (which could escape their scope), methods take callbacks:

```rust
context FileSystem<~fs> {
    // ❌ DON'T: Return value that could escape
    // open: String -> File<~fs>

    // ✅ DO: Take callback, ensure scoping
    open: (String, (File<~fs>) -> R) -> R
}
```

### Design Principles

#### 1. Context Creates Lifetime

```rust
with DatabaseCtx {    // Implicitly creates ~db lifetime
    // All resources in this block have ~db lifetime
}
```

#### 2. Methods Take Scope Callbacks

```rust
context DatabaseCtx<~db> {
    connect: ((Connection<~db>) -> R) -> R
}

// Resource never escapes callback
DatabaseCtx.connect { conn ->
    // Use conn here
}  // conn dropped automatically
```

#### 3. Nested Scopes = Nested Lifetimes

```rust
conn.beginTx { tx ->      // ~tx within ~db automatically
    // Transaction lifetime nested within connection
}
```

### Complete API Example

```rust
// Context definition
context DatabaseCtx<~db> {
    connect: ((Connection<~db>) -> R) -> R
}

// Extension methods on temporal types
impl Connection<~db> {
    beginTx: ((Transaction<~tx>) -> R) -> R where ~tx within ~db
    query: String -> Result<~db>
}

impl Transaction<~tx> {
    execute: String -> Unit
    commit: Self -> Unit    // Consumes self (affine)
    rollback: Self -> Unit  // Consumes self (affine)
}

// Usage
with DatabaseCtx {
    DatabaseCtx.connect { conn ->
        conn.beginTx { tx ->
            tx.execute("INSERT INTO users ...");
            tx.execute("INSERT INTO logs ...");
            tx.commit();
        }  // tx automatically committed or rolled back
    }  // conn automatically closed
}
```

### Advantages of Callback Pattern

1. **Safety**: Resources can't escape their scope
2. **Automatic cleanup**: No manual `close()` calls
3. **Natural nesting**: Mirrors resource hierarchies
4. **Type safety**: Compiler enforces lifetime constraints

---

## Naming Conventions

### Temporal Variable Names

After considering alternatives ("lifetime", "scope", "extent"), the recommendation is to use **descriptive lowercase names**:

```rust
// Generic temporal variables
~t, ~u, ~v

// Domain-specific temporal variables (preferred)
~conn          // Connection lifetime
~tx            // Transaction lifetime
~req           // Request lifetime
~sess          // Session lifetime
~fs            // File system lifetime
~io            // I/O operation lifetime
~http          // HTTP request lifetime
~db            // Database connection lifetime
```

### Speaking About Temporals

When discussing temporal types:

- "This function takes a File with temporal `~f`"
- "Temporal `~tx` must be within temporal `~db`"
- "The file's temporal scope ends here"

### Error Messages

```
Error: Temporal variable ~tx must be within ~db
Error: Cannot return value with temporal ~conn outside its scope
Error: Resource with temporal ~f has expired
Error: Temporal ~a does not outlive ~b
```

---

## Design Patterns

### Pattern 1: Simple Resource

**Use case**: Single resource, automatic cleanup

```rust
fun readConfig = {
    with lifetime<~f> {
        val file = openFile("config.json");
        file.read() |> parseJson
    }  // file auto-closed
}
```

### Pattern 2: Nested Resources

**Use case**: Resource hierarchy (connection → transaction)

```rust
with lifetime<~db> {
    val db = connectDatabase();

    with lifetime<~tx> where ~tx within ~db {
        val tx = db.beginTransaction();
        tx.execute("UPDATE ...");
        tx.commit();
    }  // tx cleanup before db
}  // db cleanup
```

### Pattern 3: Multiple Independent Resources

**Use case**: Multiple resources with same lifetime

```rust
with lifetime<~io> {
    val input = openFile("input.txt");
    val output = createFile("output.txt");

    input.read() |> process |> output.write;
}  // Both files closed together
```

### Pattern 4: Context-Based Pool

**Use case**: Resource pools with leasing

```rust
context ConnectionPool<~pool> {
    acquire: ((Connection<~lease>) -> R) -> R
    where ~lease within ~pool
}

with ConnectionPool {
    ConnectionPool.acquire { conn1 ->
        conn1.query("SELECT ...");
    }  // conn1 returned to pool

    ConnectionPool.acquire { conn2 ->
        conn2.query("INSERT ...");
    }  // conn2 returned to pool
}
```

---

## Best Practices

### ✅ DO

1. **Use descriptive temporal names**
   ```rust
   // ✅ Clear intent
   record Transaction<~tx, ~db> where ~tx within ~db

   // ❌ Generic and unclear
   record Transaction<~a, ~b> where ~a within ~b
   ```

2. **Prefer context-based APIs for simple cases**
   ```rust
   // ✅ Simple and safe
   with FileSystem {
       FileSystem.open("file.txt") { file ->
           file.read()
       }
   }

   // ❌ Unnecessary complexity
   val file = openFile<~f>("file.txt");
   defer close(file);
   ```

3. **Use record-based ownership for composition**
   ```rust
   // ✅ Clear ownership hierarchy
   record Session<~sess> {
       user: User
       token: Token<~sess>
   }
   ```

4. **Document temporal constraints**
   ```rust
   // ✅ Clear documentation
   // Transaction temporal must be within database temporal
   record Transaction<~tx, ~db> where ~tx within ~db {
       db: Database<~db>
       txId: Int32
   }
   ```

### ❌ DON'T

1. **Don't return temporal resources from contexts**
   ```rust
   // ❌ Resource escapes scope
   context FileSystem<~fs> {
       open: String -> File<~fs>  // WRONG
   }

   // ✅ Use callback instead
   context FileSystem<~fs> {
       open: (String, (File<~fs>) -> R) -> R  // CORRECT
   }
   ```

2. **Don't use generic names for domain-specific temporals**
   ```rust
   // ❌ Unclear
   record Connection<~t> { ... }

   // ✅ Descriptive
   record Connection<~conn> { ... }
   ```

3. **Don't nest too deeply**
   ```rust
   // ❌ Too complex
   with A {
       with B {
           with C {
               with D {
                   // Lost in nesting
               }
           }
       }
   }

   // ✅ Flatten or refactor
   fun processWithResources<~r> = {
       // Helper function reduces nesting
   }
   ```

---

## Migration from Old Syntax

If you encounter old syntax in documentation or code:

### Syntax Changes

| Old (Archived) | New (Current) | Example |
|---------------|---------------|---------|
| `'t` | `~t` | `File<~f>` not `File<'f>` |
| `'tx ⊆ 'db` | `~tx within ~db` | Use English keyword |
| `` `t `` | `~t` | Backtick syntax rejected |

### File Locations

- ✅ **Current docs**: `docs/TEMPORAL_*.md` (uses `~t`)
- 🗄️ **Archived docs**: `docs/archive/temporal-design-exploration/` (uses `'t` or `` `t ``)

---

## See Also

- **[TEMPORAL_TYPES_FINAL_DESIGN.md](TEMPORAL_TYPES_FINAL_DESIGN.md)** - Authoritative specification
- **[TEMPORAL_CONSTRAINT_RULES.md](TEMPORAL_CONSTRAINT_RULES.md)** - Formal constraint rules
- **[TEMPORAL_ASYNC_ROADMAP.md](TEMPORAL_ASYNC_ROADMAP.md)** - Implementation roadmap
- **[TAT_IMPLEMENTATION_STATUS.md](TAT_IMPLEMENTATION_STATUS.md)** - Current implementation status
- **[RESTRICT_LANG_EBNF.md](../RESTRICT_LANG_EBNF.md)** - Formal grammar specification

---

**Maintained by**: Restrict Language Core Team
**Questions?**: File an issue with tag `temporal-types`
