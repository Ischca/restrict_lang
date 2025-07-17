# Temporal Affine Types: Rethinking the Design

## Core Misunderstanding

I was conflating two separate concepts:
1. **Temporal types** - Types that have temporal bounds (type system feature)
2. **Scope blocks** - Syntactic constructs for delimiting code regions

Temporal affine types should be about **types**, not blocks!

## What Temporal Affine Types Really Are

### Traditional Affine Types
```restrict
val file = fs.open("data.txt");  // file: File
file.read();  // file consumed
// file.read();  // ERROR: already used
```

### Temporal Affine Types
```restrict
val file = fs.open("data.txt");  // file: File<'t>
file.read();  // OK: can use multiple times within lifetime 't
file.write("hello");  // OK: still within lifetime 't
// ... but when 't ends, file is automatically cleaned up
```

## The Real Innovation

The key insight: **Lifetimes are inferred from resource acquisition patterns**, not explicit blocks!

### Example 1: Automatic Lifetime Inference
```restrict
fun processFile = filename: String {
    val file = fs.open(filename);  // file: File<'1>
    val content = file.read();      // OK within '1
    content.process();
    // Compiler infers: '1 ends here, insert cleanup
}
```

### Example 2: Lifetime Extension
```restrict
fun getFileContent = filename: String {
    val file = fs.open(filename);  // file: File<'1>
    val content = file.read();      // content depends on '1
    content  // ERROR: cannot return content that depends on '1
}

// Fixed version:
fun processWithFile<'t> = filename: String processor: (File<'t>) -> Result {
    val file = fs.open(filename);  // file: File<'t>
    processor(file)  // processor can use file multiple times
    // cleanup happens after processor returns
}
```

### Example 3: Nested Lifetimes
```restrict
fun copyFile = source: String dest: String {
    val input = fs.open(source);    // input: File<'1>
    val output = fs.create(dest);    // output: File<'2>
    
    val content = input.read();      // OK
    output.write(content);           // OK
    
    // Compiler infers: '1 and '2 both end here
    // Generates: input.close(); output.close();
}
```

## Integration with Existing Restrict Features

### 1. **With Blocks (Resource Contexts)**
```restrict
// Current: Context provides resources
with Arena {
    val data = allocate(1024);
    // Arena manages memory
}

// With temporal types: Context provides temporal resources
with Database<'db> {
    val conn = connect();  // conn: Connection<'db>
    // Connection lifetime tied to context
}
```

### 2. **Affine Types + Temporal = Best of Both**
```restrict
// Traditional affine: one-time use
val token = auth.createToken();
api.call(token);  // token consumed

// Temporal affine: multiple uses within lifetime
val conn = db.connect();  // conn: Connection<'t>
conn.query("SELECT ...");  // OK
conn.query("INSERT ...");  // OK
conn.close();  // Explicit close consumes conn
// Or automatic cleanup at end of 't
```

### 3. **OSV Syntax Remains Clean**
```restrict
fun fetchData = url: String {
    val response = (url) http.get;  // response: Response<'t>
    val data = (response) parseJson;  // OK within 't
    val processed = (data) transform; // OK within 't
    processed
    // cleanup happens here
}
```

## Real-World Examples

### 1. **Database Transaction**
```restrict
fun transferMoney = from: Account to: Account amount: Money {
    val tx = db.beginTransaction();  // tx: Transaction<'t>
    
    (from, amount) tx.debit;  // Can use tx multiple times
    (to, amount) tx.credit;    // Still OK
    
    if (from.balance >= 0) {
        tx.commit();  // Explicit commit
    } else {
        tx.rollback();  // Or explicit rollback
    }
    // If neither called, automatic rollback at end of 't
}
```

### 2. **Network Connection**
```restrict
fun handleClient = client: TcpStream<'conn> {
    loop {
        val request = client.read();  // OK: multiple reads
        if request.isNone() { break; }
        
        val response = (request) process;
        client.write(response);  // OK: multiple writes
    }
    // client automatically closed when 'conn ends
}
```

### 3. **File Processing Pipeline**
```restrict
fun pipeline = input: String output: String {
    val source = fs.open(input);     // source: File<'1>
    val dest = fs.create(output);     // dest: File<'2>
    val buffer = Array.new(1024);     // buffer: Array<u8, 1024>
    
    loop {
        val bytes = source.read(buffer);  // OK
        if bytes == 0 { break; }
        dest.write(buffer, bytes);        // OK
    }
    // Both files closed automatically
}
```

## Implementation Strategy

### 1. **Type System Extension**
```rust
// In TypedType enum
enum TypedType {
    // ... existing variants ...
    Temporal {
        base_type: Box<TypedType>,
        lifetime: LifetimeId,
    }
}
```

### 2. **Lifetime Inference**
```rust
// During type checking
fn infer_lifetime(&mut self, expr: &Expr) -> LifetimeId {
    match expr {
        Expr::Call(CallExpr { function: "fs.open", .. }) => {
            // Resource acquisition creates new lifetime
            self.create_lifetime()
        }
        // ... other patterns ...
    }
}
```

### 3. **Cleanup Generation**
```rust
// During code generation
fn generate_cleanup(&mut self, lifetime: LifetimeId) {
    for resource in self.resources_with_lifetime(lifetime) {
        // Generate cleanup code
        self.emit_cleanup(resource);
    }
}
```

## Advantages Over Block-Based Approach

1. **No new syntax needed** - Works with existing language
2. **Automatic inference** - Compiler does the work
3. **Composable** - Functions can accept/return temporal types
4. **Backward compatible** - Existing code still works
5. **Type-safe** - Lifetime errors caught at compile time

## The Key Innovation

**Temporal affine types = Affine types + Automatic cleanup + Multiple uses within lifetime**

This is not about syntax blocks, but about extending the type system to track resource lifetimes and generate cleanup code automatically.

## Questions to Resolve

1. How to indicate which functions create temporal resources?
2. Should cleanup be customizable (destructors)?
3. How to handle early returns and exceptions?
4. Can lifetimes be explicitly extended?
5. How to show lifetime errors clearly?