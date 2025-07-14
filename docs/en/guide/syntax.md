# Syntax Reference

This guide covers the complete syntax of Restrict Language, from basic expressions to advanced features.

## Comments

```restrict
// Single-line comment

/* 
   Multi-line comment
   Can span multiple lines
*/

/// Documentation comment for the following item
/// Supports markdown formatting
fn documented_function() { ... }
```

## Identifiers and Keywords

### Identifiers

Identifiers must start with a letter or underscore, followed by letters, digits, or underscores:

```restrict
let valid_name = 1;
let _private = 2;
let camelCase = 3;
let snake_case = 4;
let number123 = 5;
```

### Keywords

The following are reserved keywords:

```
let mut fn type struct enum match if else while for 
loop break continue return clone freeze derive from
with as impl trait pub mod use import export true false
```

## Literals

### Numbers

```restrict
// Integers
let decimal = 42;
let hex = 0xFF;
let octal = 0o77;
let binary = 0b1010;
let with_underscores = 1_000_000;

// Floating point
let float = 3.14;
let scientific = 2.5e-10;
```

### Strings

```restrict
// String literals
let simple = "Hello, World!";
let escaped = "Line 1\nLine 2\tTabbed";
let unicode = "Unicode: \u{1F44B}";

// Raw strings
let raw = r"No escapes\n here";
let raw_hashes = r#"Can contain "quotes""#;

// Multiline strings
let multiline = """
    This is a
    multiline string
    with preserved formatting
""";
```

### Characters

```restrict
let ch = 'a';
let unicode_ch = '🦀';
let escaped_ch = '\n';
```

### Booleans

```restrict
let yes = true;
let no = false;
```

## Variables and Bindings

### Immutable Bindings

```restrict
let x = 42;          // Type inference
let y: i32 = 42;     // Explicit type
let (a, b) = (1, 2); // Pattern destructuring
```

### Mutable Bindings

```restrict
let mut counter = 0;
counter = counter + 1;  // Mutation allowed

// Mutable pipe operator
let mut data = getData();
data |>> process;  // In-place mutation
```

## Expressions

### Arithmetic

```restrict
let sum = 1 + 2;
let difference = 5 - 3;
let product = 4 * 3;
let quotient = 10 / 2;
let remainder = 7 % 3;
let power = 2 ** 8;
```

### Comparison

```restrict
let equal = x == y;
let not_equal = x != y;
let less = x < y;
let greater = x > y;
let less_eq = x <= y;
let greater_eq = x >= y;
```

### Logical

```restrict
let and_result = true && false;
let or_result = true || false;
let not_result = !true;
```

### Bitwise

```restrict
let bit_and = 0b1100 & 0b1010;  // 0b1000
let bit_or = 0b1100 | 0b1010;   // 0b1110
let bit_xor = 0b1100 ^ 0b1010;  // 0b0110
let bit_not = ~0b1010;           // Bitwise NOT
let shift_left = 1 << 3;         // 8
let shift_right = 8 >> 2;        // 2
```

## Control Flow

### If Expressions

```restrict
// Basic if
if condition {
    doSomething()
}

// If-else
let result = if x > 0 {
    "positive"
} else if x < 0 {
    "negative"
} else {
    "zero"
};

// Pattern matching in conditions
if let Some(value) = optional {
    value |> process
}
```

### Match Expressions

```restrict
// Basic match
let description = match number {
    0 => "zero",
    1 => "one",
    2..=5 => "two to five",
    _ => "other"
};

// Pattern matching with guards
match value {
    Some(x) if x > 0 => x |> process,
    Some(x) => x |> handleNegative,
    None => defaultValue()
}

// Destructuring in patterns
match point {
    { x: 0, y: 0 } => "origin",
    { x: 0, y } => "on y-axis at " ++ y.toString(),
    { x, y: 0 } => "on x-axis at " ++ x.toString(),
    { x, y } => "at (" ++ x.toString() ++ ", " ++ y.toString() ++ ")"
}
```

### Loops

```restrict
// While loop
while condition {
    doWork()
}

// For loop over range
for i in 0..10 {
    i |> println
}

// For loop over collection
for item in list {
    item |> process
}

// Loop with break
loop {
    if done() {
        break;
    }
    continue;
}

// Loop labels
'outer: loop {
    'inner: loop {
        if condition {
            break 'outer;
        }
    }
}
```

## Functions

### Function Definitions

```restrict
// Basic function
fn add(x: i32, y: i32) -> i32 {
    x + y
}

// Generic function
fn identity<T>(value: T) -> T {
    value
}

// Function with where clause
fn process<T>(data: T) -> String 
    where T: ToString
{
    data.toString()
}

// OSV-style function calls
42 |> add(10);  // add(42, 10)
"hello" |> process;
```

