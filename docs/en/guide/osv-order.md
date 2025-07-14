# OSV Word Order

One of Restrict Language's most distinctive features is its Object-Subject-Verb (OSV) word order, inspired by Japanese grammar. This design choice creates a natural flow for functional programming patterns and makes chaining operations intuitive.

## Understanding OSV

In traditional programming languages, we typically see Subject-Verb-Object (SVO) ordering:

```rust
// Traditional SVO: subject.verb(object)
console.log("Hello, World!");
list.map(fn);
user.getName();
```

Restrict Language uses OSV ordering instead:

```restrict
// OSV: object subject.verb
"Hello, World!" |> println
fn |> list.map
user |> getName
```

## The Pipe Operator `|>`

The pipe operator `|>` is the heart of OSV syntax. It takes the value on the left and passes it as the first argument to the function on the right.

```restrict
// Basic piping
42 |> toString          // Convert 42 to string
"hello" |> toUpperCase  // Convert to "HELLO"

// Chaining operations
"hello"
    |> toUpperCase      // "HELLO"
    |> reverse          // "OLLEH"
    |> println         // Prints: OLLEH
```

## Why OSV?

### 1. Natural Data Flow

OSV makes the flow of data through your program visible:

```restrict
// Data flows from left to right
rawData
    |> parse
    |> validate
    |> transform
    |> save
```

Compare with traditional style:
```rust
// Data flow is less clear
save(transform(validate(parse(rawData))));
```

### 2. Better Readability

OSV syntax reads like a sequence of transformations:

```restrict
// Read as: "Take users, filter active ones, map to names, then join"
users
    |> filter(u => u.isActive)
    |> map(u => u.name)
    |> join(", ")
```

### 3. Affine Type Clarity

With affine types, OSV makes ownership transfer explicit:

```restrict
let data = getData()
data |> process  // data ownership transferred to process
// data is no longer available here
```

## Method Calls vs Functions

Restrict distinguishes between methods (bound to objects) and free functions:

```restrict
// Method call - still uses dot notation
let user = User { name: "Alice", age: 30 }
let name = user.name  // Accessing field

// But method invocation uses pipes
user |> getName       // Calling method
user |> setAge(31)    // Method with argument
```

## Multiple Arguments

For functions with multiple arguments, use tuples or the special `|>>` operator:

```restrict
// Using tuples
(5, 3) |> add  // add(5, 3)

// Using |>> for curried application
5 |>> add(3)   // Partial application

// Multiple arguments with names
{ x: 10, y: 20 } |> drawPoint
```

## Pattern Matching in Pipes

OSV works seamlessly with pattern matching:

```restrict
result
    |> match {
        Ok(value) => value |> process,
        Err(error) => error |> logError
    }
```

## List Operations

OSV shines with list operations:

```restrict
[1, 2, 3, 4, 5]
    |> filter(n => n % 2 == 0)  // [2, 4]
    |> map(n => n * n)          // [4, 16]
    |> sum                     // 20
```

## Building Pipelines

You can create reusable pipelines:

```restrict
// Define a processing pipeline
let processUser = fn(user) => {
    user
        |> validate
        |> normalize
        |> enrichWithDefaults
}

// Use it
newUser |> processUser |> save
```

## OSV with Blocks

Blocks can be used in pipes for more complex operations:

```restrict
data |> {
    // Complex transformation
    let processed = _ |> clean
    let validated = processed |> validate
    validated |> format
}
```

## Common Patterns

### Transform and Filter
```restrict
items
    |> filter(item => item.price < 100)
    |> map(item => {
        name: item.name,
        discountedPrice: item.price * 0.9
    })
    |> sortBy(item => item.discountedPrice)
```

### Error Handling
```restrict
readFile("data.txt")
    |> parseJson
    |> extractField("users")
    |> handleError(err => {
        err |> logError
        []  // Return empty list on error
    })
```

### Builder Pattern
```restrict
Request::new()
    |> setMethod("POST")
    |> setUrl("https://api.example.com")
    |> setHeader("Content-Type", "application/json")
    |> setBody(jsonData)
    |> send
```

## Best Practices

1. **Keep pipelines readable** - If a pipeline gets too long, consider breaking it into named steps
2. **Use meaningful function names** - Since data flow is explicit, function names should describe transformations
3. **Avoid side effects in the middle** - Keep side effects at the end of pipelines
4. **Leverage type inference** - The compiler can often infer types through pipeline transformations

## Comparison with Other Languages

```restrict
// Restrict Language (OSV)
data |> process |> save

// F# and Elixir (similar piping)
data |> process |> save

// JavaScript (proposed pipe operator)
data |> process(%) |> save(%)

// Haskell
save $ process data

// Traditional OOP
save(process(data))
```

## Exercises

Try converting these traditional expressions to OSV:

1. `console.log(user.getName().toUpperCase())`
2. `array.filter(x => x > 0).map(x => x * 2).reduce((a, b) => a + b, 0)`
3. `validateEmail(normalizeString(trimWhitespace(input)))`

<details>
<summary>Solutions</summary>

```restrict
// 1.
user |> getName |> toUpperCase |> console.log

// 2.
array
    |> filter(x => x > 0)
    |> map(x => x * 2)
    |> reduce((a, b) => a + b, 0)

// 3.
input
    |> trimWhitespace
    |> normalizeString
    |> validateEmail
```
</details>

## Summary

OSV word order with the pipe operator creates a unique and powerful programming style that:
- Makes data flow explicit and visual
- Reduces nesting and improves readability
- Works naturally with functional programming patterns
- Clarifies ownership transfer in affine type systems

As you continue learning Restrict Language, you'll find that OSV becomes second nature and helps you write cleaner, more maintainable code.