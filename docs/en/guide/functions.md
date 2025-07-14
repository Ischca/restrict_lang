# Functions

Functions are first-class values in Restrict Language. They can be passed as arguments, returned from other functions, and stored in variables.

## Function Definition

Basic function syntax:

```restrict
fun add = x:Int, y:Int -> Int {
    x + y
}
```

Functions without parameters:

```restrict
fun get_answer = -> Int {
    42
}
```

Functions with implicit return type:

```restrict
fun double = x:Int {
    x * 2  // Type inferred as Int
}
```

## OSV (Object-Subject-Verb) Syntax

Restrict Language supports OSV syntax for natural function composition:

```restrict
// Traditional call
val result = add(5, 10)

// OSV call - object(s) come first, then subject (function)
val result = (5, 10) add

// Single argument OSV
val doubled = 21 double
```

## Function Composition

Chain functions naturally with OSV:

```restrict
fun increment = x:Int { x + 1 }
fun double = x:Int { x * 2 }
fun square = x:Int { x * x }

// Chain operations
val result = 5 increment double square  // ((5 + 1) * 2)² = 144
```

## Higher-Order Functions

Functions can accept and return other functions:

```restrict
fun apply_twice = f:(Int -> Int), x:Int {
    x f f  // Apply f twice using OSV
}

val quad = (double, 5) apply_twice  // Returns 20
```

## Generic Functions

Define functions that work with multiple types:

```restrict
fun identity<T> = x:T -> T {
    x
}

fun map_option<T, U> = opt:Option<T>, f:(T -> U) -> Option<U> {
    opt match {
        Some(value) => { Some(value f) }
        None => { None }
    }
}
```

## Function Values

Functions can be stored in variables:

```restrict
val add_five = |x| x + 5
val multiply = |x, y| x * y

// Use them like regular functions
val result = 10 add_five  // 15
val product = (3, 4) multiply  // 12
```

## Recursive Functions

Recursive functions are fully supported:

```restrict
fun factorial = n:Int -> Int {
    if n <= 1 
    then { 1 }
    else { n * (n - 1) factorial }
}

fun fibonacci = n:Int -> Int {
    n match {
        0 => { 0 }
        1 => { 1 }
        _ => { (n - 1) fibonacci + (n - 2) fibonacci }
    }
}
```

## Method Syntax

Functions can be defined as methods on records:

```restrict
record Point {
    x: Int,
    y: Int,
}

impl Point {
    fun distance = self:Point, other:Point -> Int {
        val dx = self.x - other.x
        val dy = self.y - other.y
        // Simplified distance calculation
        dx * dx + dy * dy
    }
}

// Usage
val p1 = Point { x: 0, y: 0 }
val p2 = Point { x: 3, y: 4 }
val dist = p1.distance(p2)  // Method call syntax
```

## Partial Application

Create new functions by partially applying arguments:

```restrict
fun add = x:Int, y:Int { x + y }

// Traditional partial application with lambda
val add5 = |y| add(5, y)

// Using it
val result = 10 add5  // 15
```

## Function Type Annotations

Explicit function type syntax:

```restrict
// Function that takes Int and returns Int
val double: (Int -> Int) = |x| x * 2

// Function that takes two Ints and returns Int
val add: (Int, Int -> Int) = |x, y| x + y

// Higher-order function type
val transformer: ((Int -> Int) -> (Int -> Int)) = |f| {
    |x| f(f(x))  // Returns a function that applies f twice
}
```

## Best Practices

1. **Use OSV for pipelines** - When chaining operations, OSV makes the flow clear
2. **Keep functions small** - Each function should do one thing well
3. **Prefer pure functions** - Avoid side effects when possible
4. **Use descriptive names** - Function names should describe what they do
5. **Consider generic functions** - When the logic is type-agnostic

## Common Patterns

### Filter-Map-Reduce
```restrict
[1, 2, 3, 4, 5]
|> filter(|x| x % 2 == 0)
|> map(|x| x * x)
|> fold(0, |acc, x| acc + x)
```

### Function Builders
```restrict
fun make_multiplier = factor:Int {
    |x| x * factor  // Returns a closure
}

val times_three = make_multiplier(3)
val result = 7 times_three  // 21
```

## See Also

- [Lambda Expressions](../advanced/lambdas.md) - Anonymous functions and closures
- [Higher-Order Functions](../advanced/higher-order.md) - Advanced functional patterns
- [Type Inference](type-inference.md) - How function types are inferred