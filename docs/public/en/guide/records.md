# Records and Prototypes

Records group named fields. In v0.0.1 they are fully useful inside Restrict
programs and source modules, while direct host-visible record exports remain
outside the stable ABI.

## Record Definitions

Record declarations list fields with explicit types:

```restrict
record Point {
    x: Int32,
    y: Int32
}

record Person {
    name: String,
    age: Int32,
    email: String
}
```

## Record Literals

Record literals use `:` field initializers:

```restrict
fun origin: () -> Point = {
    Point { x: 0, y: 0 }
}

fun default_person: () -> Person = {
    Person {
        name: "Alice",
        age: 30,
        email: "alice@example.com"
    }
}
```

Do not use `field = value` inside record literals.

## Field Access And Destructuring

Access a single field with dot notation:

```restrict
fun x_coordinate: (point: Point) -> Int32 = {
    point.x
}
```

When multiple fields are needed from the same affine record, destructure once:

```restrict
fun sum_point: (point: Point) -> Int32 = {
    val Point { x, y } = point
    x + y
}
```

Destructuring also lets explicit field names bind to different local names:

```restrict
fun distance_like: (point: Point) -> Int32 = {
    val Point { x: px, y: py } = point
    (px * px) + (py * py)
}
```

## Pattern Matching

Records can be matched by shape:

```restrict
fun classify_point: (point: Point) -> String = {
    point match {
        Point { x: 0, y: 0 } => { "origin" }
        Point { x: 0, y } => { "on y-axis" }
        Point { x, y: 0 } => { "on x-axis" }
        Point { x, y } => { "somewhere else" }
    }
}
```

Branch bodies are expressions. If a branch needs multiple fields for
calculation, bind them in the pattern and use the bindings inside the branch.

## Clone Updates

Use `.clone { ... }` to create a modified record value:

```restrict
fun move_x: (point: Point, next_x: Int32) -> Point = {
    point.clone { x: next_x }
}
```

The original record value is consumed by the update. The result is a new record
value with the changed fields.

## Freeze

`freeze` creates an immutable prototype-style value:

```restrict
fun freeze_point: (point: Point) -> Point = {
    point freeze
}
```

Do not model record updates as direct field assignment such as `point.x = 15`.
Use `.clone { ... }` for persistent updates, or a mutable binding assignment
when replacing the entire binding value.

## Impl Functions

`impl` blocks attach type-directed functions to a record. Calls remain OSV; dot
method calls are not part of Restrict.

```restrict
impl Point {
    fun squared_distance: (self: Point) -> Int32 = {
        val Point { x, y } = self
        (x * x) + (y * y)
    }

    fun translate: (self: Point, dx: Int32, dy: Int32) -> Point = {
        val Point { x, y } = self
        Point { x: x + dx, y: y + dy }
    }
}

fun main: () -> Int32 = {
    val point = Point { x: 3, y: 4 }
    (point) squared_distance
}
```

For multiple arguments, keep the receiver first:

```restrict
fun moved_x: () -> Int32 = {
    val point = Point { x: 3, y: 4 }
    val moved = (point, 1, 2) translate
    moved.x
}
```

## Nested Records

Records can contain other records:

```restrict
record Address {
    street: String,
    city: String,
    country: String
}

record Employee {
    name: String,
    age: Int32,
    address: Address
}

fun employee_city: (employee: Employee) -> String = {
    val Employee { name, age, address } = employee
    val Address { street, city, country } = address
    city
}
```

If only one nested field is needed, direct field access is fine. If several
fields are needed, destructure the outer and inner records once.

## Host Boundary

Records are not directly exported as host-visible WebAssembly values in
v0.0.1. Export scalar wrappers instead:

```restrict
export fun exported_distance: () -> Int32 = {
    val point = Point { x: 3, y: 4 }
    (point) squared_distance
}
```

See the [v0.0.1 Release Surface](../reference/release-surface.md) for the host
ABI boundary.
