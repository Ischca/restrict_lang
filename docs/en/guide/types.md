# Type System

Restrict Language features a sophisticated type system that combines static typing, affine types for memory safety, and powerful type inference. This guide explores the type system in detail.

## Affine Types

The most distinctive feature of Restrict Language is its affine type system. An affine type ensures that values can be used **at most once**.

### What are Affine Types?

```restrict
let message = "Hello";
message |> println;     // Ownership transferred to println
// message |> println;  // ERROR: message already consumed
```

This prevents common bugs like:
- Use-after-free
- Double-free
- Data races

### When Types are Consumed

A value is consumed when:

1. **Passed to a function**
```restrict
let data = getData()
data |> process  // data consumed
// data is no longer available
```

2. **Assigned to another variable**
```restrict
let x = createResource()
let y = x  // x consumed
// x is no longer available
```

3. **Returned from a function**
```restrict
fn transfer(resource: Resource) -> Resource {
    resource  // Ownership transferred to caller
}
```

### Working with Affine Types

#### Cloning

When you need to use a value multiple times, use `clone`:

```restrict
let original = "Hello"
let copy = clone original

original |> println  // OK
copy |> println      // OK
```

#### Pattern Matching

Pattern matching with affine types:

```restrict
let result = compute();
match result {
    Ok(value) => value |> process,  // value consumed in this branch
    Err(error) => error |> logError  // error consumed in this branch
}
// result is fully consumed
```

## Primitive Types

### Numeric Types

```restrict
// Signed integers
let i8_val: i8 = -128
let i16_val: i16 = -32768
let i32_val: i32 = -2147483648
let i64_val: i64 = -9223372036854775808
let i128_val: i128 = -170141183460469231731687303715884105728

// Unsigned integers
let u8_val: u8 = 255
let u16_val: u16 = 65535
let u32_val: u32 = 4294967295
let u64_val: u64 = 18446744073709551615
let u128_val: u128 = 340282366920938463463374607431768211455

// Floating point
let f32_val: f32 = 3.14159
let f64_val: f64 = 2.718281828459045

// Platform-specific
let size: usize = 100  // Pointer-sized unsigned
let diff: isize = -50  // Pointer-sized signed
```

### Boolean Type

```restrict
let is_ready: bool = true
let is_finished: bool = false

// Boolean operations
let both = is_ready && is_finished
let either = is_ready || is_finished
let not_ready = !is_ready
```

### Character Type

```restrict
let letter: char = 'A'
let emoji: char = '😀'
let unicode: char = '\u{1F600}'
```

### Unit Type

The unit type `()` represents an empty value:

```restrict
fn do_nothing() -> () {
    // Returns unit
}

let unit_value: () = ()
```

## String Types

### String (Owned)

`String` is an affine type representing owned UTF-8 text:

```restrict
let mut greeting: String = "Hello"
greeting = greeting ++ ", World!"  // Concatenation

// String is consumed when used
greeting |> println
// greeting no longer available
```

### &str (String Slice)

String slices are borrowed views into strings:

```restrict
let full_name = "John Doe"
let first_name: &str = &full_name[0..4]  // "John"
```

## Compound Types

### Arrays

Fixed-size sequences of elements:

```restrict
let numbers: [i32; 5] = [1, 2, 3, 4, 5]
let zeros: [i32; 100] = [0; 100]  // 100 zeros

// Array access
let first = numbers[0]
let last = numbers[4]
```

### Slices

Dynamic views into arrays:

```restrict
let array = [1, 2, 3, 4, 5]
let slice: &[i32] = &array[1..4]  // [2, 3, 4]

// Slice operations
slice |> len      // 3
slice[0]          // 2
```

### Tuples

Fixed-size heterogeneous collections:

```restrict
let person: (String, i32, bool) = ("Alice", 30, true)
let (name, age, active) = person  // Destructuring

// Accessing tuple elements
let coordinates: (f64, f64) = (10.5, 20.7)
let x = coordinates.0
let y = coordinates.1
```

### Vectors

Dynamic arrays (affine type):

```restrict
let mut vec: Vec<i32> = Vec::new()
vec |>> push(1)
vec |>> push(2)
vec |>> push(3)

// Vector is consumed when iterated
vec |> iter |> map(|x| x * 2) |> collect
```

## Custom Types

### Structs

Named collections of fields:

```restrict
struct User {
    name: String,
    email: String,
    age: u32,
    active: bool
}

// Creating instances
let user = User {
    name: "Alice",
    email: "alice@example.com",
    age: 30,
    active: true
}

// Field access
let name = clone user.name  // Clone to avoid consuming user
```

### Tuple Structs

Structs with unnamed fields:

```restrict
struct Point(f64, f64)
struct Color(u8, u8, u8)

let origin = Point(0.0, 0.0)
let red = Color(255, 0, 0)

// Accessing fields
let x = origin.0
let r = red.0
```

