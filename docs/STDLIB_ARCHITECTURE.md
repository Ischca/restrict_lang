# Standard Library Architecture for Restrict Language

## Overview

This document outlines the architecture and implementation strategy for Restrict Language's standard library, with a focus on backend web development capabilities.

## Core Design Decisions

### 1. Implementation Strategy: Hybrid Approach

We propose a **hybrid approach** combining both hardcoded and .rl implementations:

#### Hardcoded (in compiler):
- **Core primitives**: Memory allocation, type conversions, basic arithmetic
- **Runtime essentials**: Panic handling, assertions, core I/O
- **Performance-critical**: Functions that benefit from direct WAT generation
- **Bootstrap functions**: Minimal set needed to load .rl standard library

#### .rl Library Files:
- **High-level APIs**: HTTP, JSON, file operations, etc.
- **Complex logic**: Parsing, serialization, protocol implementations
- **User-extensible**: Allow community contributions
- **Versioned**: Can evolve independently of compiler

### 2. Backend Development Priority Stack

Priority order for backend web development:

1. **Foundation Layer** (P0)
   - WASI file I/O
   - Process environment (args, env vars, exit codes)
   - Basic networking (TCP sockets via WASI)
   - String manipulation utilities

2. **Data Layer** (P1)
   - JSON parsing and serialization
   - URL parsing and encoding
   - Base64 encoding/decoding
   - Regular expressions (basic)

3. **HTTP Layer** (P2)
   - HTTP server (using WASI sockets)
   - HTTP client
   - Request/Response abstractions
   - Middleware pattern support

4. **Application Layer** (P3)
   - Routing and path matching
   - Template rendering
   - Session management
   - Database drivers (PostgreSQL via sockets)

5. **Future Considerations** (P4)
   - Async/await runtime
   - Worker threads
   - Caching abstractions
   - Message queues

### 3. Runtime Target: WASI

Choose WASI (WebAssembly System Interface) as the primary target:

**Advantages:**
- Standard interface for system calls
- Existing runtime support (Wasmtime, Wasmer, WasmEdge)
- Growing ecosystem
- Security through capability-based model

**Limitations:**
- No built-in async I/O (yet)
- Limited to WASI preview 1 features initially
- Some features require runtime extensions

### 4. Module Organization

```
std/
├── core/              # Mostly .rl implementations
│   ├── string.rl      # String utilities
│   ├── option.rl      # Option type operations
│   ├── result.rl      # Result<T, E> for error handling
│   └── iter.rl        # Iterator patterns
│
├── io/                # Mix of hardcoded + .rl
│   ├── file.rl        # File I/O (wraps WASI)
│   ├── stdio.rl       # Console I/O
│   └── net.rl         # TCP/UDP networking
│
├── encoding/          # Pure .rl implementations
│   ├── json.rl        # JSON parser/serializer
│   ├── base64.rl      # Base64 encoding
│   ├── url.rl         # URL parsing
│   └── hex.rl         # Hex encoding
│
├── http/              # Pure .rl implementations
│   ├── server.rl      # HTTP server
│   ├── client.rl      # HTTP client
│   ├── request.rl     # Request type
│   ├── response.rl    # Response type
│   └── router.rl      # URL routing
│
└── sys/               # System interfaces
    ├── env.rl         # Environment variables
    ├── process.rl     # Process management
    └── time.rl        # Time operations
```

### 5. JSON Implementation Strategy

Since JSON is crucial for web backends, here's a specific approach:

```restrict
// std/encoding/json.rl

// JSON value type using tagged unions
export type JsonValue = 
    | Null
    | Bool(Boolean)
    | Number(Float64)
    | String(String)
    | Array(List<JsonValue>)
    | Object(List<(String, JsonValue)>)

// Type class for JSON serializable types
export context JsonSerializable {
    toJson: Self -> JsonValue
}

// Type class for JSON deserializable types
export context JsonDeserializable {
    fromJson: JsonValue -> Result<Self, JsonError>
}

// Derive macros (future feature)
// #[derive(JsonSerializable, JsonDeserializable)]
// record User { name: String, age: Int32 }
```

### 6. HTTP Server Example API

```restrict
// Example usage of future HTTP server API

import std.http.{Server, Request, Response, Router}
import std.encoding.json.{toJson, fromJson}

record User { id: Int32, name: String }

fun main = {
    val router = Router.new()
        |> Router.get("/users/:id", getUser)
        |> Router.post("/users", createUser)
        |> Router.static("/public", "./static")
    
    Server.new()
        |> Server.port(8080)
        |> Server.router(router)
        |> Server.start()
}

fun getUser = req: Request {
    val userId = req.params.get("id") |> unwrap |> parseInt
    
    // Fetch user from database...
    val user = User { id: userId, name: "Alice" }
    
    Response.ok()
        |> Response.json(user |> toJson)
}
```

### 7. Implementation Roadmap

#### Phase 1: Foundation (Week 1-2)
- [ ] Extend WASM import system for easier FFI
- [ ] Implement basic WASI wrappers in compiler
- [ ] Create std/ directory structure
- [ ] Implement core string utilities in .rl

#### Phase 2: Data Formats (Week 3-4)
- [ ] Implement JSON parser in .rl
- [ ] Add JSON serialization
- [ ] Create Result<T, E> type for error handling
- [ ] Add URL parsing

#### Phase 3: I/O & Networking (Week 5-6)
- [ ] Wrap WASI file operations
- [ ] Implement TCP socket wrapper
- [ ] Add buffered I/O
- [ ] Create stream abstractions

#### Phase 4: HTTP Implementation (Week 7-8)
- [ ] HTTP request/response parsing
- [ ] Basic HTTP server
- [ ] Routing system
- [ ] Middleware support

#### Phase 5: Polish & Examples (Week 9-10)
- [ ] Create example web applications
- [ ] Performance optimization
- [ ] Documentation
- [ ] Integration tests

### 8. Technical Challenges & Solutions

#### Challenge: Async I/O
**Solution**: Start with blocking I/O, design APIs to be async-ready. Consider green threads or callback-based approach initially.

#### Challenge: String Performance
**Solution**: Implement string operations in .rl but with careful memory management. Consider string interning for common strings.

#### Challenge: Error Handling
**Solution**: Use Result<T, E> pattern consistently. Implement Try trait for ergonomic error propagation.

#### Challenge: JSON Performance
**Solution**: Start with correctness, then optimize hot paths. Consider streaming parser for large documents.

### 9. Package Management Integration

The standard library should integrate with Warder:

```toml
# warder.toml for a web app
[dependencies]
std = "1.0"
std-http = "1.0"
std-json = "1.0"
```

This allows users to only include what they need, reducing binary size.

### 10. Migration Path for Existing Builtins

1. Keep existing hardcoded functions working
2. Gradually reimplement in .rl where appropriate
3. Mark hardcoded versions as deprecated
4. Remove after grace period

## Next Steps

1. Review and refine this architecture
2. Create proof-of-concept for JSON parser in .rl
3. Implement basic WASI file I/O wrappers
4. Build simple HTTP server prototype
5. Gather feedback from community

## Open Questions

1. Should we support async/await syntax, or use callbacks/promises?
2. How to handle platform-specific features (e.g., Unix sockets)?
3. Should standard library be a single package or multiple?
4. How to version standard library independently of compiler?
5. What's the best approach for compile-time code generation (derive macros)?