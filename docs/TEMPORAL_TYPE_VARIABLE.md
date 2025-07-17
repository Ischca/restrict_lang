# Temporal Type Variables: A Different Perspective

## Concept Shift

Instead of thinking about "lifetimes" as a separate concept, we can think of them as **temporal type variables** - just another dimension of type parameterization.

## Current Type System
```restrict
// Regular type variables
record List<T> { ... }           // T is a type variable
record Map<K, V> { ... }         // K, V are type variables

// Function with type variables
fun identity<T> = x: T -> T { x }
```

## Extended with Temporal Dimension
```restrict
// Temporal type variables use ' prefix
record File<'t> { ... }          // 't is a temporal type variable
record Connection<'t> { ... }     // 't is when this connection is valid

// Mixed type and temporal variables
record Cache<T, 'valid> { ... }  // T = type, 'valid = temporal
record Transaction<'tx, 'db> { ... }  // Both temporal variables
```

## Why "Temporal Type Variable"?

### 1. **Conceptual Unity**
```restrict
// All just type parameters with different purposes
List<Int>                    // Parameterized by type
File<'session>              // Parameterized by time
Future<String, 'completion> // Parameterized by both
```

### 2. **Familiar Mental Model**
```restrict
// Developers already understand type variables
fun map<T, U> = list: List<T>, f: T -> U -> List<U>

// Temporal variables work the same way
fun process<'t> = file: File<'t> -> Result<'t>
```

### 3. **Clear Naming Convention**
- `T, U, V` = Type variables (what it contains)
- `'t, 'u, 'v` = Temporal variables (when it's valid)
- `K, V` = Conventional type variables (Key, Value)
- `'conn, 'tx` = Descriptive temporal variables

## Syntax Benefits

### No New Keywords
```restrict
// Just an extension of existing generics
record Response<T, 'valid> {
    data: T
    timestamp: Time<'valid>
}
```

### Natural Constraints
```restrict
// Type constraints
fun sort<T: Ord> = list: List<T> -> List<T>

// Temporal constraints  
fun transfer<'tx, 'db> = tx: Transaction<'tx, 'db> -> Result
where 'tx within 'db
```

### Inference Works the Same
```restrict
// Type inference
val nums = [1, 2, 3];  // List<Int> inferred

// Temporal inference
val file = fs.open("data.txt");  // File<'1> inferred
```

## Documentation and Communication

### In Comments
```restrict
// This function accepts a Connection with temporal variable 'conn
fun query<'conn> = conn: Connection<'conn>, sql: String -> Result<'conn>
```

### In Errors
```
Error: Temporal type variable 'tx must be within 'db
Error: Cannot return value with temporal variable 'conn outside its scope
Error: Temporal variable 'a has expired
```

### In Teaching
"Just like `List<T>` is a list of some type T, `File<'t>` is a file valid during some time 't"

## Implementation Advantages

### 1. **Unified Type System**
```rust
enum TypeParam {
    Type(String),      // T, U, V
    Temporal(String),  // 't, 'u, 'v
}
```

### 2. **Consistent Rules**
- Both follow same scoping rules
- Both can be inferred
- Both can have constraints

### 3. **Gradual Complexity**
```restrict
// Level 1: No variables
fun readFile = path: String -> String

// Level 2: Type variables
fun map<T> = list: List<T>, f: T -> T -> List<T>

// Level 3: Temporal variables
fun withFile<'t> = path: String, f: File<'t> -> Result -> Result

// Level 4: Both
fun cachedQuery<T, 'cache> = key: String -> Option<T, 'cache>
```

## Complete Example

```restrict
// Temporal type variables in practice
record Server<'server> {
    port: Int
    handler: Handler<'server>
}

record Request<'req, 'server> where 'req within 'server {
    headers: Map<String, String>
    body: Stream<'req>
}

record Response<'resp, 'req> where 'resp within 'req {
    status: Int
    body: String
}

// Using temporal type variables
fun handleRequest<'server, 'req, 'resp> = 
    server: Server<'server>,
    request: Request<'req, 'server> 
    -> Response<'resp, 'req>
where 'req within 'server, 'resp within 'req {
    // Process request and return response
}
```

## Terminology

- **Temporal Type Variable**: `'t`, `'conn`, etc.
- **Type Variable**: `T`, `U`, etc.
- **Temporal Constraint**: `'a within 'b`
- **Type Constraint**: `T: Ord`

## Benefits of This Framing

1. **No new concepts** - Just another kind of type variable
2. **Consistent syntax** - Uses existing generic syntax
3. **Easy to explain** - "It's like generics, but for time"
4. **Unified mental model** - One parameterization system

What do you think? This framing makes temporal types feel like a natural extension rather than a foreign concept.