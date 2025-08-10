# Field Access Design for Restrict Language

## Problem Statement

Records in Restrict Language are affine types (use-once), which prevents accessing multiple fields:
- `p.x` consumes `p`  
- Subsequent `p.y` causes `AffineViolation`
- Borrowing is explicitly rejected as a solution

## Design Philosophy

Embrace constraints as features rather than limitations. The restriction should lead to:
- More explicit code about field usage
- Better memory safety guarantees  
- Clearer resource management patterns

## Proposed Solution: Hybrid Approach

### 1. Destructuring as Primary Pattern (Idiomatic)

```restrict
record Point { x: Int32, y: Int32 }
val p = Point { x = 10, y = 20 }

// PREFERRED: Destructure to access multiple fields
val Point { x, y } = p  // Single consumption, access to all fields

// Partial destructuring
val Point { x, .. } = p  // Only extract x, rest is discarded
```

### 2. Copy-Field Access Optimization

Field access doesn't consume the record if the field type is copyable:

```restrict
record Point { x: Int32, y: Int32 }         // Int32 is copyable
record Resource { handle: FileHandle, id: Int32 }  // FileHandle is affine

val p = Point { x = 10, y = 20 }
val x = p.x  // OK: Int32 is copyable, doesn't consume p  
val y = p.y  // OK: Int32 is copyable, p still available

val r = Resource { handle = open_file(), id = 42 }
val id = r.id        // OK: Int32 is copyable, doesn't consume r
val handle = r.handle // Consumes r (FileHandle is affine)
// val id2 = r.id     // ERROR: r already consumed
```

### 3. Compiler Guidance

When multiple non-copyable field accesses are detected, suggest destructuring:

```
Error: Cannot access field 'name' - record 'user' already consumed
Hint: Consider destructuring: `val User { name, age } = user`
```

## Implementation Details

### Type Checker Changes

```rust
fn check_field_access(&mut self, expr: &Expr, field: &str) -> Result<TypedType, TypeError> {
    // First determine the object type and field type
    let obj_ty = self.infer_expr_type(expr)?;
    let field_ty = self.get_field_type(&obj_ty, field)?;
    
    // Consumption strategy based on field type
    if self.is_copyable(&field_ty) {
        // Copyable field: access without consuming the record
        self.check_expr_without_consumption(expr)?;
    } else {
        // Affine field: consume the record  
        self.check_expr(expr)?;
    }
    
    Ok(field_ty)
}
```

### Copyable Type Rules

```rust
fn is_copyable(&self, ty: &TypedType) -> bool {
    match ty {
        // Primitives are copyable
        TypedType::Int32 | TypedType::Boolean | TypedType::Float64 
        | TypedType::Char | TypedType::Unit => true,
        
        // Heap-allocated types are not copyable
        TypedType::String | TypedType::List(_) => false,
        
        // Records are copyable only if ALL fields are copyable
        TypedType::Record { name, .. } => {
            let record_def = self.records.get(name).unwrap();
            record_def.fields.values().all(|ty| self.is_copyable(ty))
        }
        
        // Functions and handles are never copyable
        TypedType::Function { .. } => false,
        
        // Composite types
        TypedType::Option(inner) => self.is_copyable(inner),
        TypedType::Array(inner, _) => self.is_copyable(inner),
        
        // Temporal types follow base type rules
        TypedType::Temporal { base_type, .. } => self.is_copyable(base_type),
    }
}
```

## Benefits of This Approach

1. **Preserves Affine Safety**: Non-copyable resources still follow use-once semantics
2. **Practical for Primitives**: Common case of accessing multiple numeric fields just works
3. **Encourages Good Patterns**: Complex records benefit from explicit destructuring
4. **Zero-cost Abstraction**: Copyable field access compiles to simple memory reads
5. **Backward Compatible**: Existing single field access continues to work

## Examples

### Copyable Record (All fields copyable)
```restrict
record Point2D { x: Float64, y: Float64 }
val p = Point2D { x = 1.0, y = 2.0 }
val x = p.x  // OK: Float64 is copyable
val y = p.y  // OK: Float64 is copyable  
val len = sqrt(p.x * p.x + p.y * p.y)  // OK: can use p multiple times
```

### Mixed Record (Some fields affine)
```restrict  
record User { id: Int32, name: String, handle: FileHandle }
val user = User { id = 123, name = "Alice", handle = open_log() }

val id1 = user.id    // OK: Int32 is copyable
val id2 = user.id    // OK: can access copyable field multiple times

val name = user.name  // Consumes user (String is affine)
// val handle = user.handle  // ERROR: user already consumed

// PREFERRED: Destructure for multiple affine fields
val User { id, name, handle } = user  // Single consumption
```

### Resource Management
```restrict
record Database { connection: DbConn, stats: DbStats }
record DbStats { queries: Int32, errors: Int32, uptime: Float64 }

val db = Database { 
    connection = connect_db(),
    stats = DbStats { queries = 100, errors = 2, uptime = 3600.0 }
}

// Can access stats multiple times (all Int32/Float64)
val query_count = db.stats.queries  // Nested copyable access
val error_rate = db.stats.errors / db.stats.queries

val conn = db.connection  // Consumes db (DbConn is affine)
```

## Migration Path

1. **Phase 1**: Implement copyable field access optimization
2. **Phase 2**: Add compiler hints suggesting destructuring  
3. **Phase 3**: Enhance destructuring syntax with pattern guards
4. **Phase 4**: Consider advanced prototype-based view patterns

This design embraces Restrict's philosophy: constraints that lead to safety and clearer code patterns.