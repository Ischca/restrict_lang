# Hello World

Let's write your first Restrict Language program! This tutorial will guide you through creating, compiling, and running a simple "Hello World" application.

## Creating Your First Project

The easiest way to start is using Warder, the Restrict Language package manager:

```bash
# Create a new project
warder new hello-world
cd hello-world
```

This creates a project structure:

```
hello-world/
├── package.rl.toml    # Project manifest
├── src/
│   └── main.rl       # Main source file
├── tests/            # Test directory
│   └── main_test.rl  # Example test
└── README.md         # Project documentation
```

## The Hello World Program

Open `src/main.rl` in your editor. You'll see a basic template:

```restrict
// Welcome to Restrict Language!

fun main = {
    "Hello, World!" |> println
}
```

Let's understand this code:

- `fun main =` declares the main function, the entry point of your program
- `"Hello, World!"` is a string literal
- `|>` is the pipe operator, passing the value to the next function
- `println` prints the value to standard output

## Understanding OSV Syntax

Restrict Language uses Object-Subject-Verb (OSV) word order. Compare with traditional syntax:

```restrict
// Traditional (SVO): subject.verb(object)
println("Hello, World!")

// Restrict (OSV): object |> subject.verb
"Hello, World!" |> println
```

The pipe operator `|>` chains operations naturally:

```restrict
// Chain multiple operations
"hello"
    |> to_uppercase    // "HELLO"
    |> concat(" WORLD") // "HELLO WORLD"
    |> println         // Prints: HELLO WORLD
```

## Building Your Program

Build the project using Warder:

```bash
warder build
```

This compiles your Restrict code to WebAssembly and creates a cage file in the `dist/` directory:

```
Building project...
✓ Built hello-world v0.1.0 → dist/hello-world-0.1.0.rgc
ℹ Cage size: 12 KB (ABI hash: a3f2c8d9)
```

## Running Your Program

Run the compiled program:

```bash
warder run
```

You should see:

```
Hello, World!
```

You can also run the WebAssembly file directly with a runtime:

```bash
# Using wasmtime
wasmtime dist/hello-world-0.1.0.wasm

# Using wasmer
wasmer dist/hello-world-0.1.0.wasm
```

## Making It Interactive

Let's make the program more interactive by reading user input:

```restrict
import std.io.{stdin, print}

fun main = {
    "What's your name? " |> print
    
    let name = stdin.read_line()
    let greeting = "Hello, " + name + "!"
    
    greeting |> println
}
```

Key concepts:
- `import` brings functions from the standard library
- `let` creates an immutable binding
- `+` concatenates strings
- Values flow through the program via ownership transfer

## Working with Affine Types

Restrict Language's affine type system ensures each value is used at most once:

```restrict
fun main = {
    let message = "Hello, World!"
    
    // First use - ownership is transferred
    message |> println
    
    // This would cause a compile error:
    // message |> println  // Error: value used after move
}
```

To use a value multiple times, explicitly clone it:

```restrict
fun main = {
    let original = "Hello, World!"
    
    // Clone the value
    let copy = original |> clone
    
    // Now we can use both
    original |> println
    copy |> println
}
```

## Adding Functions

Create reusable functions:

```restrict
// Function that consumes its argument
fun greet(name: String) -> String = {
    "Hello, " + name + "!"
}

// Function with multiple parameters
fun greet_formal(title: String, name: String) -> String = {
    "Good day, " + title + " " + name
}

fun main = {
    "Alice" |> greet |> println
    
    // With multiple arguments
    ("Dr.", "Smith") |>> greet_formal |> println
}
```

Note the `|>>` operator for functions with multiple parameters.

## Error Handling

Restrict uses explicit error handling:

```restrict
import std.result.{Result, Ok, Err}

fun divide(a: i32, b: i32) -> Result<i32, String> = {
    if b == 0 {
        Err("Division by zero")
    } else {
        Ok(a / b)
    }
}

fun main = {
    match divide(10, 2) {
        Ok(result) => result |> println,
        Err(msg) => msg |> println
    }
}
```

## Testing Your Code

Write tests in `tests/main_test.rl`:

```restrict
import std.test.{assert, assert_eq}

test "greeting function works" = {
    let result = "World" |> greet
    assert_eq(result, "Hello, World!")
}

test "math operations" = {
    assert_eq(2 + 2, 4)
    assert(5 > 3)
}
```

Run tests with:

```bash
warder test
```

## Next Steps

Congratulations! You've written your first Restrict Language program. You've learned:

- ✅ Creating projects with Warder
- ✅ OSV syntax and pipe operators
- ✅ Affine types and ownership
- ✅ Functions and error handling
- ✅ Testing your code

Continue learning with:
- [Warder Package Manager](./warder.md) - Managing dependencies
- [Language Guide](../guide/syntax.md) - Deep dive into syntax
- [Standard Library](../std/overview.md) - Available modules and functions

## Complete Example

Here's a complete program showcasing various features:

```restrict
import std.io.{stdin, print}
import std.list.{map, filter, fold}

// Custom type
type Person = {
    name: String,
    age: i32
}

// Function with pattern matching
fun categorize_age(age: i32) -> String = {
    match age {
        0..12 => "child",
        13..19 => "teenager",
        20..59 => "adult",
        _ => "senior"
    }
}

fun main = {
    // Get user input
    "Enter your name: " |> print
    let name = stdin.read_line()
    
    "Enter your age: " |> print
    let age = stdin.read_line() |> parse_int
    
    // Create person
    let person = Person { name, age }
    
    // Display category
    let category = person.age |> categorize_age
    ("Hello, " + person.name + "! You are a " + category) |> println
    
    // Process a list
    [1, 2, 3, 4, 5]
        |> map(x => x * x)           // Square each number
        |> filter(x => x % 2 == 1)   // Keep odd numbers
        |> fold(0, (a, b) => a + b)  // Sum them up
        |> println                   // Print result
}
```

Happy coding with Restrict Language! 🚀