# Functions

Functions are first-class values in Restrict Language. Arguments come before
the function name, so calls read in OSV order and compose naturally with `|>`.

## Function Definition

Basic function syntax:

```restrict
fun add: (left: Int32, right: Int32) -> Int32 = {
    left + right
}
```

Functions without parameters use `()`:

```restrict
fun get_answer: () -> Int32 = {
    42
}
```

Return types can be inferred when the body has enough information:

```restrict
fun double: (value: Int32) = {
    value * 2
}
```

## OSV Calls

Restrict supports OSV calls only. The object value or argument tuple comes
first, then the function name.

```restrict
val sum = (5, 10) add
val doubled = 21 |> double
val answer = () get_answer
```

Traditional calls such as `add(5, 10)` and `get_answer()` are rejected by the
parser.

## Function Composition

Chain small functions by passing each result to the next call:

```restrict
fun increment: (value: Int32) -> Int32 = {
    value + 1
}

fun double: (value: Int32) -> Int32 = {
    value * 2
}

fun square: (value: Int32) -> Int32 = {
    value * value
}

fun main: () -> Int32 = {
    val stepped = 5 |> increment
    val doubled = stepped |> double
    doubled |> square
}
```

The pipe operator is equivalent for single-argument calls:

```restrict
fun main: () -> Int32 = {
    5 |> increment |> double |> square
}
```

## Higher-Order Functions

Functions can accept and return other functions:

```restrict
fun apply_twice: (value: Int32, f: Int32 -> Int32) -> Int32 = {
    value |> f |> f
}

fun double: (value: Int32) -> Int32 = {
    value * 2
}

fun main: () -> Int32 = {
    (5, double) apply_twice
}
```

## Generic Functions

Generic parameters appear between the function name and parameter list:

```restrict
fun identity: <T>(value: T) -> T = {
    value
}

fun keep_some: <T>(value: T) -> Option<T> = {
    Some(value)
}
```

## Function Values

Lambdas can be stored in variables. Add parameter annotations when there is no
expected type from a surrounding function signature, return type, or call site.

```restrict
val add_five: Int32 -> Int32 = |value: Int32| value + 5
val multiply: (Int32, Int32) -> Int32 = |left: Int32, right: Int32| left * right

val result = 10 |> add_five
val product = (3, 4) multiply
```

## Recursive Functions

Recursive functions use the same OSV call form:

```restrict
fun factorial: (value: Int32) -> Int32 = {
    value <= 1 then {
        1
    } else {
        val next = value - 1
        value * (next |> factorial)
    }
}

fun fibonacci: (value: Int32) -> Int32 = {
    value match {
        0 => { 0 }
        1 => { 1 }
        _ => {
            val one_back = value - 1
            val two_back = value - 2
            (one_back |> fibonacci) + (two_back |> fibonacci)
        }
    }
}
```

## Impl Methods

`impl` blocks attach functions to a record type, but calls still use OSV form.

```restrict
record Point {
    x: Int32
    y: Int32
}

impl Point {
    fun squared_distance: (self: Point, other: Point) -> Int32 = {
        val Point { x: x1, y: y1 } = self
        val Point { x: x2, y: y2 } = other
        val dx = x1 - x2
        val dy = y1 - y2
        (dx * dx) + (dy * dy)
    }
}

fun main: () -> Int32 = {
    val p1 = Point { x: 0, y: 0 }
    val p2 = Point { x: 3, y: 4 }
    (p1, p2) squared_distance
}
```

Traditional dot calls such as `p1.squared_distance(p2)` are not part of the
language.

## Function Type Annotations

Explicit function type syntax:

```restrict
val double_fn: Int32 -> Int32 = |value: Int32| value * 2
val add_fn: (Int32, Int32) -> Int32 = |left: Int32, right: Int32| left + right

val result = 10 |> double_fn
val sum = (3, 4) add_fn
```

## Common Patterns

### Wrapping Arguments

Use a lambda to bind one argument and keep the resulting value callable:

```restrict
fun add: (left: Int32, right: Int32) -> Int32 = {
    left + right
}

val add_five: Int32 -> Int32 = |right: Int32| (5, right) add
val result = 10 |> add_five
```

### Option Mapping

Higher-order functions work naturally with `match`:

```restrict
fun map_option: <T, U>(value: Option<T>, f: T -> U) -> Option<U> = {
    value match {
        Some(inner) => { Some(inner |> f) }
        None => { None }
    }
}
```

## Best Practices

1. **Use OSV calls consistently** - They are the only supported function call form
2. **Annotate public boundaries** - Return and parameter annotations make contracts clear
3. **Let local returns infer** - Omit return annotations only when the body is obvious
4. **Keep affine values moving** - Destructure records when multiple fields are needed
5. **Use lambdas for adapters** - Wrap fixed arguments or expected callback shapes explicitly

## See Also

- [Type Inference](type-inference.md) - How function types are inferred
- [Types](types.md) - Function type syntax and generic containers
- [Syntax](syntax.md) - Current v0.0.1 syntax reference
