# Restrict Language Specification v1.0

**THE DEFINITIVE SOURCE OF TRUTH FOR RESTRICT LANGUAGE SYNTAX**

This document is the **single authoritative specification** for Restrict Language. All other documentation is superseded by this specification. Any conflicts with other documentation should be resolved by referring to this document.

## Language Philosophy

- **OSV (Object-Subject-Verb)**: Natural function composition: `value |> function`
- **Affine Types**: Each variable can be used at most once (unless marked mutable)
- **Temporal Affine Types (TAT)**: Automatic resource cleanup with temporal scopes
- **No Side Effects**: Expression statements must be pure
- **Arena Memory**: Deterministic memory management without garbage collection

## 1. Lexical Elements

### 1.1 Keywords (Reserved)
```
fun val mut record context enum match then else while
temporal within where clone freeze pub import export
as fatal true false Some None with lifetime await spawn
```

### 1.2 Operators
```
|>      // Pipe operator (primary)
=       // Assignment
=>      // Match arrow  
->      // Function return type arrow
+  -    // Arithmetic
*  /  % // Arithmetic
== !=   // Equality
<  <=   // Comparison  
>  >=   // Comparison
&&  ||  // Logical
!       // Logical not
~       // Temporal marker
```

### 1.3 Delimiters
```
{ }     // Block/Record delimiters
( )     // Expression/Parameter grouping
[ ]     // List/Array literals
, ;     // Separators
: .     // Type annotation, field access
```

### 1.4 Literals
- **Integers**: `42`, `0xFF`, `1_000_000`
- **Floats**: `3.14`, `1.5e10`, `3.14E-2`
- **Strings**: `"hello"`, with escapes `\n \t \\ \" \'`
- **Characters**: `'a'`, `'\n'`
- **Booleans**: `true`, `false`
- **Unit**: `()`

### 1.5 Comments
- **Single-line**: `// comment`
- **Multi-line**: `/* comment */` (no nesting)

## 2. Variable Declarations

### 2.1 Immutable Variables
```rust
val x = 42              // Immutable binding
val name = "Alice"      // Type inferred
val pi: Float64 = 3.14  // Explicit type
```

### 2.2 Mutable Variables
```rust
mut val counter = 0     // Mutable binding (mut before val)
mut val items: List<String> = []  // With type annotation
```

**CRITICAL**: The syntax is `mut val`, NOT `val mut`. This is enforced by the parser.

## 3. Function Declarations

### 3.1 Standard Function Syntax
```rust
fun name: (param: Type, ...) -> ReturnType = {
    // body
}

// Examples:
fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

fun greet: (name: String) = {  // Return type inferred
    "Hello, " + name
}

fun main: () = {  // No parameters
    42
}
```

### 3.2 Generic Functions
```rust
fun identity: <T>(value: T) -> T = {
    value
}

fun map: <T, U>(list: List<T>, f: T -> U) -> List<U> = {
    // implementation
}
```

### 3.3 Temporal Functions
```rust
fun process: <~t>(data: Data<~t>) -> Result<Data<~t>, Error> = {
    data |> validate |> transform
}
```

## 4. Types

### 4.1 Basic Types
- `Int32`, `Int64`, `Float64`
- `String`, `Char`, `Boolean`
- `()` (Unit type)

### 4.2 Generic Types
```rust
List<T>           // Dynamic list
Array<T, N>       // Fixed-size array  
Option<T>         // Maybe value
Result<T, E>      // Success or error
Range<T>          // Range of values
```

### 4.3 Temporal Types
```rust
File<~f>          // File with temporal scope ~f
Connection<~db>   // Database connection with scope ~db
```

### 4.4 Function Types
```rust
Int32 -> String         // Function type
(Int32, String) -> Bool // Multi-parameter function
```

## 5. Expressions

### 5.1 Literals and Variables
```rust
42              // Integer
3.14            // Float
"hello"         // String
'x'             // Character
true            // Boolean
()              // Unit
x               // Variable reference
```

### 5.2 Function Calls

#### OSV Style (Object-Subject-Verb) - ONLY SUPPORTED SYNTAX

