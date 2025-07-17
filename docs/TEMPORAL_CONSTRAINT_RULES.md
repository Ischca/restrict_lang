# Temporal Type Constraint Rules

## Core Concepts

### 1. Temporal Type Variables
A temporal type variable `~t` represents a **scope of validity** for a resource. It answers: "When is this resource valid?"

### 2. Temporal Relationships
- `~a within ~b`: Temporal `~a` is contained within temporal `~b`
- `~a = ~b`: Temporals are the same (unified)
- `~a ⊥ ~b`: Temporals are disjoint (cannot overlap)

## Formal Rules

### Rule 1: Temporal Creation (T-Create)
```
Γ ⊢ e : T    fresh ~t
─────────────────────────── (T-Create)
Γ ⊢ create(e) : T<~t>
```
When a resource is created, it gets a fresh temporal variable.

### Rule 2: Temporal Containment (T-Within)
```
Γ ⊢ e₁ : T<~a>    Γ ⊢ e₂ : U<~b>    ~a within ~b
───────────────────────────────────────────────── (T-Within)
Γ ⊢ use(e₁, e₂) : valid
```
A resource with temporal `~a` can only be used within the scope of temporal `~b` if `~a within ~b`.

### Rule 3: Context Temporal (T-Context)
```
Γ, x : T<~ctx> ⊢ e : U
───────────────────────────────────── (T-Context)
Γ ⊢ with C { x => e } : U
```
A `with` context creates a temporal scope `~ctx` that bounds all resources created within.

### Rule 4: Function Application (T-App)
```
Γ ⊢ f : ∀~a. T<~a> → U<~a>    Γ ⊢ e : T<~b>
──────────────────────────────────────────── (T-App)
Γ ⊢ f(e) : U<~b>
```
Temporal variables are instantiated during function application.

### Rule 5: Temporal Escape Prevention (T-NoEscape)
```
Γ, ~a ⊢ e : T<~a>    ~a ∉ FreeTemporals(Γ)
─────────────────────────────────────────── (T-NoEscape)
ERROR: Cannot return T<~a> outside scope of ~a
```
A value with temporal `~a` cannot escape the scope where `~a` is defined.

### Rule 6: Nested Temporals (T-Nest)
```
Γ, ~outer ⊢ { Γ, ~inner, ~inner within ~outer ⊢ e : T }
───────────────────────────────────────────────────────── (T-Nest)
Γ, ~outer ⊢ nested { e } : T
```
Nested scopes create nested temporal relationships.

### Rule 7: Temporal Inference (T-Infer)
```
Γ ⊢ e : T    NeedsCleanup(T)    ~t fresh
──────────────────────────────────────── (T-Infer)
Γ ⊢ e : T<~t>
```
If a type needs cleanup, infer a temporal variable.

## Constraint Satisfaction Rules

### CS1: Reflexivity
```
────────────── (CS-Refl)
~a within ~a
```
Every temporal is within itself.

### CS2: Transitivity
```
~a within ~b    ~b within ~c
───────────────────────────── (CS-Trans)
~a within ~c
```
`within` is transitive.

### CS3: Anti-symmetry
```
~a within ~b    ~b within ~a
───────────────────────────── (CS-Anti)
~a = ~b
```
If temporals contain each other, they're the same.

## Type System Integration

### 1. Resource Types
```
ResourceType<~t> ::= File<~t> | Connection<~t> | Transaction<~t> | ...
```

### 2. Temporal Constraints in Types
```
Transaction<~tx, ~db> where ~tx within ~db
```

### 3. Cleanup Insertion
```
At end of scope for ~t:
  for each resource r : T<~t> in scope:
    insert cleanup(r)
```

## Examples with Rules Applied

### Example 1: Basic File I/O
```restrict
fun readFile = filename: String {
    val file = fs.open(filename);  // T-Create: file : File<~1>
    val content = file.read();      // T-App: read : File<~1> → String
    content                         // OK: String has no temporal
}  // Cleanup for ~1 inserted here
```

### Example 2: Transaction within Database
```restrict
fun transfer = {
    with Database {                    // T-Context: creates ~db
        Database.connect { conn ->     // conn : Connection<~db>
            conn.beginTx { tx ->       // T-Create: tx : Transaction<~tx>
                                       // CS: ~tx within ~db (from type)
                tx.execute("...");     // T-Within: OK, using tx within ~db
            }  // Cleanup for ~tx
        }  // Cleanup for ~db
    }
}
```

### Example 3: Escape Error
```restrict
fun leak = {
    with FileSystem {              // T-Context: creates ~fs
        val file = open("test");   // file : File<~fs>
        file                       // T-NoEscape: ERROR!
    }                              // ~fs not in outer scope
}
```

## Inference Algorithm

### 1. Constraint Collection Phase
```
infer(create(e)) = 
    let T = infer(e)
    let ~t = fresh_temporal()
    return (T<~t>, {})

infer(with C { e }) = 
    let ~ctx = fresh_temporal()
    let (T, constraints) = infer(e) with ~ctx
    return (T, constraints)

infer(f(x)) where f : ∀~a. T<~a> → U<~a> =
    let (T', C1) = infer(x)
    let ~b = fresh_temporal()
    let C2 = unify(T<~b>, T')
    return (U<~b>, C1 ∪ C2)
```

### 2. Constraint Solving Phase
```
solve(constraints):
    1. Build directed graph of temporals
    2. Check for cycles (error if found)
    3. Compute transitive closure
    4. Verify all 'within' constraints
    5. Insert cleanup at scope boundaries
```

## Safety Properties

### Property 1: No Use After Free
**Theorem**: If a program type-checks with temporal constraints, no resource is used after its temporal scope ends.

**Proof sketch**: By T-NoEscape and cleanup insertion.

### Property 2: No Resource Leaks
**Theorem**: All resources are cleaned up exactly once.

**Proof sketch**: By construction of cleanup insertion and affine types.

### Property 3: Nested Cleanup Order
**Theorem**: If `~a within ~b`, then cleanup(~a) happens before cleanup(~b).

**Proof sketch**: By CS2 (transitivity) and scope nesting.

## Open Questions

1. **Temporal Polymorphism**: How to handle `∀~t. T<~t> → U<~t>`?
2. **Temporal Bounds**: Should we support `~t: MinLifetime`?
3. **Async Temporals**: How do temporals work across await points?
4. **Temporal Algebra**: Can we define `~a ∪ ~b` (union) or `~a ∩ ~b` (intersection)?

## Next Steps

1. Implement constraint collection in type checker
2. Build constraint solver
3. Add cleanup code generation
4. Prove safety properties formally