### Enums

Sum types with variants:

```restrict
enum Result<T, E> {
    Ok(T),
    Err(E)
}

enum Message {
    Quit,
    Move { x: i32, y: i32 },
    Write(String),
    ChangeColor(u8, u8, u8)
}

// Pattern matching
let msg = Message::Move { x: 10, y: 20 }
match msg {
    Message::Quit => quit(),
    Message::Move { x, y } => moveTo(x, y),
    Message::Write(text) => text |> display,
    Message::ChangeColor(r, g, b) => setColor(r, g, b)
}
```

## Type Aliases

Create alternative names for types:

```restrict
type UserId = u64
type Result<T> = Result<T, String>
type Callback = fn(Event) -> bool

let id: UserId = 12345
let result: Result<i32> = Ok(42)
```

## Option and Result

### Option Type

Represents optional values:

```restrict
enum Option<T> {
    Some(T),
    None
}

// Using Option
let maybe_number: Option<i32> = Some(42)
let nothing: Option<i32> = None

// Pattern matching
match maybe_number {
    Some(n) => n |> process,
    None => handleMissing()
}

// Option methods
maybe_number |> map(|n| n * 2)
maybe_number |> unwrap_or(0)
```

### Result Type

Represents success or failure:

```restrict
enum Result<T, E> {
    Ok(T),
    Err(E)
}

// Using Result
let result: Result<i32, String> = Ok(42)
let error: Result<i32, String> = Err("Failed")

// Error handling
result
    |> map(|n| n * 2)
    |> map_err(|e| "Error: " ++ e)
    |> unwrap_or_else(|_| 0)
```

## Generic Types

### Generic Functions

```restrict
fn identity<T>(value: T) -> T {
    value
}

fn swap<A, B>(pair: (A, B)) -> (B, A) {
    let (a, b) = pair
    (b, a)
}
```

### Generic Structs

```restrict
struct Container<T> {
    value: T
}

impl<T> Container<T> {
    fn new(value: T) -> Container<T> {
        Container { value }
    }
    
    fn get(self) -> T {
        self.value  // Consumes container
    }
}
```

### Type Constraints

```restrict
fn display<T: ToString>(value: T) {
    value |> toString |> println
}

fn process<T>(items: Vec<T>) -> Vec<String>
    where T: ToString + Clone
{
    items |> map(|item| item |> toString) |> collect
}
```

## Type Inference

Restrict Language has powerful type inference:

```restrict
// Compiler infers types
let x = 42           // i32
let y = 3.14         // f64
let z = "hello"      // &str
let vec = vec![1, 2, 3]  // Vec<i32>

// Partial type annotations
let numbers: Vec<_> = vec![1, 2, 3]
let result = parse::<i32>("42")
```

## Prototype-Based Types

Restrict Language supports prototype-based inheritance:

```restrict
// Create a prototype
let animal_proto = freeze {
    species: "unknown",
    makeSound: fn() { "..." |> println }
}

// Derive from prototype
let dog = clone animal_proto with {
    species: "dog",
    makeSound: fn() { "Woof!" |> println }
}

// Type with derivation bound
fn feed<T from animal_proto>(animal: T) {
    animal.species |> println
    animal.makeSound()
}
```

## Memory Safety

The affine type system ensures memory safety without garbage collection:

```restrict
// Resource management
with file = openFile("data.txt") {
    file |> read |> process
}  // file automatically closed

// No double-free
let resource = allocate()
resource |> use
// resource |> use  // ERROR: already consumed

// No use-after-free
let data = getData()
let processed = data |> transform  // data consumed
// data |> print  // ERROR: data no longer available
```

## Best Practices

1. **Embrace affine types** - They prevent bugs at compile time
2. **Use clone sparingly** - Only when you truly need multiple uses
3. **Leverage type inference** - But add annotations for clarity
4. **Pattern match exhaustively** - The compiler ensures all cases are handled
5. **Use Option and Result** - For explicit error handling

## Advanced Topics

### Phantom Types

```restrict
struct Distance<Unit> {
    value: f64,
    _unit: PhantomData<Unit>
}

struct Meters
struct Feet

let d1: Distance<Meters> = Distance::new(100.0)
let d2: Distance<Feet> = Distance::new(328.0)
// Can't accidentally mix units
```

### Associated Types

```restrict
trait Container {
    type Item
    fn get(self) -> Self::Item
}

impl Container for Box<T> {
    type Item = T
    fn get(self) -> T {
        self.value
    }
}
```

## Summary

Restrict Language's type system provides:
- **Memory safety** through affine types
- **Expressiveness** through generics and type inference
- **Performance** with zero-cost abstractions
- **Correctness** through exhaustive pattern matching

The combination of affine types and OSV syntax creates a unique programming experience that catches bugs at compile time while remaining ergonomic and expressive.