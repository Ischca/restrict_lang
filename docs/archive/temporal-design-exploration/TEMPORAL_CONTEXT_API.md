# Temporal Affine Types: Context-Based Chain API

## Core Innovation

Combine **Context + Lifetime + Resource** into a natural chain API that feels like Restrict Language.

```restrict
with DatabaseCtx {                       // ① 'db lifetime implicitly created
    DatabaseCtx.connect { conn ->        // ② conn: Conn<'db>
        
        conn.beginTx { tx ->             // ③ tx: Tx<'tx>, 'tx within 'db
            tx businessLogic             // ④ Just use it naturally
        }                                // ⑤ tx.drop() automatic
        
    }                                    // ⑥ conn.drop() automatic
}                                        // ⑦ 'db scope ends
```

## Key Design Principles

### 1. **Context Creates Lifetime**
```restrict
with DatabaseCtx {    // Implicitly creates 'db lifetime
    // All resources in this block have 'db lifetime
}
```

### 2. **Methods Take Scope Callbacks**
```restrict
context DatabaseCtx<'db> {
    connect: (Conn<'db> -> R) -> R
}

// Instead of returning values, methods take callbacks
// This ensures resources never escape their lifetime
```

### 3. **Nested Scopes = Nested Lifetimes**
```restrict
conn.beginTx { tx ->      // 'tx within 'db automatically
    // Transaction lifetime is nested within connection lifetime
}
```

## Complete API Design

### Context Definition
```restrict
context DatabaseCtx<'db> {
    // Methods return through callbacks, not values
    connect: (Conn<'db> -> R) -> R
}

// Extension methods on temporal types
impl Conn<'db> {
    beginTx: (Tx<'tx> -> R) -> R where 'tx within 'db
    query: String -> Result<'db>
}

impl Tx<'tx> {
    execute: String -> Unit
    commit: Self -> Unit    // Consumes self (affine)
    rollback: Self -> Unit  // Consumes self (affine)
}
```

### Usage Patterns

#### Simple Query
```restrict
with DatabaseCtx {
    DatabaseCtx.connect { conn ->
        conn.query("SELECT * FROM users")
    }
}
```

#### Transaction
```restrict
with DatabaseCtx {
    DatabaseCtx.connect { conn ->
        conn.beginTx { tx ->
            tx.execute("INSERT INTO logs VALUES (...)")
            tx.commit()
        }
    }
}
```

#### Error Handling
```restrict
with DatabaseCtx {
    DatabaseCtx.connect { conn ->
        conn.beginTx { tx ->
            try {
                updateAccounts(tx)
                tx.commit()
            } catch e {
                tx.rollback()
                throw e
            }
        }
    }
}
```

## OSV Integration

The chain API works naturally with OSV syntax:

```restrict
with DatabaseCtx {
    DatabaseCtx.connect { conn ->
        ("users") conn.table 
        |> filter(|u| u.active)
        |> map(|u| u.email)
        |> collect
    }
}
```

## Implementation Strategy

### 1. **Compiler Transformation**

The compiler transforms callback-style into resource management:

```restrict
// User writes:
DatabaseCtx.connect { conn ->
    conn.query("SELECT 1")
}

// Compiler generates:
{
    val conn = DatabaseCtx.__internal_connect();  // Conn<'db>
    try {
        conn.query("SELECT 1")
    } finally {
        conn.drop()
    }
}
```

### 2. **Lifetime Inference**

```restrict
// Context block creates parent lifetime
with DatabaseCtx {              // 'db created here
    
    // Method calls infer child lifetimes
    DatabaseCtx.connect { conn ->  // conn: Conn<'db>
        
        // Nested calls create nested lifetimes
        conn.beginTx { tx ->       // tx: Tx<'tx> where 'tx within 'db
            // ...
        }
    }
}
```

### 3. **Type Safety Guarantees**

```restrict
// ❌ Cannot escape lifetime
with DatabaseCtx {
    val escaped = DatabaseCtx.connect { conn ->
        conn  // ERROR: Cannot return Conn<'db> outside 'db
    }
}

// ❌ Cannot use after consume
conn.beginTx { tx ->
    tx.commit()
    tx.execute("...")  // ERROR: tx already consumed
}

// ✅ Automatic cleanup on all paths
conn.beginTx { tx ->
    if condition {
        tx.commit()
        return early  // tx.drop() still called
    }
    tx.rollback()
}  // Guaranteed cleanup
```

## Advanced Patterns

### Async Integration
```restrict
with AsyncDatabaseCtx {
    AsyncDatabaseCtx.connect { conn ->
        conn.beginTxAsync { tx ->
            val result = tx.queryAsync("SELECT ...") await;
            tx.commit()
            result
        }
    }
}
```

### Resource Pooling
```restrict
with ConnectionPool<'pool> {
    ConnectionPool.acquire { conn ->
        // conn: Conn<'lease> where 'lease within 'pool
        conn.query("SELECT ...")
    }  // Connection returned to pool
}
```

### Nested Contexts
```restrict
with DatabaseCtx {
    DatabaseCtx.connect { conn ->
        with CacheCtx {
            CacheCtx.get(key) || {
                val result = conn.query("SELECT ...");
                CacheCtx.put(key, result);
                result
            }
        }
    }
}
```

## Benefits

1. **Zero New Syntax** - Just contexts and closures
2. **Natural Chaining** - Fits Restrict's style perfectly
3. **Automatic Safety** - Can't leak or double-free
4. **Composable** - Contexts can nest and combine
5. **Exception Safe** - Cleanup on all exit paths

## Comparison with Other Approaches

### Traditional Approach
```restrict
// Manual resource management
val conn = Database.connect();
try {
    val tx = conn.beginTransaction();
    try {
        // work
        tx.commit();
    } finally {
        if (!tx.isCommitted()) {
            tx.rollback();
        }
    }
} finally {
    conn.close();
}
```

### Our Approach
```restrict
// Automatic and safe
with DatabaseCtx {
    DatabaseCtx.connect { conn ->
        conn.beginTx { tx ->
            // work
            tx.commit()
        }
    }
}
```

## Implementation Phases

### Phase 1: Basic Context-Lifetime Binding
- Contexts implicitly create lifetimes
- Simple resource cleanup

### Phase 2: Callback-Style Methods  
- Implement scope callback pattern
- Compiler transformation

### Phase 3: Nested Lifetime Inference
- Automatic 'tx within 'db constraints
- Full type safety

This design perfectly captures the essence of Restrict Language while providing bulletproof resource management!