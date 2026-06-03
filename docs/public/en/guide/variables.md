# Variables and Mutability

Restrict Language uses affine ownership for heap-backed values. A `String`,
record, `List`, or `Array` binding can be consumed at most once unless it is
declared mutable. Copyable primitives such as `Int32`, `Int64`, `Float64`,
`Boolean`, `Char`, and `()` can be reused because each use copies the value.
Unless a section explicitly says "top-level", examples on this page show local
binding forms.

## Immutable Variables

By default, bindings are immutable. Heap-backed values move when used:

```restrict
val message = "Hello"
val next = message    // message is moved to next
// val again = message // Error: message has already been used
```

Copyable primitives can be read more than once:

```restrict
val score: Int32 = 42
val doubled = score + score
val passed = score >= 40
```

## Top-Level Constants

At file scope, v0.0.1 supports only literal constants that can be emitted as
host-visible WebAssembly globals: `Int32`, `Int64`, `Float64`, `Boolean`,
`Char`, and `()`.

```restrict
pub val release_bias: Int32 = 3
pub val risk_limit: Float64 = 0.75
pub val no_payload: () = ()
```

`String`, records, lists or arrays, `Option`, and `Result` values require a
composite host ABI or runtime allocation. Declare those values inside functions
instead of exporting them as top-level bindings in v0.0.1. Top-level mutable
bindings are not constants and are also outside the current release surface.

## Mutable Variables

When you need to modify a value, use `mut`:

```restrict
mut val counter = 0
counter = counter + 1  // OK: mutable variables can be reassigned
counter = counter + 1  // OK: and used multiple times
val final_count = counter  // counter is consumed here
```

## Data Flow with `|>`

The pipe operator `|>` passes a value to a function. Bind intermediate results
with `val` when they need a name:

```restrict
val x = 42
val temp = x |> double
val result = temp
```

## Mutable Binding

Mutable bindings use `mut val` and explicit assignment:

```restrict
mut val counter = 0
counter = counter + 1
counter = counter + 1
```

## Shadowing

You can shadow variables by creating new bindings with the same name:

```restrict
val x = 5
val x = x + 1  // New x shadows the old one
val x = "now I'm a string"  // Types can change with shadowing
```

## Pattern Binding

Variables can be bound through pattern matching:

```restrict
val (a, b) = (10, 20)  // Destructures tuple
val Person { name, age } = person  // Destructures record

// In match expressions
some_option match {
    Some(value) => { value * 2 }  // value is bound here
    None => { 0 }
}
```

## Best Practices

1. **Use immutable variables by default** - Only use `mut` when necessary
2. **Let the affine system guide you** - If a heap-backed value needs multiple fields, destructure it once
3. **Embrace the pipe operator** - It makes data flow explicit and clear
4. **Use meaningful names** - Since variables are often used only once, make them descriptive

## Common Patterns

### Accumulator Pattern
```restrict
fun sum_list: (lst: List<Int32>, acc: Int32) -> Int32 = {
    lst match {
        [] => { acc }
        [head | tail] => { (tail, acc + head) sum_list }
    }
}
```

### Builder Pattern
```restrict
fun build_person: () -> Person = {
    val draft = Person { name: "Alice", age: 0, email: "" };
    val aged = (draft, 25) set_age;
    val complete = (aged, "alice@example.com") set_email;
    complete build
}
```

## See Also

- [Affine Types](affine-types.md) - Deep dive into the affine type system
- [Functions](functions.md) - How functions interact with variables
- [Pattern Matching](../advanced/patterns.md) - Advanced pattern binding
