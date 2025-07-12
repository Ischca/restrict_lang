# Restrict Language Tutorial

## Getting Started

Welcome to Restrict Language! This tutorial will guide you through the basics of writing programs in this functional language that compiles to WebAssembly.

## Installation and Setup

```bash
# Clone the repository
git clone https://github.com/your-username/restrict_lang
cd restrict_lang

# Build the compiler
cargo build --release

# Compile a program
./target/release/restrict_lang examples/hello.rl
```

## Your First Program

Let's start with a simple "Hello World" equivalent:

```rust
// hello.rl
fun main = {
    val message = "Hello, Restrict Language!";
    val number = 42;
    number
}
```

This program:
- Defines a function called `main` 
- Creates an immutable string variable
- Creates an immutable integer variable
- Returns the number (the last expression in a block is returned)

## Variables and Types

### Variable Declarations

```rust
// Immutable variables (default)
val name = "Alice"
val age = 30
val height = 5.8

// Mutable variables
val mut counter = 0
counter = counter + 1  // OK: can reassign mutable variables

// With explicit types
val score: Int = 100
val pi: Float64 = 3.14159
val active: Boolean = true
```

### The Affine Type System

Restrict Language uses an "affine" type system - each variable can be used at most once:

```rust
val x = 42
val y = x    // x is "consumed" here
// val z = x // ERROR: x has already been used!

// Exception: mutable variables can be reused
val mut count = 0
count = count + 1  // OK
count = count + 1  // OK
```

This prevents accidental copying of resources and makes memory usage predictable.

## Functions

### Basic Functions

```rust
// Simple function with parameters and return type
fun add = x:Int, y:Int -> Int {
    x + y
}

// Function without explicit return type (inferred)
fun multiply = x:Int, y:Int {
    x * y
}

// Zero-parameter function
fun get_greeting = {
    "Hello!"
}
```

### Function Calls: OSV Syntax

Restrict Language uses Object-Subject-Verb syntax, which puts arguments before the function:

```rust
// Traditional: add(5, 10)
// OSV syntax: (5, 10) add
val result = (5, 10) add      // Calls add(5, 10)
val doubled = (21) double     // Calls double(21)  
val greeting = () get_greeting // Calls get_greeting()

// You can also use traditional syntax
val result2 = add(5, 10)
```

Why OSV? It makes function composition and pipelining more natural!

## Lambda Expressions

Lambdas are anonymous functions that can capture variables from their environment:

```rust
// Simple lambda
val double = |x| x * 2
val result = (21) double  // Returns 42

// Multi-parameter lambda
val add = |x, y| x + y
val sum = (5, 10) add

// Lambda with block body
val complex_calc = |x| {
    val doubled = x * 2;
    val incremented = doubled + 1;
    incremented
}
```

### Closures

Lambdas can "capture" variables from their surrounding scope:

```rust
fun make_adder = n:Int {
    |x| x + n  // Captures 'n' from the function parameter
}

val add5 = make_adder(5)
val result = (10) add5  // Returns 15

fun counter_factory = start:Int {
    val mut count = start;
    || {  // Zero-parameter lambda
        count = count + 1;
        count
    }
}

val counter = counter_factory(0)
val first = counter()   // Returns 1
val second = counter()  // Returns 2
```

## Pattern Matching

Pattern matching is like a powerful switch statement:

```rust
fun describe_number = n:Int {
    n match {
        0 => { "zero" }
        1 => { "one" }
        2 => { "two" }
        _ => { "some other number" }  // _ is wildcard
    }
}

// With Option types
fun safe_divide = x:Int, y:Int {
    y match {
        0 => { None }
        _ => { Some(x / y) }
    }
}

val result = safe_divide(10, 2) match {
    Some(value) => { "Result: " + value }
    None => { "Cannot divide by zero!" }
}
```

## Lists and Collections

### Lists

Lists are dynamic arrays with a header containing length and capacity:

```rust
// Create lists
val numbers = [1, 2, 3, 4, 5]
val empty: List<Int> = []
val strings = ["hello", "world"]

// List operations
val first = numbers[0]           // Get element
val length = len(numbers)        // Get length
val doubled = map(numbers, |x| x * 2)  // Transform elements
val evens = filter(numbers, |x| x % 2 == 0)  // Filter elements
```

### List Pattern Matching

```rust
fun process_list = numbers:List<Int> {
    numbers match {
        [] => { "empty list" }
        [x] => { "single element: " + x }
        [first | rest] => { "first: " + first + ", rest has " + len(rest) + " elements" }
        [a, b] => { "exactly two: " + a + " and " + b }
        [a, b, c | _] => { "at least three, starting with: " + a }
    }
}
```

