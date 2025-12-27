# Temporal Affine Types: Minimal Design

## Core Insight

We don't need new syntax! Just:
1. **Lifetime parameters** on types (`'t`)
2. **Existing context system** for scoping
3. **Readable constraints** instead of mathematical symbols

## Simplified Syntax

### 1. **Just Type Parameters**
```restrict
// Temporal types look like generics with '
record File<'f> {
    handle: FileHandle
}

record Database<'db> {
    connection: Connection
}

record Transaction<'tx, 'db> {
    db: Database<'db>
    state: TxState
}
```

### 2. **Use Existing Context System**
```restrict
// No need for "with lifetime" - just use contexts!
context FileSystem {
    open: String -> File<'current>
    read: File<'current> -> String
}

fun processFile = filename: String {
    with FileSystem {
        val file = open(filename);
        val content = read(file);
        content
    }  // file cleaned up when context ends
}
```

### 3. **Lifetime Constraints - Natural Language**

Instead of mathematical symbols, use readable keywords:

#### Option A: `outlives`
```restrict
fun beginTransaction<'tx, 'db> = db: Database<'db> -> Transaction<'tx, 'db>
where 'db outlives 'tx {
    // database must outlive transaction
}
```

#### Option B: `within`
```restrict
fun beginTransaction<'tx, 'db> = db: Database<'db> -> Transaction<'tx, 'db>
where 'tx within 'db {
    // transaction within database lifetime
}
```

#### Option C: `lifetime ... extends ...`
```restrict
fun beginTransaction<'tx, 'db> = db: Database<'db> -> Transaction<'tx, 'db>
where lifetime 'db extends 'tx {
    // db lifetime extends beyond tx
}
```

#### Option D: Just use type constraints
```restrict
// Transaction type itself declares the constraint
record Transaction<'tx, 'db> where 'tx within 'db {
    db: Database<'db>
    state: TxState
}

// No need to repeat in function
fun beginTransaction<'tx, 'db> = db: Database<'db> -> Transaction<'tx, 'db> {
    // Constraint already in type definition
}
```

## Complete Examples

### 1. **File I/O (Simple)**
```restrict
record File<'f> {
    handle: FileHandle
}

fun copyFile = source: String dest: String {
    val input = fs.open(source);   // File<'1>
    val output = fs.create(dest);   // File<'2>
    
    input.read() |> output.write;
    
    // Both files cleaned up here
}
```

### 2. **Database Transactions (Complex)**
```restrict
record Database<'db> {
    conn: Connection
}

record Transaction<'tx, 'db> where 'tx within 'db {
    db: Database<'db>
    id: TransactionId
}

fun transferMoney = from: Account to: Account amount: Money {
    val db = Database.connect();
    val tx = db.beginTransaction();  // 'tx within 'db enforced
    
    tx.execute("UPDATE accounts SET balance = balance - ? WHERE id = ?", [amount, from]);
    tx.execute("UPDATE accounts SET balance = balance + ? WHERE id = ?", [amount, to]);
    
    tx.commit();
    // tx cleaned up, then db cleaned up
}
```

### 3. **With Contexts (Natural Scoping)**
```restrict
context Database<'db> {
    connect: String -> Connection<'db>
    query: String -> Result<'db>
}

fun getUsers = {
    with Database {
        val result = query("SELECT * FROM users");
        result.toList()
    }  // Database connection cleaned up
}
```

### 4. **Async Integration**
```restrict
// Temporal types work with async naturally
fun fetchDataAsync<'http> = urls: List<String> {
    with HttpClient<'http> {
        urls 
        |> map(|url| get(url))      // List<Future<Response<'http>>>
        |> Future.all               // Future<List<Response<'http>>>
        |> await
    }  // All responses cleaned up
}
```

## Why This Works Better

### 1. **No New Syntax**
- Just lifetime parameters like generics
- Reuse existing `with` blocks
- Natural language constraints

### 2. **Consistent with Restrict Philosophy**
- Contexts already manage resources
- Affine types already prevent misuse
- Just adding temporal dimension

### 3. **Easy to Learn**
```restrict
// Looks like normal generics
List<T>           // Generic over type
File<'f>          // Generic over lifetime
Map<K, V>         // Multiple type params
Transaction<'tx, 'db>  // Multiple lifetime params
```

### 4. **Progressive Complexity**
```restrict
// Start simple
val file = fs.open("data.txt");  // Lifetime inferred

// Add explicit lifetimes when needed
fun processFile<'f> = file: File<'f> {
    // ...
}

// Add constraints only when necessary
record Transaction<'tx, 'db> where 'tx within 'db {
    // ...
}
```

## Implementation Steps

### 1. **Extend Type System**
```rust
// Just add lifetime parameters to existing types
enum Type {
    Named(String),
    Generic(String, Vec<Type>, Vec<Lifetime>),  // Add lifetimes
    // ...
}
```

### 2. **Inference**
```rust
// Infer lifetimes like type parameters
fn infer_lifetime(expr: &Expr) -> Lifetime {
    match expr {
        Expr::Call("fs.open", _) => fresh_lifetime(),
        // ...
    }
}
```

### 3. **Cleanup Generation**
```rust
// Generate cleanup at scope boundaries
fn generate_cleanup(scope: &Scope) {
    for (var, lifetime) in scope.temporal_vars() {
        emit_cleanup(var);
    }
}
```

## Advantages

1. **Minimal syntax addition** - Just `'` for lifetimes
2. **Leverages existing features** - Contexts, affine types
3. **Natural constraints** - `within`, `outlives` instead of `⊆`
4. **Progressive disclosure** - Simple cases stay simple

This feels much more "Restrict-like" - what do you think?