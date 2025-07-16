# Pattern Matching

Pattern matching is a powerful feature in Restrict Language that allows you to destructure and match against complex data structures. The language follows OSV (Object-Subject-Verb) syntax for match expressions.

## Basic Syntax

Match expressions in Restrict Language use OSV syntax:

```rust
expression match {
    pattern1 => { result1 }
    pattern2 => { result2 }
    _ => { default }
}
```

## Literal Patterns

Match against literal values:

```rust
val number = 42
number match {
    0 => { "zero" }
    1 => { "one" }
    42 => { "the answer" }
    _ => { "other" }
}
```

## Variable Binding

Bind matched values to variables:

```rust
val value = 10
value match {
    x => { x * 2 }  // Binds the value to x
}
```

## Option Patterns

Match against Option types:

```rust
val maybe: Option<Int> = 42 some
maybe match {
    Some(value) => { value * 2 }
    None => { 0 }
}
```

## List Patterns

Destructure lists with various patterns:

```rust
val numbers = [1, 2, 3, 4]

// Empty list pattern
[] match {
    [] => { "empty" }
    _ => { "not empty" }
}

// Head and tail pattern
numbers match {
    [] => { "empty" }
    [head | tail] => { head }  // Returns 1
}

// Exact length patterns
val pair = [1, 2]
pair match {
    [a, b] => { a + b }  // Returns 3
    _ => { 0 }
}
```

## Record Patterns

Destructure records and extract fields:

```rust
record Point { x: Int y: Int }

val origin = Point { x: 0 y: 0 }
val point = Point { x: 10 y: 20 }

// Match specific field values
origin match {
    Point { x: 0 y: 0 } => { "origin" }
    Point { x y } => { x + y }
    _ => { "unknown" }
}

// Field binding with shorthand syntax
point match {
    Point { x y } => { x * y }  // Both x and y are bound
}

// Explicit field patterns
point match {
    Point { x: px y: py } => { px + py }  // Bind to different names
}
```

## Nested Patterns

Combine patterns for complex matching:

```rust
record Person { name: String age: Int }
record Company { name: String employees: List<Person> }

val company = Company {
    name: "Tech Corp"
    employees: [
        Person { name: "Alice" age: 30 }
        Person { name: "Bob" age: 25 }
    ]
}

company match {
    Company { name employees: [] } => { "No employees" }
    Company { name employees: [first | rest] } => { first.name }
    _ => { "Unknown" }
}
```

## Exhaustiveness

The type checker ensures that match expressions are exhaustive. You must either:
- Cover all possible cases
- Include a wildcard pattern (`_`)

```rust
// This will fail type checking - not exhaustive
val opt: Option<Int> = None<Int>
opt match {
    Some(x) => { x }
    // Missing None case!
}

// This is correct - exhaustive
opt match {
    Some(x) => { x }
    None => { 0 }
}
```

## Important Notes

- All pattern bodies must be enclosed in braces `{ }`
- Match expressions follow OSV syntax: `expression match { ... }`
- The affine type system ensures each binding is used at most once
- Patterns are checked for exhaustiveness at compile time

## Future Features

- Pattern guards with `then` conditions
- Tuple patterns (when tuples are implemented)
- Advanced list patterns with rest syntax `[x, y, ...rest]`