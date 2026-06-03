# OSV Word Order

Restrict Language uses Object-Subject-Verb (OSV) word order for calls. The
value being transformed comes first, then the function that receives it. This
keeps data flow visible and avoids function-first call syntax.

## Supported Call Forms

```restrict
value |> function
(left, right) combine
() make_default
```

Function-first calls are not part of the v0.0.1 surface.

## Pipe Operator

The pipe operator passes the value on the left as the first argument to the
function on the right.

```restrict
fun double: (value: Int32) -> Int32 = {
    value * 2
}

fun main: () -> Int32 = {
    21 |> double
}
```

Pipelines read left to right:

```restrict
fun score: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () -> Int32 = {
    41
        |> score
        |> double
}
```

## Multiple Arguments

Functions with multiple arguments use a tuple before the function name.

```restrict
fun add: (left: Int32, right: Int32) -> Int32 = {
    left + right
}

fun main: () -> Int32 = {
    (10, 32) add
}
```

The same rule applies inside larger expressions:

```restrict
fun clamp_min: (value: Int32, minimum: Int32) -> Int32 = {
    value < minimum then {
        minimum
    } else {
        value
    }
}

fun main: () -> Int32 = {
    val raw = 7
    (raw, 10) clamp_min
}
```

## Collection Pipelines

Higher-order functions are still OSV. The collection and function value are
both arguments, so they appear together before `map`, `filter`, or `fold`.

```restrict
fun main: () -> Int32 = {
    val numbers: List<Int32> = [1, 2, 3, 4]
    val kept = (numbers, |n| n > 1) filter
    val doubled = (kept, |n| n * 2) map
    (doubled, 0, |total, n| total + n) fold
}
```

## Pattern Matching

`match` keeps the matched value first.

```restrict
fun score_or_zero: (score: Option<Int32>) -> Int32 = {
    score match {
        Some(value) => { value }
        None => { 0 }
    }
}
```

## Impl Functions

`impl` blocks define functions that are selected by the receiver type. They do
not introduce object-style method calls; the call still places the receiver in
the argument position.

```restrict
record Score {
    value: Int32
}

record Penalty {
    value: Int32
}

impl Score {
    fun amount: (self: Score) -> Int32 = {
        self.value
    }
}

impl Penalty {
    fun amount: (self: Penalty) -> Int32 = {
        0 - self.value
    }
}

fun main: () -> Int32 = {
    val score = Score { value: 11 }
    (score) amount
}
```

When the same function name exists for multiple record types, the receiver type
chooses the implementation.

## Record Values

Record literals use `:` field initializers, and field access is separate from
function calls.

```restrict
record Point {
    x: Int32,
    y: Int32
}

fun sum_point: (point: Point) -> Int32 = {
    point.x + point.y
}

fun main: () -> Int32 = {
    val point = Point { x: 10, y: 20 }
    point |> sum_point
}
```

## Best Practices

1. Keep pipelines short enough to scan.
2. Name functions by the transformation they perform.
3. Put side-effecting functions at the end of a pipeline.
4. Use `val` for intermediate names when it makes ownership transfer clearer.

## Summary

OSV makes ownership and data flow explicit: values move from left to right,
multi-argument calls use tuples, and `impl` functions are still called through
OSV syntax.