```rust
// ✅ CORRECT: OSV syntax (Object-Subject-Verb)
value |> function           // Single argument via pipe
(arg1, arg2) function       // Multiple arguments via tuple  
() function                 // No arguments via unit

// ❌ COMPILE ERROR: Traditional function calls NOT supported  
function(args)              // ERROR: Traditional syntax forbidden
function()                  // ERROR: Traditional syntax forbidden
object.method(args)         // ERROR: Traditional syntax forbidden
```

**CRITICAL RULE**: Restrict Language **exclusively** uses OSV syntax. 
Arguments always come BEFORE the function name. Traditional parenthetical 
function calls `function(args)` will cause compilation errors.

**OSV Pattern Examples:**
```rust
// Data flows left-to-right naturally
"hello world" |> to_uppercase |> reverse |> println

// Multiple arguments use tuple syntax
(10, 20) add                    // Instead of add(10, 20)
(1, 2, 3, 4) sum_all           // Instead of sum_all(1, 2, 3, 4)

// Complex expressions maintain clarity
val result = user_data
    |> validate_input
    |> transform_data  
    |> save_to_database
    |> generate_response

// Method-like calls still use OSV
user.profile get_name          // Instead of user.profile.get_name()
database.connection close      // Instead of database.connection.close()
```

### 5.3 Binary Operations
```rust
x + y           // Addition
x - y           // Subtraction  
x * y           // Multiplication
x / y           // Division
x % y           // Modulo
x == y          // Equality
x != y          // Inequality
x < y           // Less than
x <= y          // Less than or equal
x > y           // Greater than
x >= y          // Greater than or equal
x && y          // Logical and
x || y          // Logical or
```

### 5.4 Conditional Expressions
```rust
condition then {
    // true branch
} else {
    // false branch
}

// Example:
age >= 18 then { "adult" } else { "minor" }
```

### 5.5 Match Expressions
```rust
value match {
    pattern => { result }
    pattern => { result }
    _ => { default }
}

// Example:
x match {
    Some(v) => { v * 2 }
    None => { 0 }
}
```

### 5.6 List/Array Literals
```rust
[1, 2, 3]           // List literal
[1..10]             // Range (creates Range<Int32>)
[]                  // Empty list
```

**DEPRECATED**: `[|1, 2, 3|]` syntax is no longer supported.

### 5.7 Record Literals
```rust
Person { name = "Alice", age = 30 }
Point { x = 0, y = 0 }
```

### 5.8 Lambda Expressions
```rust
|x| x * 2           // Single parameter
|x, y| x + y        // Multiple parameters
|x: Int32| x + 1    // With type annotations
```

## 6. Patterns (for match expressions)

### 6.1 Basic Patterns
```rust
_               // Wildcard
x               // Variable binding
42              // Literal
true            // Boolean literal
"hello"         // String literal
```

### 6.2 Option Patterns
```rust
Some(x)         // Extract value from Some
None            // Match None
```

### 6.3 List Patterns
```rust
[]              // Empty list
[x]             // Single element
[x, y]          // Exact elements
[head | tail]   // Head and tail (cons pattern)
```

### 6.4 Record Patterns  
```rust
Person { name, age }                    // Extract all fields
Person { name = "Alice", age }          // Partial match with literal
Point { x = 0, y = 0 }                  // Exact match
```

### 6.5 Spread Destructuring Patterns

Spread destructuring allows extraction of specific fields while capturing remaining fields in a rest binding:

```rust
// Basic spread pattern
Person { name, email, ...rest }         // Extract name and email, rest gets remaining fields

// Spread with explicit field patterns
User { id: userId, name, ...userMeta }  // Extract id as userId, name as name, rest as userMeta

// Spread in match expressions
value match {
    User { role: "admin", ..._ } => { "Administrator access" }
    User { department: "IT", name, ..._ } => { "IT user: " + name }
    User { name, ...profile } => { process_user(name, profile) }
}

// Nested spread patterns (if supported)
Company { 
    name: companyName,
    contact: Contact { email, ...contactInfo },
    ...companyDetails 
} => {
    // Extract company name, contact email, and group remaining fields
    process_company(companyName, email, contactInfo, companyDetails)
}

// Wildcard spread (ignore remaining fields)
Point { x, y, ..._ } => { calculate_distance(x, y) }
```

