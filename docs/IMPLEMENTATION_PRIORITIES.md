# Implementation Priorities for Restrict Language

## 🎯 Immediate Priorities (This Week)

### 1. Fix Core Language Features
These are blocking basic usage and must be fixed first:

#### Recursive Functions
```restrict
// Currently broken - infinite loop or stack overflow
fun factorial = n: Int32 {
    if n <= 1 then 1
    else n * factorial(n - 1)  // FIXME: Recursion fails
}
```
**Action**: Fix function table and self-reference resolution

#### Pattern Matching Completion  
```restrict
// Works but incomplete
list match {
    [] => 0
    [x | xs] => x + sum(xs)  // FIXME: Recursive patterns
}
```
**Action**: Complete pattern compilation and exhaustiveness checking

### 2. TAT Cleanup Generation
The temporal type system is designed but needs runtime support:

```restrict
with lifetime<~io> {
    val file = fs.open("data.txt");
    // TODO: Generate cleanup code here
}  // <- Insert file.close() in WASM
```

**Tasks**:
- [ ] Identify cleanup points in AST
- [ ] Generate WASM cleanup blocks
- [ ] Handle early returns/exceptions
- [ ] Test with actual resources

## 📅 Next Sprint Priorities

### 1. Error Message Poetry
Transform cryptic errors into helpful haikus:

```
Error: Temporal escape from ~io
// Becomes:
Resource escapes time—
~io ends but reference lives.
Lifetime mismatch found.
```

### 2. Clone Strategy System
```restrict
trait CloneStrategy<T> {
    fun clone = self: T -> T
}

// Automatic for simple types
impl CloneStrategy<Int32> {
    fun clone = self { self }  // Copy
}

// Explicit for affine types
impl CloneStrategy<File> {
    fun clone = self { 
        error("Cannot clone affine File")
    }
}
```

## 🗓️ Month Priorities

### 1. Arena-Lifetime Integration
Connect memory arenas to temporal scopes:

```restrict
with lifetime<~compute> {
    with arena(1_000_000) {  // 1MB arena for ~compute
        val matrix = Matrix.new(1000, 1000);
        val result = matrix.multiply(matrix);
        result.sum()
    }  // Arena freed with ~compute
}
```

### 2. Module System Foundation
```restrict
module DataProcessing {
    export record Dataset<~data> {
        entries: List<Entry>
    }
    
    export fun analyze<~batch> = 
        data: Dataset<~batch> -> Results<~batch>
}
```

## 🎨 Design Decisions to Document

### 1. OSV Purity Levels

Define when OSV is required vs optional:

**Required OSV**:
- Function calls: `(arg) function`  
- Method calls: `obj.method`
- Pipe operations: `data |> process`

**Flexible Syntax**:
- Binary operators: `a + b` (not `(a, b) add`)
- Property access: `obj.field`
- Literals and constants

### 2. Temporal Inference Rules

Document when temporal parameters can be inferred:

```restrict
// Explicit
fun process<~io> = file: File<~io> -> Result<~io>

// Inferred
val file = fs.open("data.txt");  // Infers File<~1>
val result = process(file);      // Infers Result<~1>
```

### 3. Affine Type Exceptions

Some types may bypass affine rules:

```restrict
// Always affine (resources)
- File, Socket, Transaction

// Never affine (pure data)  
- Int32, String, Bool

// Context-dependent
- List<T> - affine if T is affine
- Record { ... } - affine if any field is affine
```

## 📊 Success Metrics

### Week 1 Success
- [ ] Factorial function works
- [ ] Basic pattern matching works
- [ ] TAT cleanup generated for one resource type

### Month 1 Success  
- [ ] All examples in README work
- [ ] Error messages are helpful
- [ ] Can write non-trivial programs

### Quarter 1 Success
- [ ] Module system usable
- [ ] Arena integration complete  
- [ ] Ready for async design

## 🚫 What NOT to Do Now

### Avoid These Temptations:
1. **Multi-dimensional scopes** - Stick with temporal only
2. **Complex type features** - No higher-kinded types yet
3. **Optimization** - Correctness first, speed later
4. **Breaking changes** - Stabilize existing features

### Defer These Features:
1. Async/await - Need solid temporal foundation first
2. Distributed temporals - Local correctness first
3. Effect system - Temporals are effects enough
4. SIMD/GPU - Focus on safety, not performance

## 📝 Documentation Priorities

### For Each Fixed Feature:
1. Update examples to show it working
2. Add test case to prevent regression  
3. Document in CHANGELOG
4. Update tutorial if needed

### New Documentation Needed:
1. **Temporal Types Tutorial** - Gentle introduction
2. **Migration Guide** - For existing code
3. **Pattern Cookbook** - Common patterns
4. **Error Reference** - What each error means

## 🎭 Communication Strategy

### Internal Team:
"We're building the world's first production Temporal Affine Type system. Each bug fixed brings us closer to something revolutionary."

### Open Source Community:
"Restrict combines Rust's safety with functional elegance. Try our latest build and tell us what breaks!"

### Future Users:
"Imagine never worrying about resource leaks, double-frees, or use-after-free again. That's Restrict's promise."

## Remember

As Doc Sage teaches: *"Documentation that cannot be understood might as well not exist."*

Every feature we implement must be:
1. **Understandable** - Clear mental model
2. **Documented** - With examples
3. **Tested** - Including edge cases
4. **Useful** - Solves real problems

Let's build a language that makes the impossible possible, and the possible easy! 🌸