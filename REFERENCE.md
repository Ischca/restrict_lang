# Restrict Language Reference

## Table of Contents

1. [Overview](#overview)
2. [Language Philosophy](#language-philosophy)
3. [Syntax and Grammar](#syntax-and-grammar)
4. [Type System](#type-system)
5. [Functions and Lambdas](#functions-and-lambdas)
6. [Pattern Matching](#pattern-matching)
7. [Memory Management](#memory-management)
8. [Control Flow](#control-flow)
9. [Records and Objects](#records-and-objects)
10. [Lists and Arrays](#lists-and-arrays)
11. [Error Handling](#error-handling)
12. [Examples](#examples)
13. [WebAssembly Backend](#webassembly-backend)

## Overview

Restrict Language is a statically-typed functional programming language that compiles to WebAssembly. It features an affine type system, pattern matching, lambda expressions with closures, and arena-based memory management.

### Key Features

- **Affine Type System**: Variables can be used at most once (except mutable variables)
- **Arena Memory Management**: No garbage collection, deterministic memory usage
- **Pattern Matching**: Exhaustive pattern matching with type safety
- **Lambda Expressions**: First-class functions with closure capture
- **WebAssembly Target**: Compiles to efficient WebAssembly code
- **OSV Syntax**: Object-Subject-Verb syntax for function calls

## Language Philosophy

Restrict Language is designed with the following principles:

1. **Resource Awareness**: The affine type system prevents accidental resource duplication
2. **Memory Safety**: Arena allocation provides memory safety without garbage collection
3. **Predictable Performance**: Compile-time memory layout and no hidden allocations
4. **Functional Programming**: Immutability by default with controlled mutation
5. **Type Safety**: Static typing with type inference where possible

## Syntax and Grammar

### Comments

```rust
// Single line comment

/* Multi-line
   comment */

val x = 42;  // Inline comment
```

### Variable Declarations

```rust
// Immutable binding
val x = 42

// Mutable binding
val mut y = 10
y = 20  // Reassignment

// Type annotations
val z: Int = 42
val name: String = "hello"
```

### Function Declarations

```rust
// Simple function
fun add = x:Int, y:Int { x + y }

// Function with explicit return type
fun multiply = x:Int, y:Int -> Int { x * y }

// Zero-parameter function
fun get_answer = { 42 }

// Function with block body
fun complex_calculation = x:Int {
    val doubled = x * 2;
    val result = doubled + 1;
    result
}
```

### Lambda Expressions

```rust
// Simple lambda
val double = |x| x * 2

// Multi-parameter lambda
val add = |x, y| x + y

// Lambda with block body
val complex = |x| {
    val temp = x * 2;
    temp + 1
}

// Nested lambdas (currying)
val curry_add = |x| |y| x + y
```

### Function Calls

Restrict Language uses Object-Subject-Verb (OSV) syntax for function calls:

```rust
// Traditional call: f(x, y)
// OSV syntax: (x, y) f

val result = (5, 10) add      // Calls add(5, 10)
val doubled = (21) double     // Calls double(21)
val answer = () get_answer    // Calls get_answer()

// Can also use traditional syntax
val result2 = add(5, 10)
```

## Type System

### Primitive Types

```rust
Int32     // 32-bit signed integer
Float64   // 64-bit floating point
Boolean   // true or false
String    // String literals
Char      // Single character
Unit      // Empty type ()
```

### Collection Types

```rust
List<T>      // Dynamic list with header (length + capacity)
Array<T, N>  // Fixed-size array without header
Option<T>    // Optional values (Some(value) or None)
```

### Function Types

```rust
Int -> Int           // Function taking Int, returning Int
Int, Int -> Int      // Function taking two Ints, returning Int
() -> Int            // Function taking no parameters, returning Int
```

### Record Types

```rust
record Person {
    name: String,
    age: Int,
}

// Frozen records (immutable after creation)
record Point {
    x: Int,
    y: Int,
} frozen
```

### Affine Type System

Variables can be used at most once (linear types), except:

```rust
val x = 42
val y = x    // x is consumed here
// val z = x // Error: x already used

// Mutable variables can be used multiple times
val mut counter = 0
counter = counter + 1  // OK
counter = counter + 1  // OK
```

## Functions and Lambdas

### Function Definition

```rust
fun factorial = n:Int -> Int {
    n match {
        0 => { 1 }
        _ => { n * factorial(n - 1) }
    }
}
```

### Lambda Expressions and Closures

```rust
fun make_adder = n:Int {
    |x| x + n  // Captures 'n' from environment
}

val add5 = make_adder(5)
val result = (10) add5  // Returns 15
```

### Type Inference

The compiler uses bidirectional type checking for better inference:

```rust
// Parameter type inferred from usage
val is_positive = |x| x > 0  // x inferred as Int32

// Type inferred from function signature
fun apply_int_func = f:Int->Int, x:Int { (x) f }
val double = |x| x * 2  // x inferred as Int32 when passed to apply_int_func
```

### Higher-Order Functions

```rust
fun map = list:List<T>, f:T->U -> List<U> {
    // Map implementation
}

fun filter = list:List<T>, predicate:T->Boolean -> List<T> {
    // Filter implementation
}
```

## Pattern Matching

### Basic Pattern Matching

```rust
val x = 42
x match {
    0 => { "zero" }
    1 => { "one" }
    _ => { "other" }
}
```

### Option Pattern Matching

```rust
val maybe_value: Option<Int> = Some(42)
maybe_value match {
    Some(value) => { value * 2 }
    None => { 0 }
}
```

### List Pattern Matching

```rust
val numbers = [1, 2, 3, 4]
numbers match {
    [] => { "empty" }
    [head | tail] => { "head: " + head }
    [a, b] => { "exactly two elements" }
    [a, b, c | rest] => { "at least three elements" }
}
```

### Record Pattern Matching

```rust
record Point { x: Int, y: Int }

val point = Point { x: 10, y: 20 }
point match {
    Point { x: 0, y: 0 } => { "origin" }
    Point { x, y } => { "point at " + x + ", " + y }
}
```

## Memory Management

### Arena Allocation

```rust
// Create a new arena
val arena = new_arena(1024)  // 1KB arena

// Use arena for allocations
arena {
    val list = [1, 2, 3, 4]  // Allocated in arena
    val record = Person { name: "Alice", age: 30 }
    // All allocations freed when arena scope ends
}
```

### Default Arena

```rust
// Set default arena for automatic allocation
use_default_arena(2048)

val global_list = [1, 2, 3]  // Uses default arena
```

### Memory Layout

- **Lists**: Header (8 bytes) + Elements
  - Header: [length: i32][capacity: i32]
  - Elements: Contiguous array of values

- **Arrays**: Direct element storage (no header)
  - Fixed size known at compile time

- **Records**: Contiguous field storage
  - Fields stored in declaration order

## Control Flow

### Conditional Expressions

```rust
// If-then-else
val result = if condition { value1 } else { value2 }

// Pattern-based conditionals
val status = user match {
    Some(u) => { "logged in as " + u.name }
    None => { "not logged in" }
}
```

### Loops

```rust
// While loops
val mut i = 0
while i < 10 {
    // loop body
    i = i + 1
}

// Recursive functions for iteration
fun count_down = n:Int {
    n match {
        0 => { "done" }
        _ => { count_down(n - 1) }
    }
}
```

### Pipe Operations

```rust
// Data pipeline
val result = data
    |> filter(is_positive)
    |> map(double)
    |> take(10)

// Mutable pipe (|>>)
val mut accumulator = 0
values |>> accumulator = accumulator + _
```

## Records and Objects

### Record Definition

```rust
record User {
    id: Int,
    name: String,
    email: String,
}
```

### Record Creation

```rust
val user = User {
    id: 1,
    name: "Alice",
    email: "alice@example.com"
}
```

### Record Methods

```rust
impl User {
    fun get_display_name = self:User {
        self.name + " <" + self.email + ">"
    }
    
    fun update_email = self:User, new_email:String {
        User { id: self.id, name: self.name, email: new_email }
    }
}

// Method calls
val display = user.get_display_name()
val updated = user.update_email("newemail@example.com")
```

### Frozen Records

```rust
record ImmutablePoint { x: Int, y: Int } frozen

val point = ImmutablePoint { x: 10, y: 20 }
// val frozen_point = freeze(point)  // Convert to frozen
```

### Record Cloning

```rust
val user2 = clone(user)  // Deep copy
val modified = User { ...user, name: "Bob" }  // Update syntax
```

## Lists and Arrays

### List Operations

```rust
// List creation
val numbers = [1, 2, 3, 4, 5]
val empty: List<Int> = []

// List operations
val head = numbers[0]           // First element
val tail = numbers[1..]         // All but first
val length = len(numbers)       // Get length
val appended = numbers + [6, 7] // Concatenation
```

### Array Operations

```rust
// Fixed-size arrays
val coordinates: Array<Int, 3> = [|10, 20, 30|]

// Array access
val x = coordinates[0]
val y = coordinates[1]
val z = coordinates[2]
```

### List Comprehensions

```rust
// Map operation
val doubled = [x * 2 | x <- numbers]

// Filter operation  
val evens = [x | x <- numbers, x % 2 == 0]

// Complex transformations
val results = [
    process(x) 
    | x <- input_data, 
      is_valid(x),
      x > threshold
]
```

## Error Handling

### Option Types

```rust
fun safe_divide = x:Int, y:Int -> Option<Int> {
    y match {
        0 => { None }
        _ => { Some(x / y) }
    }
}

val result = safe_divide(10, 2) match {
    Some(value) => { "Result: " + value }
    None => { "Division by zero" }
}
```

### Result Types (Future Feature)

```rust
// Planned for future versions
Result<T, E>  // Either Ok(T) or Err(E)
```

### Error Propagation

```rust
fun chain_operations = input:Int -> Option<Int> {
    safe_divide(input, 2)
        .map(|x| x * 3)
        .filter(|x| x > 0)
}
```

## Examples

### Complete Program Examples

#### Factorial Calculator

```rust
// Recursive factorial
fun factorial = n:Int -> Int {
    n match {
        0 => { 1 }
        1 => { 1 }
        _ => { n * factorial(n - 1) }
    }
}

fun main = {
    val result = factorial(5);
    result  // Returns 120
}
```

#### List Processing

```rust
// Filter and map operations
fun process_numbers = numbers:List<Int> -> List<Int> {
    numbers
        |> filter(|x| x > 0)        // Keep positive numbers
        |> map(|x| x * x)           // Square each number
        |> take(10)                 // Take first 10
}

fun main = {
    val input = [1, -2, 3, -4, 5, 6, 7, 8, 9, 10];
    val result = process_numbers(input);
    result
}
```

#### User Management System

```rust
record User {
    id: Int,
    name: String,
    email: String,
    active: Boolean,
}

impl User {
    fun is_valid = self:User -> Boolean {
        len(self.name) > 0 && len(self.email) > 5
    }
    
    fun activate = self:User -> User {
        User { ...self, active: true }
    }
}

fun find_user = users:List<User>, id:Int -> Option<User> {
    users |> filter(|u| u.id == id) |> first
}

fun main = {
    val users = [
        User { id: 1, name: "Alice", email: "alice@example.com", active: false },
        User { id: 2, name: "Bob", email: "bob@example.com", active: true }
    ];
    
    val user = find_user(users, 1) match {
        Some(u) => { u.activate() }
        None => { User { id: 0, name: "", email: "", active: false } }
    };
    
    user
}
```

#### Lambda and Closure Examples

```rust
// Higher-order function example
fun make_counter = start:Int -> () -> Int {
    val mut count = start;
    || {
        count = count + 1;
        count
    }
}

fun apply_twice = f:Int->Int, x:Int -> Int {
    val once = (x) f;
    (once) f
}

fun main = {
    // Counter with closure
    val counter = make_counter(0);
    val first = counter();   // Returns 1
    val second = counter();  // Returns 2
    
    // Function composition
    val double = |x| x * 2;
    val result = apply_twice(double, 5);  // Returns 20
    
    result
}
```

## WebAssembly Backend

### Compilation Process

1. **Lexical Analysis**: Source code → Tokens
2. **Parsing**: Tokens → Abstract Syntax Tree (AST)
3. **Type Checking**: AST → Type-checked AST
4. **Code Generation**: Type-checked AST → WebAssembly Text (WAT)

### Generated WebAssembly Features

#### Memory Layout

```wat
(module
  (memory 1)  ;; 64KB pages
  
  ;; String constants
  (data (i32.const 1024) "hello world")
  
  ;; Arena management
  (global $arena_start (mut i32) (i32.const 32768))
  (global $arena_end (mut i32) (i32.const 65536))
```

#### Function Generation

```wat
;; Simple function
(func $add (param $x i32) (param $y i32) (result i32)
  local.get $x
  local.get $y
  i32.add
)

;; Lambda with closure
(func $lambda_0 (param $closure i32) (param $x i32) (result i32)
  ;; Load captured variable from closure
  local.get $closure
  i32.const 4
  i32.add
  i32.load
  
  ;; Add to parameter
  local.get $x
  i32.add
)
```

#### Function Tables for Lambdas

```wat
;; Function table for indirect calls
(table 10 funcref)
(elem (i32.const 0) $lambda_0 $lambda_1)

;; Indirect call
(func $call_lambda (param $table_index i32) (param $arg i32) (result i32)
  local.get $arg
  local.get $table_index
  call_indirect (type $lambda_type)
)
```

### Runtime System

#### Memory Management

- **Stack**: Local variables and function call frames
- **Heap**: Arena-allocated objects (lists, records, closures)
- **Constants**: String literals and other constants

#### Type Representations

- **Int32**: Direct i32 values
- **Boolean**: i32 (0 = false, 1 = true)
- **Lists**: Pointer to [length][capacity][elements...]
- **Records**: Pointer to [field1][field2]...[fieldN]
- **Lambdas**: Function table index + optional closure pointer

### Performance Characteristics

- **Function Calls**: Direct calls compile to `call` instructions
- **Lambda Calls**: Indirect calls through function table
- **Memory Access**: Linear memory with bounds checking
- **Arithmetic**: Native WebAssembly integer/float operations

### WASI Integration

```wat
;; WASI imports for I/O
(import "wasi_snapshot_preview1" "fd_write" 
  (func $fd_write (param i32 i32 i32 i32) (result i32)))
(import "wasi_snapshot_preview1" "proc_exit" 
  (func $proc_exit (param i32)))
```

## Future Features

### Planned Additions

1. **Async/Await**: Asynchronous programming support
2. **Modules**: Module system for code organization  
3. **Generics**: Parametric polymorphism
4. **Traits**: Interface-like abstractions
5. **Error Types**: Result<T, E> for error handling
6. **String Interpolation**: Template string syntax
7. **Destructuring**: Advanced pattern matching
8. **Type Classes**: Haskell-style type classes

### Experimental Features

1. **Dependent Types**: Types that depend on values
2. **Effect System**: Tracking side effects in types
3. **Linear Types**: More sophisticated resource tracking
4. **Compile-time Evaluation**: Constant folding and evaluation

---

*This reference document covers Restrict Language v0.1. For the latest updates and examples, see the project repository.*