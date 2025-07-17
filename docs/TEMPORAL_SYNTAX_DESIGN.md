# Temporal Affine Types: Syntax Design Considerations

## Overview

This document explores different syntax options for temporal affine types in Restrict Language, considering readability, consistency with existing features, and implementation complexity.

## Core Concepts to Express

1. **Lifetime scopes** - Blocks where temporal resources are valid
2. **Temporal type annotations** - Types with lifetime parameters
3. **Lifetime relationships** - Constraints between lifetimes
4. **Automatic cleanup** - Resource management at scope boundaries

## Syntax Options

### 1. **Lifetime Scope Syntax**

#### Option A: `with lifetime` (Rust-inspired)
```restrict
with lifetime<'t> {
    val file = fs.open("data.txt");
    // file has lifetime 't
}  // automatic cleanup
```

#### Option B: `temporal` block (Novel)
```restrict
temporal<'t> {
    val file = fs.open("data.txt");
    // file has lifetime 't
}
```

#### Option C: `scope` block (Simple)
```restrict
scope {
    val file = fs.open("data.txt");
    // automatic cleanup, anonymous lifetime
}
```

#### Option D: `using` block (C#-inspired)
```restrict
using {
    val file = fs.open("data.txt");
    // automatic cleanup
}
```

#### Option E: `resource` block (Explicit)
```restrict
resource<'t> {
    val file = fs.open("data.txt");
    // file has lifetime 't
}
```

### 2. **Temporal Type Annotations**

#### Option A: Generic-style parameters
```restrict
temporal<'t> record Connection {
    socket: Socket
}

fun handleRequest = conn: Connection<'t> {
    // ...
}
```

#### Option B: Attribute-style annotations
```restrict
@temporal('t)
record Connection {
    socket: Socket
}

fun handleRequest = conn: @temporal('t) Connection {
    // ...
}
```

#### Option C: Type modifier
```restrict
record Connection lifetime<'t> {
    socket: Socket
}

fun handleRequest = conn: Connection lifetime<'t> {
    // ...
}
```

#### Option D: Inline lifetime
```restrict
record Connection {
    socket: Socket
} with lifetime<'t>

fun handleRequest = conn: Connection with lifetime<'t> {
    // ...
}
```

### 3. **Lifetime Relationships**

#### Option A: Where clauses (Rust-style)
```restrict
fun processData<'a, 'b> = data: Data<'a> buffer: Buffer<'b>
where 'a ⊆ 'b {
    // 'a is contained in 'b
}
```

#### Option B: Constraint syntax
```restrict
fun processData<'a: 'b, 'b> = data: Data<'a> buffer: Buffer<'b> {
    // 'a is contained in 'b
}
```

#### Option C: Explicit relationships
```restrict
fun processData = data: Data<'a> buffer: Buffer<'b>
requires 'a ⊆ 'b {
    // explicit requirement
}
```

#### Option D: Natural language
```restrict
fun processData = data: Data<'a> buffer: Buffer<'b>
where 'a within 'b {
    // more readable
}
```

### 4. **Anonymous Lifetimes**

#### Option A: Inferred (no annotation)
```restrict
scope {
    val file = fs.open("data.txt");
    // lifetime inferred
}
```

#### Option B: Underscore placeholder
```restrict
temporal<'_> {
    val file = fs.open("data.txt");
    // anonymous lifetime
}
```

#### Option C: Auto keyword
```restrict
temporal<auto> {
    val file = fs.open("data.txt");
    // automatic lifetime
}
```

## OSV Syntax Integration

### Current OSV Patterns
```restrict
// Current: Object-Subject-Verb
val result = (data) process;
val user = (userId) fetchUser;
```

### Temporal OSV Extensions

#### Option A: Temporal subjects
```restrict
temporal<'t> {
    val file = ("data.txt") fs.open;
    val content = (file) read;  // file has lifetime 't
}
```

#### Option B: Temporal objects
```restrict
scope {
    val content = ("data.txt"@'t) fs.open |> read;
    // object has explicit lifetime
}
```

#### Option C: Temporal verbs
```restrict
scope {
    val file = ("data.txt") fs.open@temporal;
    // verb creates temporal resource
}
```

## Integration with Existing Features

