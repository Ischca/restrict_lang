# Temporal Types + Async: Implementation Roadmap

## Vision

Combine temporal type variables with async programming to create a unique, safe concurrency model where:
- Resources are automatically managed across async boundaries
- Data races are impossible due to affine types
- Async operations respect temporal constraints

## Phase 1: Temporal Type Foundation (Week 1-2)

### 1.1 Parser Extensions
```restrict
// Add ~ prefix parsing
record File<~f> { ... }

// Add 'within' constraint parsing
where ~tx within ~db
```

**Tasks:**
- [ ] Modify lexer to recognize `~` as temporal prefix
- [ ] Extend type parser for `<~t>` syntax
- [ ] Add `within` keyword and constraint parsing
- [ ] Update AST to include temporal type parameters

### 1.2 Type System Core
```restrict
// Temporal type representation
enum TypeParam {
    Type(String),      // T, U, V
    Temporal(String),  // ~t, ~u, ~v
}

// Temporal constraints
struct TemporalConstraint {
    inner: TemporalVar,  // ~tx
    outer: TemporalVar,  // ~db
}
```

**Tasks:**
- [ ] Add TemporalVar to type system
- [ ] Implement basic temporal inference
- [ ] Add constraint validation
- [ ] Create test suite for temporal types

## Phase 2: Context-Resource Integration (Week 3-4)

### 2.1 Context Lifetime Binding
```restrict
// Contexts implicitly create temporal variables
context FileSystem<~fs> {
    open: (String, File<~fs> -> R) -> R
}
```

**Tasks:**
- [ ] Link contexts to temporal variables
- [ ] Implement callback-style resource methods
- [ ] Add automatic cleanup generation
- [ ] Test with FileSystem example

### 2.2 Nested Temporal Scopes
```restrict
with Database {                    // ~db created
    Database.connect { conn ->     // conn: Connection<~db>
        conn.beginTx { tx ->       // tx: Transaction<~tx>
            // Verify ~tx within ~db
        }
    }
}
```

**Tasks:**
- [ ] Implement nested lifetime tracking
- [ ] Validate temporal relationships
- [ ] Generate proper cleanup order
- [ ] Error messages for constraint violations

## Phase 3: Async Foundation (Week 5-6)

### 3.1 Future Type with Temporals
```restrict
// Futures carry temporal information
type Future<T, ~completion> = {
    poll: Self -> PollResult<T, ~completion>
}

type PollResult<T, ~t> = 
    | Ready(T)
    | Pending(Future<T, ~t>)
```

**Tasks:**
- [ ] Define Future type with temporal
- [ ] Implement basic async/await transformation
- [ ] Ensure temporals propagate through futures

### 3.2 Async Context
```restrict
context Async<~async> {
    spawn: (fn() -> T) -> Future<T, ~async>
    await: Future<T, ~async> -> T
}
```

**Tasks:**
- [ ] Create Async context
- [ ] Implement green thread scheduler
- [ ] Add await mechanism
- [ ] Test basic async operations

## Phase 4: Temporal Async Integration (Week 7-8)

### 4.1 Async Resource Management
```restrict
// Resources valid across async boundaries
fun fetchData<~http> = url: String {
    with Async with HttpClient<~http> {
        val response = (url) http.get |> await;  // Response<~http>
        val data = response.json() |> await;     // Data valid within ~http
        data
    }  // Cleanup even after async operations
}
```

**Tasks:**
- [ ] Ensure temporal cleanup after async
- [ ] Handle async cancellation
- [ ] Validate temporal constraints across await points

### 4.2 Temporal Channels
```restrict
// Channel lifetimes for safe communication
record Channel<T, ~ch> {
    sender: Sender<T, ~ch>
    receiver: Receiver<T, ~ch>
}

fun worker<~work> = {
    with Async {
        val (tx, rx) = Channel.create<Int, ~work>();
        
        spawn(|| {
            (42) tx.send;
            tx.close;
        });
        
        val result = rx.receive |> await;
    }  // Channel cleaned up after async
}
```

**Tasks:**
- [ ] Implement temporal channels
- [ ] Ensure send/receive respect lifetimes
- [ ] Add channel cleanup
- [ ] Test concurrent scenarios

## Phase 5: Advanced Features (Week 9-10)

### 5.1 Structured Concurrency
```restrict
// Parent task owns child temporals
fun processItems<~batch> = items: List<Item> {
    with Async<~batch> {
        val futures = items |> map(|item| {
            spawn<~task>(|| {   // ~task within ~batch
                item.process()
            })
        });
        
        futures |> Future.all |> await
    }  // All tasks cancelled if not complete
}
```

### 5.2 Async Resource Pools
```restrict
// Pool with temporal leases
context ConnectionPool<~pool> {
    acquire: (Connection<~lease> -> R) -> Future<R, ~pool>
    where ~lease within ~pool
}
```

## Theoretical Work (Parallel Track)

### Formal Model
1. **Temporal Logic**: Define formal semantics for temporal constraints
2. **Safety Proofs**: Prove no use-after-free, no data races
3. **Inference Algorithm**: Formalize temporal inference rules
4. **Async Semantics**: Define how temporals work across await points

### Research Questions
1. How do temporals interact with exceptions?
2. Can we infer optimal cleanup points?
3. How to handle recursive temporal relationships?
4. What's the relationship between temporal and linear logic?

## Implementation Strategy

### Week 1-2: Basic Temporal Types
```restrict
// Start simple
record File<~f> { handle: FileHandle }
fun readFile<~io> = file: File<~io> -> String
```

### Week 3-4: Context Integration
```restrict
// Add context support
with FileSystem {
    FileSystem.open("data.txt") { file ->
        file.read()
    }
}
```

### Week 5-6: Simple Async
```restrict
// Basic futures
fun fetchUrl = url: String {
    with Async {
        (url) http.get |> await
    }
}
```

### Week 7-8: Temporal + Async
```restrict
// Combined power
fun processAsync<~work> = {
    with Async with Resources<~work> {
        // Safe async resource management
    }
}
```

## Success Metrics

1. **Correctness**: No temporal safety violations in test suite
2. **Ergonomics**: Clean syntax that feels natural
3. **Performance**: Zero runtime overhead for temporal checks
4. **Async Safety**: No data races in concurrent code
5. **Learnability**: Clear error messages and documentation

## Next Immediate Steps

1. Set up temporal type test framework
2. Implement `~` prefix in parser
3. Create basic File I/O example
4. Write formal temporal constraint rules
5. Design async Future type

This roadmap balances theoretical rigor with practical implementation, building towards a unique async model that leverages temporal types for safety and ergonomics!