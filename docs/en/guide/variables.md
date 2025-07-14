# Variables and Mutability

In Restrict Language, variables follow an **affine type system**, meaning each variable can be used at most once. This design choice eliminates many common programming errors and makes memory management predictable without garbage collection.

## Immutable Variables

By default, all variables in Restrict Language are immutable and affine:

```restrict
val x = 42
val y = x    // x is moved to y, x is no longer accessible
// val z = x // Error: x has already been used!
```

This single-use rule applies to all types:

```restrict
val message = "Hello"
message |> println    // message is consumed here
// message |> println // Error: message already used!
```

## Mutable Variables

When you need to modify a value, use `mut`:

```restrict
val mut counter = 0
counter = counter + 1  // OK: mutable variables can be reassigned
counter = counter + 1  // OK: and used multiple times
val final_count = counter  // counter is consumed here
```

## Variable Binding with `|>`

The pipe operator `|>` creates immutable bindings:

```restrict
42 |> x       // Binds 42 to x
|> double     // Passes x to double function
|> result     // Binds the result

// Equivalent to:
val x = 42
val temp = x double
val result = temp
```

## Mutable Binding with `|>>`

For mutable bindings, use the double pipe:

```restrict
0 |>> mut counter
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
val Person { name, age } = get_person()  // Destructures record

// In match expressions
some_option match {
    Some(value) => { value * 2 }  // value is bound here
    None => { 0 }
}
```

## Best Practices

1. **Use immutable variables by default** - Only use `mut` when necessary
2. **Let the affine system guide you** - If you need to use a value multiple times, consider if it should be a function parameter instead
3. **Embrace the pipe operators** - They make data flow explicit and clear
4. **Use meaningful names** - Since variables are often used only once, make them descriptive

## Common Patterns

### Accumulator Pattern
```restrict
fun sum_list = lst:List<Int>, acc:Int {
    lst match {
        [] => { acc }
        [head | tail] => { tail (acc + head) sum_list }
    }
}
```

### Builder Pattern
```restrict
Person { name: "Alice", age: 0 }
|> set_age(25)
|> set_email("alice@example.com")
|> build
```

## See Also

- [Affine Types](affine-types.md) - Deep dive into the affine type system
- [Functions](functions.md) - How functions interact with variables
- [Pattern Matching](../advanced/patterns.md) - Advanced pattern binding