### Lambda Expressions

```restrict
// Simple lambda
let add_one = |x| x + 1;

// With type annotations
let multiply: fn(i32, i32) -> i32 = |x, y| x * y;

// Capturing variables
let factor = 10;
let scale = |x| x * factor;

// In higher-order functions
list |> map(|x| x * 2) |> filter(|x| x > 10);
```

## Types

### Primitive Types

```restrict
// Integers
i8, i16, i32, i64, i128
u8, u16, u32, u64, u128

// Floating point
f32, f64

// Boolean
bool

// Character
char

// String (affine type)
String
```

### Compound Types

```restrict
// Arrays (fixed size)
let array: [i32; 5] = [1, 2, 3, 4, 5];

// Slices (view into array)
let slice: &[i32] = &array[1..4];

// Tuples
let tuple: (i32, String, bool) = (42, "hello", true);
let (x, y, z) = tuple;  // Destructuring

// Option type
let some_value: Option<i32> = Some(42);
let no_value: Option<i32> = None;

// Result type
let success: Result<i32, String> = Ok(42);
let failure: Result<i32, String> = Err("error message");
```

### Custom Types

```restrict
// Structs
struct Point {
    x: f64,
    y: f64
}

// Tuple structs
struct Color(u8, u8, u8);

// Enums
enum Status {
    Active,
    Inactive,
    Pending { since: DateTime }
}

// Type aliases
type Distance = f64;
type Callback = fn(Event) -> bool;
```

## Pattern Matching

### Patterns

```restrict
// Literal patterns
match x {
    0 => "zero",
    1 => "one",
    _ => "other"
}

// Variable patterns
let Some(value) = optional;

// Wildcard pattern
let (first, _, third) = triple;

// Range patterns
match score {
    0..=59 => "F",
    60..=69 => "D",
    70..=79 => "C",
    80..=89 => "B",
    90..=100 => "A",
    _ => "Invalid"
}

// Struct patterns
let Point { x, y } = point;
let Point { x: px, y: py } = point;  // Rename

// Guard clauses
match value {
    Some(x) if x > 0 => "positive",
    Some(x) if x < 0 => "negative",
    Some(_) => "zero",
    None => "none"
}
```

## Modules and Imports

```restrict
// Module definition
mod math {
    pub fn add(x: i32, y: i32) -> i32 {
        x + y
    }
    
    pub mod advanced {
        pub fn pow(base: f64, exp: f64) -> f64 {
            base ** exp
        }
    }
}

// Imports
use std::collections::List;
use math::add;
use math::advanced::pow;

// Import with alias
use std::string::String as Str;

// Glob imports
use std::prelude::*;
```

## Attributes

```restrict
// Function attributes
#[inline]
fn fast_function() { ... }

#[test]
fn test_addition() {
    assert_eq!(2 + 2, 4);
}

// Derive attributes
#[derive(Debug, Clone)]
struct Point { x: f64, y: f64 }

// Module attributes
#[cfg(test)]
mod tests {
    // Test module
}
```

## Special Syntax

### With Blocks (Resource Management)

```restrict
with file = openFile("data.txt") {
    file |> readContents |> process;
}  // file automatically closed

with db = connectDatabase(url) {
    db |> query("SELECT * FROM users");
}  // connection automatically closed
```

### Clone and Freeze

```restrict
// Clone creates a mutable copy
let original = { x: 10, y: 20 };
let mut copy = clone original;
copy.x = 30;  // OK

// Freeze creates an immutable prototype
let prototype = freeze { x: 10, y: 20 };
let instance = clone prototype;
// prototype cannot be modified
```

### Derivation Bounds

```restrict
// Generic with derivation bound
fn process<T from Base>(value: T) -> Result<String> {
    // T must be derived from Base prototype
    value |> validate |> transform
}
```

## Operator Precedence

1. Member access: `.`
2. Function calls, array indexing
3. Unary: `-`, `!`, `~`
4. Power: `**`
5. Multiplicative: `*`, `/`, `%`
6. Additive: `+`, `-`
7. Shift: `<<`, `>>`
8. Bitwise AND: `&`
9. Bitwise XOR: `^`
10. Bitwise OR: `|`
11. Comparison: `<`, `>`, `<=`, `>=`
12. Equality: `==`, `!=`
13. Logical AND: `&&`
14. Logical OR: `||`
15. Range: `..`, `..=`
16. Assignment: `=`
17. Pipe: `|>`, `|>>`

## Summary

This syntax reference covers the essential elements of Restrict Language. The syntax is designed to be:

- **Familiar** to Rust programmers
- **Natural** with OSV word order
- **Safe** with affine types
- **Expressive** for functional programming

For more detailed examples and patterns, see the [Language Guide](./README.md).