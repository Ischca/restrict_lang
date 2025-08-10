# Restrict Language Design Synthesis 2025

## 🌸 Haiku of Our Journey
```
Six sages debate—
Temporal flows through affine types.
Safety blooms in WASM.
```

## Executive Summary

After an intense design discussion featuring six expert personas, Restrict Language stands at a crossroads. The team has explored ambitious features that would make Restrict uniquely powerful, but also discovered critical design flaws that demand careful navigation.

### The Six Sages and Their Wisdom

1. **Linear Lord (線形君主)** - Proposed a three-layer scope model combining temporal, spatial, and capability dimensions
2. **Parser Wizard (構文解析の魔導士)** - Championed OSV syntax consistency and philosophical purity  
3. **Prototype Picasso (プロトタイプ・ピカソ)** - Envisioned differential inheritance with affine types
4. **Type Detective (型探偵)** - Sought a coherent type system unifying all features
5. **WASM Warrior (WASM戦士)** - Confirmed implementation feasibility in WebAssembly
6. **Test Alchemist (テストの錬金術師)** - Discovered critical edge cases and design flaws

### Current State: What Works Today

✅ **Temporal Affine Types (TAT)** - Basic implementation complete:
- Temporal type variables with `~` prefix
- `with lifetime<~t> { ... }` blocks  
- Sublifetime relationships (`~tx within ~db`)
- Basic escape detection

✅ **OSV Syntax** - Fully committed:
- Removed traditional `func(args)` syntax
- Pure Object-Subject-Verb ordering
- Pipe operators for composition

✅ **Core Type System**:
- Affine types (use-at-most-once)
- Option types with pattern matching
- Records with prototype-based inheritance
- Lambda expressions with closures

## Key Decisions Required

### 1. Scope Model Complexity
**The Challenge**: Three-layer scopes (temporal/spatial/capability) create exponential complexity and ambiguity.

**Options**:
- A) **Temporal-First**: Make temporal the primary dimension, others secondary
- B) **Unified Scopes**: Single scope model with optional attributes
- C) **Staged Introduction**: Start with temporal, add others later

**Recommendation**: Option A - Temporal-First
```restrict
// Clear, understandable, implementable
with lifetime<~io> {
    val file = fs.open("data.txt");  // Temporal scope primary
    val buffer = arena.alloc(1024);  // Spatial as attribute
    process(file, buffer);
}  // Both cleaned up based on temporal boundary
```

### 2. Prototype + Affine Types
**The Challenge**: Cloning prototypes with affine fields breaks uniqueness guarantees.

**Options**:
- A) **Restrict Cloning**: Disallow cloning records with affine fields
- B) **Move Semantics**: Clone moves affine fields from parent
- C) **Explicit Strategies**: Require clone strategies for affine fields

**Recommendation**: Option C - Explicit Strategies
```restrict
record Resource { 
    handle: FileHandle  // Affine
}

impl Resource {
    fun clone = self {
        // Must explicitly handle affine fields
        Resource { handle: self.handle.duplicate() }
    }
}
```

### 3. Async/Concurrency Model
**The Challenge**: Temporal scopes + async = potential for races and lifetime violations.

**Options**:
- A) **Structured Concurrency**: All async within parent temporal scope
- B) **Temporal Channels**: Explicit lifetime parameters on channels
- C) **No Async Yet**: Defer until temporal model is solid

**Recommendation**: Option A with B - Structured Concurrency with Temporal Channels
```restrict
fun parallel<~batch> = tasks: List<Task> {
    with lifetime<~batch> {
        val results = Channel<Result, ~batch>.new();
        
        tasks.each { task |
            spawn<~task> where ~task within ~batch {
                val result = task.execute();
                results.send(result);  // Safe: ~batch outlives ~task
            }
        };
        
        results.collect()
    }  // All tasks complete before ~batch ends
}
```

## Phased Implementation Roadmap

### Phase 1: Solidify Foundation (Q1 2025)
1. **Complete TAT Implementation**
   - ✅ Basic temporal types
   - ⏳ Cleanup code generation
   - ⏳ Arena integration
   - ⏳ Better error messages

2. **Stabilize Core Features**
   - ⏳ Fix recursive function support
   - ⏳ Complete pattern matching
   - ⏳ Method resolution

### Phase 2: Prototype Enhancement (Q2 2025)
1. **Safe Differential Inheritance**
   - Explicit clone strategies
   - Affine field handling
   - Method resolution order

2. **Enhanced Type System**
   - Bidirectional inference with temporals
   - Generic temporal parameters
   - Effect tracking

### Phase 3: Concurrency (Q3 2025)
1. **Structured Async/Await**
   - Temporal-aware futures
   - Scope-bounded spawning
   - Deadlock prevention

2. **Temporal Channels**
   - Lifetime-parameterized channels
   - Safe cross-scope communication

### Phase 4: Advanced Features (Q4 2025)
1. **Module System**
   - Temporal module boundaries
   - Safe resource sharing

2. **Foreign Function Interface**
   - WASM component model
   - Temporal safety across boundaries

## How to Communicate These Changes

### For Current Users

#### "The Affine Promise"
> Restrict ensures your resources are used exactly once—no leaks, no double-frees. With temporal types, this safety extends across time.

```restrict
// Your code becomes self-documenting about resource lifetimes
fun processFile = path: String {
    with lifetime<~io> {
        val file = fs.open(path);      // Born with ~io
        val data = file.readAll();     // Lives within ~io  
        process(data)                  // Returns before ~io ends
    }  // file automatically closed, guaranteed!
}
```

### For New Users

#### "Write Like You Think"
> In Restrict, you describe what you want, not how to manage memory. The language ensures safety while you focus on logic.

```restrict
// Natural expression of intent
"Hello, World!" |> println;

// Clear resource management  
with Database {
    users 
    |> filter(active)
    |> map(normalize)
    |> save
}  // Everything cleaned up automatically
```

### For Language Enthusiasts

#### "Type Theory Meets Practical Safety"
> Restrict pioneered Temporal Affine Types—extending linear logic across time dimensions while remaining pragmatic and implementable.

## The Poetic Path Forward

### Tanka for Our Future
```
Temporal rivers
Flow through affine landscapes where
Each resource is one—
Safety emerges from constraints,
Freedom blooms in boundaries.
```

### Design Philosophy Haiku
```
Constrain to liberate,
Simple rules yield safe programs—
Complexity tamed.
```

## Conclusion

Restrict Language stands ready to become the first production language with Temporal Affine Types. By carefully navigating the design challenges identified by our six sages, we can create a language that is:

1. **Safe** - Memory and resource safety guaranteed by type system
2. **Simple** - OSV syntax and clear resource management
3. **Powerful** - Temporal types enable new programming patterns
4. **Practical** - Compiles to efficient WASM, runs everywhere

The path forward requires discipline: we must resist the temptation to implement everything at once. By focusing on temporal types as our north star and carefully staging additional features, we can deliver a language that fulfills its promise of making systems programming both safe and joyful.

Remember the wisdom of Test Alchemist: *"バグは宝物"* (Bugs are treasures). Each design flaw discovered now saves countless runtime errors later. Through careful design and incremental implementation, Restrict will emerge as a jewel among programming languages—cut by constraints, polished by practice, brilliant in its simplicity.

## Call to Action

1. **Immediate**: Fix recursive functions and pattern matching
2. **Next Sprint**: Complete TAT cleanup generation
3. **Next Month**: Implement explicit clone strategies
4. **Next Quarter**: Design structured concurrency model

Let us build a language where safety and expressiveness dance in harmony! 🌸