### Arrays

Arrays are fixed-size with no header overhead:

```rust
// Fixed-size arrays (note the |..| syntax)
val coordinates: Array<Int, 3> = [|10, 20, 30|]
val point2d: Array<Float64, 2> = [|1.0, 2.0|]

// Access elements
val x = coordinates[0]
val y = coordinates[1]
val z = coordinates[2]
```

## Records (Structs)

Records group related data together:

```rust
// Define a record type
record Person {
    name: String,
    age: Int,
    email: String,
}

// Create record instances
val alice = Person {
    name: "Alice Smith",
    age: 30,
    email: "alice@example.com"
}

// Access fields
val name = alice.name
val age = alice.age

// Pattern matching on records
fun greet_person = person:Person {
    person match {
        Person { name: "Alice", age, email } => { "Hello Alice, age " + age }
        Person { name, age: 0, email } => { "Hello baby " + name }
        Person { name, age, email } => { "Hello " + name }
    }
}
```

### Record Methods

You can implement methods for records:

```rust
impl Person {
    fun get_display_name = self:Person {
        self.name + " <" + self.email + ">"
    }
    
    fun is_adult = self:Person {
        self.age >= 18
    }
    
    fun have_birthday = self:Person {
        Person { ...self, age: self.age + 1 }  // Update age, keep other fields
    }
}

// Use methods
val display = alice.get_display_name()
val is_adult = alice.is_adult()
val older_alice = alice.have_birthday()
```

## Memory Management with Arenas

Instead of garbage collection, Restrict Language uses arena allocation:

```rust
// Create an arena (1KB)
val arena = new_arena(1024)

// Use arena for allocations
arena {
    val big_list = [1, 2, 3, /* ... many elements ... */]
    val user = Person { name: "Bob", age: 25, email: "bob@test.com" }
    
    // Process data...
    val processed = process_data(big_list, user)
    
    // All memory automatically freed when leaving arena scope!
}

// Set a default arena for convenience
use_default_arena(2048)
val global_data = [1, 2, 3]  // Uses default arena
```

## Control Flow

### Conditionals

```rust
// If-expressions (not statements!)
val status = if age >= 18 { "adult" } else { "minor" }

// Complex conditions
val category = if score >= 90 {
    "excellent"
} else if score >= 70 {
    "good"  
} else {
    "needs improvement"
}
```

### Loops and Recursion

```rust
// While loops (imperative style)
fun count_up = limit:Int {
    val mut i = 0;
    while i < limit {
        // do something with i
        i = i + 1
    }
}

// Recursion (functional style - preferred)
fun factorial = n:Int -> Int {
    n match {
        0 => { 1 }
        1 => { 1 }
        _ => { n * factorial(n - 1) }
    }
}

fun sum_list = numbers:List<Int> -> Int {
    numbers match {
        [] => { 0 }
        [head | tail] => { head + sum_list(tail) }
    }
}
```

## Practical Examples

### Example 1: Todo List Manager

```rust
record Todo {
    id: Int,
    title: String,
    completed: Boolean,
}

impl Todo {
    fun complete = self:Todo {
        Todo { ...self, completed: true }
    }
    
    fun is_completed = self:Todo {
        self.completed
    }
}

fun find_todo = todos:List<Todo>, id:Int -> Option<Todo> {
    todos match {
        [] => { None }
        [head | tail] => {
            head.id match {
                id => { Some(head) }
                _ => { find_todo(tail, id) }
            }
        }
    }
}

fun complete_todo = todos:List<Todo>, id:Int -> List<Todo> {
    map(todos, |todo| {
        todo.id match {
            id => { todo.complete() }
            _ => { todo }
        }
    })
}

fun main = {
    val todos = [
        Todo { id: 1, title: "Learn Restrict Language", completed: false },
        Todo { id: 2, title: "Build an app", completed: false },
        Todo { id: 3, title: "Deploy to production", completed: false }
    ];
    
    val updated_todos = complete_todo(todos, 1);
    len(updated_todos)
}
```

### Example 2: Data Processing Pipeline

