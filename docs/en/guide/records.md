# Records and Prototypes

Records are the primary way to group related data in Restrict Language. They use a prototype-based system with `clone` and `freeze` operations for inheritance and immutability.

## Record Definition

Records are defined with space-separated fields:

```rust
record Point {
    x: Int
    y: Int
}

record Person {
    name: String
    age: Int
    email: String
}
```

## Creating Records

Create record instances with space-separated field values:

```rust
val origin = Point { x: 0 y: 0 }
val point = Point { x: 10 y: 20 }

val alice = Person {
    name: "Alice"
    age: 30
    email: "alice@example.com"
}
```

## Field Access

Access record fields using dot notation:

```rust
val p = Point { x: 5 y: 10 }
val x_coord = p.x  // 5
val y_coord = p.y  // 10
```

## Pattern Matching on Records

Records can be destructured in pattern matching:

```rust
val point = Point { x: 10 y: 20 }

// Match specific values
point match {
    Point { x: 0 y: 0 } => { "origin" }
    Point { x: 0 y } => { "on y-axis" }
    Point { x y: 0 } => { "on x-axis" }
    Point { x y } => { x + y }
}

// Shorthand field binding
point match {
    Point { x y } => { x * y }  // x and y are bound to field values
}

// Rename bindings
point match {
    Point { x: px y: py } => { px + py }
}
```

## Methods

Implement methods for records using `impl` blocks:

```rust
impl Point {
    fun distance = self: Point {
        // Calculate distance from origin
        val x_sq = self.x * self.x
        val y_sq = self.y * self.y
        sqrt(x_sq + y_sq)
    }
    
    fun translate = self: Point dx: Int dy: Int {
        Point { x: self.x + dx y: self.y + dy }
    }
}

// Usage
val p = Point { x: 3 y: 4 }
val dist = p distance        // 5.0
val moved = (p, 1, 2) translate  // Point { x: 4 y: 6 }
```

## Prototype System

Records support prototype-based inheritance through `clone` and `freeze`:

### Clone

Create a mutable copy of a record:

```rust
val base = Point { x: 0 y: 0 }
val mut copy = base clone

// Modify the clone
copy.x = 10
// base remains unchanged
```

### Freeze

Make a record immutable:

```rust
val mut point = Point { x: 5 y: 10 }
point.x = 15  // OK, point is mutable

val frozen = point freeze
// frozen.x = 20  // Error: cannot modify frozen record
```

## Nested Records

Records can contain other records:

```rust
record Address {
    street: String
    city: String
    country: String
}

record Employee {
    name: String
    age: Int
    address: Address
}

val emp = Employee {
    name: "Bob"
    age: 25
    address: Address {
        street: "123 Main St"
        city: "Springfield"
        country: "USA"
    }
}

// Access nested fields
val city = emp.address.city
```

## Type Annotations

Always specify types for record fields:

```rust
record Config {
    host: String      // Required type annotation
    port: Int         // Required type annotation
    debug: Bool       // Required type annotation
}
```

## Important Notes

- Fields are space-separated, not comma-separated
- Field order matters for record construction
- Records follow affine typing - each record value can be used at most once
- Pattern matching on records requires exhaustive patterns or a wildcard

## Future Features

- Generic records with type parameters
- Default field values
- Field visibility modifiers
- Derived implementations (like equality)