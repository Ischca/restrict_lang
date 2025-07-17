# Temporal Affine Types: Unified Syntax Design

## Core Concept

Temporal affine types combine:
1. **Lifetime parameters** (`'t`) on types
2. **Lifetime scopes** for explicit lifetime management
3. **Automatic resource cleanup** at lifetime boundaries

## Syntax Elements

### 1. **Lifetime Parameters on Types**
```restrict
// Record with lifetime parameter
record Database<'db> {
    conn: Connection
}

// Function accepting temporal types
fun runQuery<'db> = db: Database<'db> query: String {
    db.conn.execute(query)
}
```

### 2. **Lifetime Scopes (Optional)**
```restrict
// Explicit lifetime scope
with lifetime<'db> {
    val db = Database.connect();
    // db has lifetime 'db
}  // cleanup at scope end

// Implicit lifetime (inferred)
fun processFile = filename: String {
    val file = fs.open(filename);  // lifetime inferred
    file.read()
}  // cleanup at function end
```

### 3. **Lifetime Relationships**
```restrict
with lifetime<'db> {
    val db = Database.connect();
    
    with lifetime<'tx> where 'tx ⊆ 'db {
        val tx = db.beginTransaction();
        // tx must not outlive db
    }  // tx cleanup
}  // db cleanup
```

## Two Complementary Approaches

### Approach A: **Type-Driven (Implicit)**
```restrict
// Types carry lifetime information
record File<'f> {
    handle: FileHandle
}

// Lifetime inferred from usage
fun copyFile = source: String dest: String {
    val src = fs.open(source);    // src: File<'1>
    val dst = fs.create(dest);     // dst: File<'2>
    
    src.read() |> dst.write;
    
    // Compiler infers cleanup points
}
```

### Approach B: **Scope-Driven (Explicit)**
```restrict
// Explicit lifetime management
fun copyFile = source: String dest: String {
    with lifetime<'io> {
        val src = fs.open(source);   // src: File<'io>
        val dst = fs.create(dest);    // dst: File<'io>
        
        src.read() |> dst.write;
    }  // Both cleaned up here
}
```

## Unified Model: Both Are Valid

### Simple Cases: Type-Driven
```restrict
// For simple resource management, let the compiler infer
fun readConfig = {
    val file = fs.open("config.json");  // File<'inferred>
    file.read() |> parseJson
}  // file cleaned up automatically
```

### Complex Cases: Scope-Driven
```restrict
// For complex lifetime relationships, be explicit
with lifetime<'server> {
    val server = Server.start(8080);
    
    with lifetime<'req> where 'req ⊆ 'server {
        server.accept() |> spawn(|conn| {
            with lifetime<'conn> where 'conn ⊆ 'req {
                handleConnection(conn);
            }
        });
    }
}
```

## Complete Example: Database with Transactions

```restrict
// Type definitions
record Database<'db> {
    conn: Connection
    config: DbConfig
}

record Transaction<'tx, 'db> where 'tx ⊆ 'db {
    db: Database<'db>
    state: TxState
}

// Implicit lifetime approach
fun transferMoney = from: AccountId to: AccountId amount: Money {
    val db = Database.connect("postgres://localhost");
    val tx = db.beginTransaction();
    
    tx.debit(from, amount);
    tx.credit(to, amount);
    
    if tx.validate() {
        tx.commit();
    } else {
        tx.rollback();
    }
    // db automatically closed after tx
}

// Explicit lifetime approach  
fun transferMoneyExplicit = from: AccountId to: AccountId amount: Money {
    with lifetime<'db> {
        val db = Database.connect("postgres://localhost");
        
        with lifetime<'tx> where 'tx ⊆ 'db {
            val tx = db.beginTransaction();
            
            tx.debit(from, amount);
            tx.credit(to, amount);
            
            if tx.validate() {
                tx.commit();
            } else {
                tx.rollback();
            }
        }  // tx cleaned up if not committed/rolled back
    }  // db cleaned up
}
```

## OSV Integration

```restrict
// Temporal types work naturally with OSV
with lifetime<'io> {
    val file = ("data.txt") fs.open;         // file: File<'io>
    val processed = (file) read 
                    |> parse 
                    |> transform;
    
    (processed, "output.txt") fs.write;      // New file also has 'io
}  // All files cleaned up
```

## When to Use Each Approach

### Use Type-Driven (Implicit) When:
- Simple resource management
- Single resource or independent resources  
- Lifetime is function-scoped
- Want minimal syntax

### Use Scope-Driven (Explicit) When:
- Complex lifetime relationships
- Multiple related resources
- Need precise control over cleanup order
- Want to document resource boundaries

## Implementation Insights

### 1. **Parser Changes**
```rust
// Add lifetime parameters to type parser
Type::Generic(name, params, lifetimes)

// Add 'with lifetime' block parsing
Expr::WithLifetime { 
    lifetime: LifetimeParam,
    constraints: Vec<LifetimeConstraint>,
    body: Block 
}
```

### 2. **Type System**
```rust
enum TypedType {
    // Existing variants...
    Temporal {
        base: Box<TypedType>,
        lifetime: LifetimeId,
    }
}

struct LifetimeConstraint {
    sub: LifetimeId,    // 'a
    sup: LifetimeId,    // 'b where 'a ⊆ 'b
}
```

### 3. **Inference Algorithm**
```rust
// Infer lifetimes when not explicit
fn infer_lifetime(expr: &Expr) -> LifetimeId {
    match expr {
        // Resource creation gets fresh lifetime
        Expr::Call(f) if is_resource_creator(f) => fresh_lifetime(),
        
        // Propagate lifetimes through expressions
        Expr::Pipe(e1, e2) => unify_lifetimes(
            infer_lifetime(e1), 
            infer_lifetime(e2)
        ),
        
        // ...
    }
}
```

## Benefits of This Design

1. **Flexibility**: Use implicit or explicit as needed
2. **Gradual Adoption**: Start simple, add complexity when needed  
3. **Type Safety**: Lifetime errors caught at compile time
4. **Zero Cost**: No runtime overhead
5. **Composable**: Functions can accept/return temporal types

## Open Questions

1. Should `with lifetime` blocks be expressions that return values?
2. How to handle lifetime parameters in type aliases?
3. Should we support lifetime elision rules like Rust?
4. Can lifetimes be "upgraded" (extended)?
5. How to integrate with async/await?