**Spread Pattern Rules:**
- Spread pattern `...rest` must be the last element in a record pattern
- Rest binding captures all unmatched fields as a new record
- Use `..._` to ignore remaining fields
- Rest binding maintains the original record type but only with remaining fields

## 7. Statements

### 7.1 Variable Declarations
```rust
val x = 42              // Immutable
mut val counter = 0     // Mutable
```

### 7.2 Assignments
```rust
counter = counter + 1   // Only for mutable variables
```

### 7.3 Expression Statements
```rust
42                      // Expression as statement
println("hello")        // Function call
x + y                   // Must be pure (no side effects)
```

## 8. Record Types

### 8.1 Basic Records
```rust
record Person {
    name: String
    age: Int32
}

record Point<T> {
    x: T
    y: T
}
```

### 8.2 Temporal Records
```rust
record File<~t> {
    path: String
    handle: FileHandle<~t>
}

record Connection<~db> where ~tx within ~db {
    url: String
    session: Session<~tx>
}
```

## 9. Context Declarations

### 9.1 Basic Context
```rust
context Database {
    connection: Connection
    timeout: Int32
}
```

### 9.2 Context-Bound Functions
```rust
@Database
fun query: (sql: String) -> Result<Data, Error> = {
    // Can access connection and timeout implicitly
    connection |> execute sql
}
```

## 10. Temporal Resource Management

### 10.1 Temporal Scopes
```rust
temporal ~t {
    val resource = Resource<~t> { ... }
    // resource automatically cleaned up when ~t ends
}
```

### 10.2 With Expressions
```rust
with Database { connection = conn } {
    "SELECT * FROM users" |> query
}

with lifetime<~f> {
    val file = File<~f> { path = "/tmp/data" }
    file |> read
}
```

### 10.3 Temporal Constraints
```rust
where ~inner within ~outer     // inner lifetime contained in outer
```

## 11. Prototype Operations

### 11.1 Clone
```rust
val newObj = obj.clone { field = newValue }
```

### 11.2 Freeze
```rust
val frozen = obj freeze         // Make immutable
val cloneAndFreeze = obj.clone { field = value } freeze
```

## 12. Import/Export

### 12.1 Imports
```rust
import "std/io" as io
import "std/collections" as collections
```

### 12.2 Exports
```rust
pub fun publicFunction: () = { ... }
pub record PublicType { ... }
```

## 13. Operator Precedence (Highest to Lowest)

1. Field access: `.field`, `.clone`, `freeze`
2. Unary: `!`, `-`
3. Multiplicative: `*`, `/`, `%`
4. Additive: `+`, `-`
5. Relational: `<`, `<=`, `>`, `>=`
6. Equality: `==`, `!=`
7. Logical AND: `&&`
8. Logical OR: `||`
9. Pipe: `|>` (left associative)
10. OSV function calls (right associative)

## 14. Standard Library Types

### 14.1 Collections
- `List<T>` - Dynamic list
- `Array<T, N>` - Fixed-size array
- `Range<T>` - Range type (from `[start..end]`)

### 14.2 Error Handling
- `Option<T>` - May contain value (`Some(T)`) or `None`
- `Result<T, E>` - Success (`Ok(T)`) or error (`Err(E)`)

### 14.3 Basic Functions
```rust
println: (String) -> ()
print_int: (Int32) -> ()
toString: (T) -> String
```

## 15. DEPRECATED AND REMOVED SYNTAX

The following syntax is **NO LONGER SUPPORTED** and will cause compilation errors:

### 15.1 Removed Keywords
- `let` (use `val` instead)
- `fn` (use `fun` instead)  
- `if` (use `then/else` instead)
- `Unit` as type name (use `()`)

### 15.2 Removed Operators
- `|>>` mutable pipe operator (removed)

