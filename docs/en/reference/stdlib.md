# Standard Library Reference

The Restrict Language standard library provides essential functionality for everyday programming tasks. All standard library modules follow the affine type system and OSV syntax conventions.

## Core Modules

### std::prelude

Automatically imported types and functions available in every Restrict program.

```restrict
// Automatically available:
println, print, clone, freeze, toString
Option, Result, Vec, String
```

### std::io

Input/output operations.

```restrict
use std::io::*;

// Reading input
let input = readLine();
let content = readFile("data.txt")?;

// Writing output
"Hello" |> print;        // Without newline
"World" |> println;      // With newline
content |> writeFile("output.txt")?;

// Formatted output
format!("Name: {}, Age: {}", name, age) |> println;
```

### std::string

String manipulation utilities.

```restrict
use std::string::*;

// String operations
"hello" |> toUpperCase;      // "HELLO"
"WORLD" |> toLowerCase;      // "world"
"  trim me  " |> trim;       // "trim me"
"one,two,three" |> split(","); // ["one", "two", "three"]

// Parsing
"42" |> parse::<i32>();      // Ok(42)
"3.14" |> parse::<f64>();    // Ok(3.14)

// String building
let mut sb = StringBuilder::new();
sb |>> append("Hello");
sb |>> append(" ");
sb |>> append("World");
sb |> build();  // "Hello World"
```

### std::collections

Data structures and collections.

```restrict
use std::collections::*;

// Vector
let mut vec = Vec::new();
vec |>> push(1);
vec |>> push(2);
vec |> len();  // 2

// HashMap
let mut map = HashMap::new();
map |>> insert("key", "value");
map |> get("key");  // Some("value")

// HashSet
let mut set = HashSet::new();
set |>> insert(1);
set |> contains(1);  // true

// List (functional linked list)
let list = List::cons(1, List::cons(2, List::empty()));
list |> head();  // Some(1)
list |> tail();  // List with [2]
```

### std::iter

Iterator traits and utilities.

```restrict
use std::iter::*;

// Creating iterators
[1, 2, 3] |> iter();
1..10 |> iter();

// Iterator operations
vec
    |> iter()
    |> map(|x| x * 2)
    |> filter(|x| x > 5)
    |> take(3)
    |> collect::<Vec<_>>();

// Custom iterator
struct Counter {
    count: u32
}

impl Iterator for Counter {
    type Item = u32;
    
    fn next(&mut self) -> Option<u32> {
        self.count += 1;
        Some(self.count)
    }
}
```

### std::option

Option type utilities.

```restrict
use std::option::*;

let maybe = Some(42);

// Transformations
maybe |> map(|x| x * 2);          // Some(84)
maybe |> filter(|x| x > 50);      // None
maybe |> flatMap(|x| Some(x + 1)); // Some(43)

// Extracting values
maybe |> unwrap();                 // 42 (panics if None)
maybe |> unwrapOr(0);             // 42
maybe |> unwrapOrElse(|| compute()); // 42

// Chaining
maybe
    |> map(|x| x.toString())
    |> orElse(|| Some("default"))
    |> unwrap();
```

### std::result

Result type for error handling.

```restrict
use std::result::*;

let result: Result<i32, String> = Ok(42);

// Transformations
result |> map(|x| x * 2);         // Ok(84)
result |> mapErr(|e| e.len());    // Ok(42)
result |> andThen(|x| Ok(x + 1)); // Ok(43)

// Error handling
result |> unwrap();                // 42 (panics if Err)
result |> unwrapOr(0);            // 42
result |> unwrapOrElse(|e| handleError(e));

// Pattern matching
match result {
    Ok(value) => value |> process,
    Err(error) => error |> logError
}

// Try operator
fn divide(a: i32, b: i32) -> Result<i32, String> {
    if b == 0 {
        Err("Division by zero")
    } else {
        Ok(a / b)
    }
}

fn calculate() -> Result<i32, String> {
    let x = divide(10, 2)?;  // Early return on error
    let y = divide(x, 2)?;
    Ok(y)
}
```

### std::fs

File system operations.

```restrict
use std::fs::*;

// Reading files
let content = readFile("input.txt")?;
let bytes = readBytes("data.bin")?;

// Writing files
"Hello, World!" |> writeFile("output.txt")?;
bytes |> writeBytes("output.bin")?;

// File operations
exists("file.txt");           // bool
remove("temp.txt")?;
rename("old.txt", "new.txt")?;
copy("src.txt", "dst.txt")?;

// Directory operations
createDir("new_folder")?;
removeDir("old_folder")?;
readDir(".")?;  // Iterator of entries

// File metadata
let meta = metadata("file.txt")?;
meta |> isFile();      // bool
meta |> isDir();       // bool
meta |> len();         // u64 (file size)
meta |> modified();    // DateTime
```

### std::time

Time and date functionality.

```restrict
use std::time::*;

// Current time
let now = Instant::now();
let timestamp = SystemTime::now();

// Duration
let duration = Duration::fromSecs(60);
let elapsed = now.elapsed();

// Formatting
timestamp |> formatRFC3339();  // "2024-01-15T10:30:00Z"

// Sleep
Duration::fromMillis(100) |> sleep;
```

### std::sync

Synchronization primitives (for future multi-threading support).

```restrict
use std::sync::*;

// Atomic operations
let counter = AtomicU32::new(0);
counter |> fetchAdd(1);
counter |> load();

// Once (one-time initialization)
let INIT = Once::new();
INIT.callOnce(|| {
    // Initialize only once
    setupGlobals();
});
```

### std::mem

Memory utilities.

