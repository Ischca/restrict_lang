# Temporal Resources: Record vs Context vs Both

## The Core Question

Where should temporal resources (with lifetimes) belong?

1. **Record-based**: Resources are fields in records
2. **Context-based**: Resources are managed by contexts  
3. **Hybrid**: Both, depending on use case

## Option 1: Record-Based Ownership

Resources are struct fields with lifetime parameters:

```restrict
record Connection<'conn> {
    socket: Socket
    buffer: Buffer
}

record Database<'db> {
    connection: Connection<'db>
    config: Config
}

// Usage
fun processData = {
    val db = Database.connect("postgres://localhost");  // db: Database<'1>
    db.connection.query("SELECT * FROM users");
    // db.drop() automatically called
}
```

### Pros:
- Clear ownership model
- Composable (records can contain other records)
- Familiar to Rust developers
- Works with existing type system

### Cons:
- Lifetime parameters everywhere
- Complex for simple resources
- Manual propagation of lifetimes

## Option 2: Context-Based Management

Resources exist only within context scopes:

```restrict
context Database {
    // No fields, just operations
    query: String -> Result
    execute: String -> Unit
}

// Usage
with Database {
    query("SELECT * FROM users")
    execute("INSERT INTO logs ...")
}  // All resources cleaned up
```

### Pros:
- Simple syntax
- No lifetime annotations needed
- Natural scoping
- Matches existing `with Arena` pattern

### Cons:
- Can't pass resources between functions
- Limited composability
- All operations must be within context block

## Option 3: Hybrid Approach (Recommended)

Use contexts for resource acquisition, records for resource holding:

```restrict
// Records hold resources with lifetimes
record Connection<'conn> {
    socket: Socket
    id: ConnectionId
}

// Contexts manage resource lifecycle
context Database<'db> {
    // Context methods return via callbacks
    connect: (Connection<'db> -> R) -> R
}

// Extensions on records for operations
impl Connection<'conn> {
    query: String -> Result<'conn>
    execute: String -> Unit
}

// Usage combines both
with Database {
    Database.connect { conn ->    // conn: Connection<'db>
        conn.query("SELECT * FROM users")
    }
}
```

## Design Principles for Hybrid

### 1. **Contexts Create, Records Hold**

```restrict
// Context creates the resource
context FileSystem<'fs> {
    open: (String, File<'fs> -> R) -> R
}

// Record holds the resource
record File<'f> {
    handle: FileHandle
    path: String
}

// Record has methods
impl File<'f> {
    read: Unit -> String
    write: String -> Unit
}
```

### 2. **Lifetime Flows: Context → Record → Operations**

```restrict
with FileSystem {              // 'fs lifetime created
    FileSystem.open("data.txt") { file ->  // file: File<'fs>
        val content = file.read();         // operations on record
        file.write(content.uppercase());
    }
}
```

### 3. **Records Can Escape Within Context**

```restrict
// Records can be passed around within their lifetime
fun processFile<'f> = file: File<'f> {
    file.read() |> transform |> file.write
}

with FileSystem {
    FileSystem.open("input.txt") { file ->
        processFile(file)  // OK: 'f is within FileSystem context
    }
}
```

## Complete Example: Database with Transactions

```restrict
// Records define structure
record Connection<'conn> {
    socket: Socket
    config: Config
}

record Transaction<'tx, 'conn> where 'tx within 'conn {
    conn: Connection<'conn>
    id: TransactionId
    committed: Bool  // Mutable, for state tracking
}

// Context manages lifecycle
context Database<'db> {
    connect: (String, Connection<'db> -> R) -> R
}

// Operations on records
impl Connection<'conn> {
    beginTx: (Transaction<'tx, 'conn> -> R) -> R where 'tx within 'conn
    query: String -> Result<'conn>
}

impl Transaction<'tx, 'conn> {
    execute: String -> Unit
    
    // Affine: consumes self
    commit: Unit -> Unit {
        if !self.committed {
            __internal_commit(self.id);
            self.committed = true;
        }
        consume self;  // Can't use after commit
    }
    
    rollback: Unit -> Unit {
        if !self.committed {
            __internal_rollback(self.id);
        }
        consume self;
    }
}

// Usage
with Database {
    Database.connect("postgres://localhost") { conn ->
        conn.beginTx { tx ->
            tx.execute("UPDATE accounts SET balance = balance - 100 WHERE id = 1");
            tx.execute("UPDATE accounts SET balance = balance + 100 WHERE id = 2");
            
            if validateTransfer() {
                tx.commit();
            } else {
                tx.rollback();
            }
        }  // Auto-rollback if neither commit nor rollback called
    }
}
```

## Why Hybrid Works Best

### 1. **Separation of Concerns**
- **Contexts**: Resource lifecycle management
- **Records**: Resource state and operations
- **Lifetimes**: Connect them together

### 2. **Flexibility**
```restrict
// Simple case: just use context
with FileSystem {
    FileSystem.readFile("config.json") |> parseConfig
}

// Complex case: pass records around
with Database {
    Database.connect(url) { conn ->
        processBusinessLogic(conn)
        generateReport(conn)
    }
}
```

### 3. **Type Safety**
```restrict
// Can't escape lifetime
val leaked = with Database {
    Database.connect(url) { conn ->
        conn  // ERROR: Can't return Connection<'db> outside 'db
    }
}

// Can't use after consume
tx.commit();
tx.execute("...");  // ERROR: tx already consumed
```

## Implementation Guidelines

### Phase 1: Basic Records with Lifetimes
```restrict
record File<'f> {
    handle: FileHandle
}

fun openFile = path: String -> File<'?> {
    // Lifetime inference needed
}
```

### Phase 2: Context-Callback Pattern
```restrict
context FileSystem<'fs> {
    open: (String, File<'fs> -> R) -> R
}
```

### Phase 3: Full Integration
- Automatic drop generation
- Lifetime inference
- Nested lifetime checking

## Decision: Hybrid Approach

The hybrid approach gives us:
1. **Clear ownership** (records hold resources)
2. **Automatic management** (contexts control lifecycle)
3. **Flexibility** (can pass resources within lifetime)
4. **Safety** (can't leak or misuse resources)

This aligns with Restrict's philosophy: explicit where it matters, automatic where it's safe.