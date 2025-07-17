# Lifetime Parameter Naming in Restrict Language

## What is `'t`?

The `'t` syntax represents a **lifetime parameter** - a compile-time label that tracks how long a resource is valid.

## Naming Options

### 1. **"Lifetime"** (Rust's choice)
```restrict
record File<'lifetime> { ... }
// "File with lifetime 'lifetime"
```
- ✅ Accurate and technical
- ❌ Long and abstract
- ❌ Sounds like runtime duration

### 2. **"Scope"**
```restrict
record File<'scope> { ... }
// "File with scope 'scope"
```
- ✅ Familiar programming concept
- ✅ Implies bounded region
- ❌ Might confuse with lexical scope

### 3. **"Extent"**
```restrict
record File<'extent> { ... }
// "File with extent 'extent"
```
- ✅ Suggests temporal bounds
- ✅ Less overloaded term
- ❌ Less common in programming

### 4. **"Span"**
```restrict
record File<'span> { ... }
// "File with span 'span"
```
- ✅ Short and clear
- ✅ Implies duration
- ✅ Not overloaded
- ❌ Might suggest time measurement

### 5. **"Phase"**
```restrict
record File<'phase> { ... }
// "File with phase 'phase"
```
- ✅ Suggests lifecycle stage
- ✅ Unique to Restrict
- ❌ Might imply discrete states

### 6. **"Era"**
```restrict
record File<'era> { ... }
// "File with era 'era"
```
- ✅ Temporal connotation
- ✅ Short and memorable
- ❌ Might be too poetic

### 7. **"Temporal"** or "Temp"
```restrict
record File<'temp> { ... }
// "File with temporal 'temp"
```
- ✅ Directly references temporal types
- ✅ Clear meaning
- ❌ 'temp' might suggest temporary

### 8. **"Bound"**
```restrict
record File<'bound> { ... }
// "File with bound 'bound"
```
- ✅ Suggests constraints
- ✅ Mathematical precision
- ❌ Might be confused with type bounds

## Recommendation: **"Span"**

I recommend using **"span"** for the following reasons:

1. **Clear Meaning**: A span clearly suggests a duration or extent
2. **Not Overloaded**: Unlike "scope" or "lifetime", it's not heavily used elsewhere
3. **Natural Usage**: 
   - "This resource has span 'a"
   - "Span 'tx is within span 'db"
   - "The file's span ends here"
4. **Short and Memorable**: Easy to say and write

## Usage Examples

### Documentation
```restrict
// This function takes a Connection with span 'conn
fun query<'conn> = conn: Connection<'conn> sql: String -> Result<'conn>

// Transaction span must be within database span
record Transaction<'tx, 'db> where 'tx within 'db { ... }
```

### Error Messages
```
Error: Cannot return value with span 'conn outside its defining context
Error: Span 'tx must be within span 'db
Error: Resource with span 'a cannot outlive span 'b
```

### Conventions
```restrict
// Common span names
'a, 'b, 'c     // Generic spans
'conn          // Connection span  
'tx            // Transaction span
'req           // Request span
'sess          // Session span
'ctx           // Context span
```

## Alternative: Context-Specific Terms

We could also use different terms in different contexts:

- **For I/O**: "session" (e.g., `File<'session>`)
- **For Memory**: "arena" (e.g., `Buffer<'arena>`)
- **For Network**: "connection" (e.g., `Socket<'connection>`)
- **For Database**: "transaction" (e.g., `Query<'transaction>`)

But this might be more confusing than helpful.

## Final Decision

**"Span"** is the recommended term for lifetime parameters in Restrict Language.

```restrict
// Read as: "File with span 'f"
record File<'f> {
    handle: FileHandle
}

// Read as: "span 'tx within span 'db"
record Transaction<'tx, 'db> where 'tx within 'db {
    conn: Connection<'db>
}

// In conversation: "The file's span ends when the context closes"
with FileSystem {
    FileSystem.open("data.txt") { file ->
        // file has span bound to FileSystem context
    }
}
```