```restrict
use std::mem::*;

// Size information
sizeOf::<i32>();      // 4
sizeOf::<String>();   // Platform-specific

// Memory operations
let mut x = 5;
let y = 10;
swap(&mut x, &mut y);  // x = 10, y = 5

// Taking ownership
let value = take(&mut option);  // Moves out, leaving None
```

### std::convert

Type conversion traits.

```restrict
use std::convert::*;

// Into trait
let string: String = "hello" |> into();
let number: i64 = 42i32 |> into();

// TryInto trait
let small: i32 = 1000i64 |> tryInto()?;

// From trait implementation
impl From<i32> for MyType {
    fn from(value: i32) -> Self {
        MyType { value }
    }
}
```

### std::hash

Hashing utilities.

```restrict
use std::hash::*;

// Hashing values
let hash = "hello" |> hash();

// Custom hashable type
#[derive(Hash)]
struct Point {
    x: i32,
    y: i32
}
```

### std::fmt

Formatting and display traits.

```restrict
use std::fmt::*;

// Display trait
impl Display for Point {
    fn fmt(&self, f: &mut Formatter) -> Result {
        write!(f, "({}, {})", self.x, self.y)
    }
}

// Debug trait
#[derive(Debug)]
struct Complex {
    real: f64,
    imag: f64
}

// Usage
point |> toString();     // Uses Display
complex |> debug();      // Uses Debug
```

### std::math

Mathematical functions.

```restrict
use std::math::*;

// Basic operations
abs(-5);         // 5
min(3, 7);       // 3
max(3, 7);       // 7
clamp(15, 0, 10); // 10

// Floating point
3.14 |> floor();  // 3.0
3.14 |> ceil();   // 4.0
3.14 |> round();  // 3.0
16.0 |> sqrt();   // 4.0

// Trigonometry
PI;              // 3.141592...
E;               // 2.718281...
45.0 |> toRadians() |> sin();
1.0 |> asin() |> toDegrees();

// Powers and logarithms
2.0 |> pow(3.0);  // 8.0
100.0 |> log10(); // 2.0
E |> ln();        // 1.0
```

### std::random

Random number generation.

```restrict
use std::random::*;

// Random values
let mut rng = Rng::new();
rng |> nextU32();              // Random u32
rng |> nextF64();              // Random f64 in [0, 1)
rng |> range(1, 100);          // Random in range [1, 100)

// Random selection
let items = vec!["a", "b", "c"];
items |> choose(&mut rng);      // Random element

// Shuffling
let mut numbers = vec![1, 2, 3, 4, 5];
numbers |>> shuffle(&mut rng);
```

### std::net

Networking functionality (async-ready).

```restrict
use std::net::*;

// TCP
let listener = TcpListener::bind("127.0.0.1:8080")?;
for stream in listener.incoming() {
    stream? |> handleClient;
}

// HTTP client (simplified)
let response = http::get("https://example.com")?;
response |> status();  // 200
response |> body();    // Response content
```

### std::env

Environment and program arguments.

```restrict
use std::env::*;

// Command line arguments
let args = args();  // Vec<String>
let program = args[0];  // Program name

// Environment variables
let home = var("HOME")?;
setVar("MY_VAR", "value");
removeVar("OLD_VAR");

// Working directory
let cwd = currentDir()?;
setCurrentDir("/tmp")?;
```

### std::process

Process management.

```restrict
use std::process::*;

// Running commands
let output = Command::new("ls")
    |> arg("-la")
    |> output()?;

output |> stdout() |> toString();
output |> status() |> success();  // bool

// Exit
exit(0);  // Success
exit(1);  // Error
```

## Type Traits

### Clone

```restrict
trait Clone {
    fn clone(&self) -> Self;
}

// Usage
let original = MyType::new();
let copy = clone original;
```

### ToString

```restrict
trait ToString {
    fn toString(&self) -> String;
}

// Usage
42 |> toString();  // "42"
```

### Default

```restrict
trait Default {
    fn default() -> Self;
}

// Usage
let value = MyType::default();
```

### Eq and Ord

```restrict
trait Eq {
    fn eq(&self, other: &Self) -> bool;
}

trait Ord {
    fn cmp(&self, other: &Self) -> Ordering;
}

// Derivable
#[derive(Eq, Ord)]
struct Point { x: i32, y: i32 }
```

## Error Types

Common error types in the standard library:

```restrict
enum IoError {
    NotFound,
    PermissionDenied,
    ConnectionRefused,
    Other(String)
}

enum ParseError {
    InvalidFormat,
    Overflow,
    Empty
}
```

## Best Practices

1. **Use Result for fallible operations** - Don't panic unnecessarily
2. **Leverage type inference** - But add annotations for clarity
3. **Prefer iterators over loops** - More functional and composable
4. **Use standard traits** - Clone, ToString, etc. for consistency
5. **Handle resources with `with`** - Automatic cleanup

## Example: File Processing

```restrict
use std::fs::*;
use std::io::*;

fn processFile(path: String) -> Result<(), IoError> {
    // Read file
    let content = path |> readFile()?;
    
    // Process lines
    let processed = content
        |> lines()
        |> map(|line| line |> trim())
        |> filter(|line| !line.isEmpty())
        |> map(|line| line |> toUpperCase())
        |> collect::<Vec<_>>()
        |> join("\n");
    
    // Write result
    processed |> writeFile("output.txt")?;
    
    Ok(())
}

fn main() {
    match "input.txt" |> processFile {
        Ok(()) => "File processed successfully" |> println,
        Err(e) => format!("Error: {:?}", e) |> println
    }
}
```

The standard library is designed to work seamlessly with Restrict Language's affine type system and OSV syntax, providing a safe and ergonomic programming experience.