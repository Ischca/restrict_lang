# Affine Types

Affine types are the cornerstone of Restrict Language's memory safety and resource management. In an affine type system, every value must be used **at most once**. This constraint might seem limiting at first, but it provides powerful guarantees and encourages clean, efficient code.

## What Are Affine Types?

In type theory, affine types sit between:
- **Linear types**: Must be used exactly once
- **Unrestricted types**: Can be used any number of times

Restrict Language uses affine types, meaning values can be used **zero or one time**, but not more.

## Basic Rules

### Rule 1: Single Use

Once a value is used, it cannot be used again:

```restrict
val x = 42
val y = x    // x is moved to y
// val z = x // Error: x was already used!
```

### Rule 2: Functions Consume Arguments

When you pass a value to a function, the function consumes it:

```restrict
fun consume = x:Int { x + 1 }

val num = 10
val result = num consume
// num consume  // Error: num already used!
```

### Rule 3: Mutable Variables Are Multi-Use

The `mut` keyword allows multiple uses:

```restrict
val mut counter = 0
counter = counter + 1  // First use
counter = counter + 2  // Second use - OK!
val final = counter    // Consumed here
```

## Why Affine Types?

### 1. Memory Safety Without GC

Affine types ensure that:
- No value is accidentally aliased
- Resources are cleaned up deterministically
- No garbage collector is needed

```restrict
val file = open_file("data.txt")
val contents = file read_all  // file is consumed
// file.close()  // Error: file already used!
// No need to worry - file is automatically closed
```

### 2. Prevent Common Bugs

Many bugs come from using values after they've been invalidated:

```restrict
val list = [1, 2, 3]
val sorted = list sort      // list is consumed
// val first = list[0]      // Error: prevents use-after-move
val first = sorted[0]       // OK: use the sorted version
```

### 3. Clear Data Flow

Affine types make data flow explicit:

```restrict
val data = fetch_data()
|> validate
|> transform
|> save  // Each step consumes the previous value
```

## Working With Affine Types

### Cloning When Needed

If you need to use a value multiple times, clone it explicitly:

```restrict
val original = ComplexData { /* ... */ }
val copy = original.clone()

process1(original)  // Consumes original
process2(copy)      // Uses the copy
```

### Borrowing Pattern

For read-only access without consuming, use accessor functions:

```restrict
record Person {
    name: String,
    age: Int,
}

impl Person {
    // Accessor doesn't consume self
    fun get_name = self:Person -> String {
        self.name.clone()  // Return a copy
    }
}

val person = Person { name: "Alice", age: 30 }
val name = person.get_name()  // Doesn't consume person
val age = person.age          // This consumes person
```

### Return Values

Functions can return values to "give them back":

```restrict
fun process_and_return = data:Data -> Data {
    // Process data...
    data  // Return it instead of consuming
}

val data = create_data()
val processed = data process_and_return
// processed is available for further use
```

## Advanced Patterns

### Affine Resources

Perfect for managing resources that should only be used once:

```restrict
record Token {
    value: String,
}

fun use_token = token:Token {
    // Token is consumed after use
    authenticate(token.value)
}

val token = Token { value: "secret" }
token use_token
// token use_token  // Error: prevents double-spending!
```

### State Machines

Affine types can encode state transitions:

```restrict
record ConnectionClosed { }
record ConnectionOpen { handle: Int }

fun connect = _:ConnectionClosed -> ConnectionOpen {
    ConnectionOpen { handle: establish_connection() }
}

fun send = conn:ConnectionOpen, data:String -> ConnectionOpen {
    // Send data...
    conn  // Return the connection for reuse
}

fun close = conn:ConnectionOpen -> ConnectionClosed {
    close_handle(conn.handle)
    ConnectionClosed { }
}

// Usage enforces correct state transitions
val conn = ConnectionClosed { } connect
val conn = (conn, "Hello") send
val closed = conn close
// Can't send after close!
```

## Best Practices

1. **Embrace the constraint** - Design APIs that work naturally with single-use values
2. **Use `mut` sparingly** - Only when you truly need multiple uses
3. **Clone explicitly** - Make copying intentional and visible
4. **Return what you don't consume** - Functions should return values they don't fully process
5. **Let the compiler guide you** - Type errors often reveal design improvements

## Common Misconceptions

### "It's Too Restrictive"

Affine types don't limit what you can express, they just require you to be explicit about resource usage. Most values in real programs are naturally used only once.

### "I Need to Clone Everything"

In practice, cloning is rare. Good API design and proper use of mutable bindings eliminate most needs for cloning.

### "It's Like Rust's Ownership"

While similar, Restrict Language's affine types are simpler:
- No borrowing or lifetimes
- No distinction between moves and copies
- Mutable variables can be used multiple times

## See Also

- [Variables and Mutability](variables.md) - How affine types affect variable usage
- [Memory Management](../advanced/memory.md) - Arena allocation and affine types
- [Resource Management](../patterns/resources.md) - Patterns for managing resources