### 15.3 Removed Syntax Patterns
- `val mut x = 5` (use `mut val x = 5`)
- `[|1, 2, 3|]` array literals (use `[1, 2, 3]`)
- `if condition { ... }` (use `condition then { ... }`)
- `while condition { ... }` (use `condition while { ... }`)

## 16. EXAMPLES

### 16.1 Hello World
```rust
fun main: () = {
    "Hello, Restrict Language!" |> println
}
```

### 16.2 Basic Arithmetic
```rust
fun add: (x: Int32, y: Int32) -> Int32 = {
    x + y
}

fun main: () = {
    val result = (10, 20) add
    result |> print_int
}
```

### 16.3 Pattern Matching
```rust
fun describe: (x: Option<Int32>) -> String = {
    x match {
        Some(n) => { "Got number: " + n.toString() }
        None => { "No number" }
    }
}
```

### 16.4 Comprehensive Pattern Matching Examples
```rust
// Advanced Option pattern matching
fun process_maybe: (data: Option<User>) -> String = {
    data match {
        Some(User { name, role: "admin", ..._ }) => { "Admin: " + name }
        Some(User { name, department: "IT", ..._ }) => { "IT user: " + name }
        Some(User { name, ..._ }) => { "Regular user: " + name }
        None => { "No user data" }
    }
}

// List pattern matching with spread
fun analyze_list: (numbers: List<Int32>) -> String = {
    numbers match {
        [] => { "Empty list" }
        [single] => { "One item: " + single.toString() }
        [first, second] => { "Two items: " + first.toString() + ", " + second.toString() }
        [head | tail] => { "Head: " + head.toString() + ", tail has " + tail.length.toString() + " items" }
    }
}

// Complex nested pattern matching
record Address { street: String, city: String, zipcode: String }
record Person { name: String, age: Int32, address: Address, tags: List<String> }

fun categorize_person: (person: Person) -> String = {
    person match {
        // Pattern with nested record destructuring
        Person { 
            age, 
            address: Address { city: "Tokyo", ..._ },
            tags,
            ..._ 
        } when age >= 65 => { "Senior citizen in Tokyo" }
        
        // Pattern with list matching
        Person { name, tags: ["VIP" | _], ..._ } => { "VIP member: " + name }
        Person { name, tags: [], ..._ } => { "Untagged user: " + name }
        
        // Catch-all with spread
        Person { name, age, ...profile } => { 
            "Regular user: " + name + " (" + age.toString() + ")" 
        }
    }
}
```

### 16.5 Records and Methods
```rust
record Point {
    x: Int32
    y: Int32
}

fun distance: (self: Point, other: Point) -> Float64 = {
    val dx = self.x - other.x
    val dy = self.y - other.y
    ((dx * dx + dy * dy) as Float64).sqrt()
}
```

### 16.6 Temporal Resource Management
```rust
fun processFile: (path: String) -> Result<String, Error> = {
    temporal ~file {
        val file = File<~file> { path = path }
        file |> read
        // file automatically closed when ~file scope ends
    }
}
```

## 17. MIGRATION GUIDE

If you have existing code using deprecated syntax:

### 17.1 Variable Declarations
```rust
// OLD (incorrect)
val mut x = 5
let x = 5

// NEW (correct)  
mut val x = 5
val x = 5
```

### 17.2 Function Declarations
```rust
// OLD (some docs show this)
fun add = x:Int y:Int { x + y }

// NEW (correct)
fun add: (x: Int32, y: Int32) -> Int32 = { x + y }
```

### 17.3 Array Literals
```rust
// OLD (deprecated)
[|1, 2, 3|]

// NEW (correct)
[1, 2, 3]
```

### 17.4 Conditionals
```rust
// OLD (not supported)
if condition { ... } else { ... }

// NEW (correct)
condition then { ... } else { ... }
```

---

## COMPLIANCE

This specification defines Restrict Language v1.0. All implementations, documentation, tutorials, and examples MUST conform to this specification. 

**Parser Implementation**: The official parser in `src/parser.rs` implements this specification exactly.

**Documentation**: All other documentation files are superseded by this specification.

**Last Updated**: 2025-01-10  
**Version**: 1.0.0  
**Status**: CANONICAL SOURCE OF TRUTH