```rust
record Sale {
    amount: Int,
    customer_id: Int,
    product_id: Int,
}

fun process_sales = sales:List<Sale> -> Int {
    sales
        |> filter(|sale| sale.amount > 100)      // High-value sales only
        |> map(|sale| sale.amount)               // Extract amounts
        |> fold(0, |acc, amount| acc + amount)   // Sum them up
}

fun top_customers = sales:List<Sale>, limit:Int -> List<Int> {
    sales
        |> group_by(|sale| sale.customer_id)     // Group by customer
        |> map(|(customer_id, customer_sales)| {
            val total = sum(map(customer_sales, |s| s.amount));
            (customer_id, total)
        })
        |> sort_by(|(_, total)| total)           // Sort by total
        |> reverse()                             // Highest first
        |> take(limit)                           // Take top N
        |> map(|(customer_id, _)| customer_id)   // Extract IDs
}

fun main = {
    val sales = [
        Sale { amount: 150, customer_id: 1, product_id: 101 },
        Sale { amount: 75, customer_id: 2, product_id: 102 },
        Sale { amount: 200, customer_id: 1, product_id: 103 },
        Sale { amount: 50, customer_id: 3, product_id: 101 },
        Sale { amount: 300, customer_id: 2, product_id: 104 }
    ];
    
    val total_high_value = process_sales(sales);
    val top_2_customers = top_customers(sales, 2);
    
    total_high_value
}
```

### Example 3: Parser Combinator

```rust
record Parser<T> {
    parse: String -> Option<(T, String)>  // (result, remaining_input)
}

fun char_parser = expected:Char -> Parser<Char> {
    Parser {
        parse: |input| {
            input match {
                "" => { None }
                _ => {
                    val first_char = input[0];
                    first_char match {
                        expected => { Some((first_char, input[1..])) }
                        _ => { None }
                    }
                }
            }
        }
    }
}

fun map_parser = parser:Parser<A>, f:A->B -> Parser<B> {
    Parser {
        parse: |input| {
            (input) parser.parse match {
                Some((result, remaining)) => { Some(((result) f, remaining)) }
                None => { None }
            }
        }
    }
}

fun sequence_parser = p1:Parser<A>, p2:Parser<B> -> Parser<(A, B)> {
    Parser {
        parse: |input| {
            (input) p1.parse match {
                Some((result1, remaining1)) => {
                    (remaining1) p2.parse match {
                        Some((result2, remaining2)) => { Some(((result1, result2), remaining2)) }
                        None => { None }
                    }
                }
                None => { None }
            }
        }
    }
}

fun main = {
    val hello_parser = sequence_parser(
        char_parser('H'),
        char_parser('i')
    );
    
    val result = ("Hi there") hello_parser.parse;
    result match {
        Some(((h, i), remaining)) => { 1 }  // Success
        None => { 0 }  // Failed
    }
}
```

## Best Practices

### 1. Embrace Immutability

```rust
// Good: Create new values instead of modifying
fun update_person_age = person:Person, new_age:Int {
    Person { ...person, age: new_age }
}

// Avoid: Mutation unless necessary
fun bad_update = person:Person, new_age:Int {
    person.age = new_age  // This would require mutable records
}
```

### 2. Use Pattern Matching

```rust
// Good: Exhaustive pattern matching
fun handle_option = maybe_value:Option<Int> {
    maybe_value match {
        Some(value) => { process_value(value) }
        None => { default_value() }
    }
}

// Avoid: Assuming success
fun bad_handle = maybe_value:Option<Int> {
    val value = unwrap(maybe_value)  // Could crash!
    process_value(value)
}
```

### 3. Leverage the Type System

```rust
// Good: Use types to prevent errors
record UserId { id: Int }
record ProductId { id: Int }

fun get_user = user_id:UserId -> Option<User> {
    // Implementation
}

// Prevents accidentally passing product ID as user ID
// get_user(ProductId { id: 123 })  // Type error!
```

### 4. Think in Pipelines

```rust
// Good: Functional pipeline
fun process_data = input:List<String> {
    input
        |> filter(|s| len(s) > 0)        // Remove empty strings
        |> map(trim)                     // Trim whitespace  
        |> map(to_lowercase)             // Normalize case
        |> filter(|s| starts_with(s, "a")) // Only words starting with 'a'
        |> sort()                        // Sort alphabetically
}
```

### 5. Use Arenas for Performance

```rust
// Good: Group related allocations in arenas
fun process_large_dataset = data:List<RawData> {
    new_arena(1024 * 1024) {  // 1MB arena
        val parsed = map(data, parse_record);
        val filtered = filter(parsed, is_valid);
        val results = map(filtered, transform);
        summarize(results)  // Only summary escapes arena
    }
}
```

## What's Next?

Now that you've learned the basics, you can:

1. **Explore the standard library** (when available)
2. **Build real applications** using WebAssembly
3. **Contribute to the language** development
4. **Read the detailed reference** for advanced features

Check out more examples in the `examples/` directory and refer to `REFERENCE.md` for complete language documentation.

Happy coding with Restrict Language! 🚀