### 1. **Context System Integration**

#### Option A: Temporal contexts
```restrict
context FileSystem<'t> {
    open: String -> File<'t>
    read: File<'t> -> String
}

with FileSystem<'t> {
    val file = "data.txt" open;
    val content = file read;
}
```

#### Option B: Lifetime-aware contexts
```restrict
context FileSystem {
    open: String -> File<'current>
    read: File<'current> -> String
}

temporal<'t> {
    with FileSystem {
        val file = "data.txt" open;
        val content = file read;
    }
}
```

### 2. **Pipe Operator Integration**

#### Option A: Temporal pipes
```restrict
temporal<'t> {
    "data.txt" 
    |> fs.open
    |> read
    |> process
}  // automatic cleanup
```

#### Option B: Lifetime-aware pipes
```restrict
scope {
    "data.txt" 
    |>@ fs.open    // @-pipe creates temporal resource
    |> read
    |> process
}
```

## Recommended Syntax

### 1. **Lifetime Scopes: `scope` block**
```restrict
// Simple and clear
scope {
    val file = fs.open("data.txt");
    // automatic cleanup
}

// With explicit lifetime when needed
scope<'t> {
    val file = fs.open("data.txt");
    // file has lifetime 't
}
```

**Rationale:**
- Short and clear
- Consistent with existing block syntax
- Optional lifetime parameter for advanced use
- Emphasizes resource scoping

### 2. **Temporal Types: Generic-style**
```restrict
record Connection<'t> {
    socket: Socket
}

fun handleRequest = conn: Connection<'t> {
    // ...
}
```

**Rationale:**
- Consistent with existing generic syntax
- Familiar to developers
- Clear lifetime parameter

### 3. **Lifetime Relationships: Where clauses**
```restrict
fun processData<'a, 'b> = data: Data<'a> buffer: Buffer<'b>
where 'a ⊆ 'b {
    // ...
}
```

**Rationale:**
- Mathematical notation is precise
- Separates constraints from main signature
- Extensible for other constraints

### 4. **Anonymous Lifetimes: Inferred**
```restrict
scope {
    val file = fs.open("data.txt");
    // lifetime automatically inferred
}
```

**Rationale:**
- Reduces cognitive load
- Good defaults for common cases
- Explicit annotation when needed

## Complete Example

```restrict
// File processing with temporal types
record FileProcessor<'t> {
    input: File<'t>
    output: File<'t>
    buffer: Array<u8, 1024>
}

fun processFile = inputPath: String outputPath: String {
    scope<'files> {
        val processor = FileProcessor {
            input: (inputPath) fs.open,
            output: (outputPath) fs.create,
            buffer: Array.new()
        };
        
        scope<'chunk> where 'chunk ⊆ 'files {
            loop {
                val chunk = (processor.input) read(1024);
                if chunk.isEmpty() { break; }
                
                val processed = (chunk) transform;
                (processed) processor.output.write;
            }
        }
    }  // All files automatically closed
}

// Async integration
fun processFileAsync = inputPath: String outputPath: String {
    scope<'files> {
        with Async {
            val processor = FileProcessor {
                input: (inputPath) fs.open,
                output: (outputPath) fs.create,
                buffer: Array.new()
            };
            
            val future = (processor) processInBackground;
            future await;
        }
    }  // Files cleaned up even after async completion
}
```

## Implementation Considerations

### 1. **Parser Changes**
- Add `scope` keyword and block parsing
- Extend type system for lifetime parameters
- Add `where` clause parsing

### 2. **Type Checker Changes**
- Lifetime inference algorithm
- Lifetime relationship checking
- Scope boundary validation

### 3. **Code Generation**
- Automatic cleanup insertion
- WASM resource management
- Exception safety

## Open Questions

1. Should lifetime parameters be mandatory or optional?
2. How to handle lifetime errors in a user-friendly way?
3. Should we support lifetime polymorphism?
4. How to integrate with foreign function interfaces?
5. Should cleanup be customizable or always automatic?

## Next Steps

1. Implement basic `scope` block parsing
2. Add simple lifetime inference
3. Create proof-of-concept with file operations
4. Gather feedback and iterate on syntax
5. Extend to full temporal type system