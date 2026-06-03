# Operators Reference

This page summarizes the v0.0.1 operator surface. Restrict keeps calls OSV, so
operators either combine expressions directly or route a value into a function.

## Arithmetic

| Operator | Meaning | Notes |
| --- | --- | --- |
| `+` | Addition or string concatenation | Numeric addition; `String + String` concatenates. |
| `-` | Subtraction | Numeric. |
| `*` | Multiplication | Numeric. |
| `/` | Division | Numeric. |
| `%` | Remainder | Numeric. |

```restrict
fun arithmetic_score: () -> Int32 = {
    val base = 10 + 5 * 2
    val adjusted = base - 3
    val divided = adjusted / 2
    divided + (adjusted % 2)
}

fun greeting: () -> String = {
    "Hello, " + "Restrict"
}
```

## Comparison And Equality

| Operator | Meaning |
| --- | --- |
| `==` | Equal |
| `!=` | Not equal |
| `<` | Less than |
| `<=` | Less than or equal |
| `>` | Greater than |
| `>=` | Greater than or equal |

```restrict
fun passing: (score: Int32) -> Boolean = {
    score >= 80
}

fun same_label: (left: String, right: String) -> Boolean = {
    left == right
}
```

## Boolean Operators

| Operator | Meaning |
| --- | --- |
| `&&` | Logical and |
| `||` | Logical or |
| `!` | Logical not |

```restrict
fun should_release: (tests_pass: Boolean, warnings_clear: Boolean) -> Boolean = {
    tests_pass && warnings_clear
}

fun should_hold: (blocked: Boolean, degraded: Boolean) -> Boolean = {
    blocked || degraded
}

fun invert: (value: Boolean) -> Boolean = {
    !value
}
```

## Pipe Operator

`|>` sends the value on the left to the function on the right:

```restrict
fun increment: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () -> Int32 = {
    41 |> increment
}
```

The pipe is the single-argument OSV call form. Multiple arguments use grouped
OSV calls instead:

```restrict
fun add: (left: Int32, right: Int32) -> Int32 = {
    left + right
}

fun main: () -> Int32 = {
    (20, 22) add
}
```

The removed mutable pipe operator `|>>` is not part of v0.0.1.

## Field Access

`.` reads a field from a record value. Because records are affine values, avoid
using the same non-copy record repeatedly. Destructure once when multiple fields
are needed:

```restrict
record Reading {
    value: Int32,
    weight: Int32
}

fun weighted: (reading: Reading) -> Int32 = {
    val Reading { value, weight } = reading
    value * weight
}
```

Record update uses `.clone { ... }`:

```restrict
fun reset_weight: (reading: Reading) -> Reading = {
    reading.clone { weight: 1 }
}
```

## Conditional And Match Forms

`then`, `else`, and `match` are expression forms rather than symbolic
operators, but they behave like operators in the expression grammar:

```restrict
fun label: (score: Int32) -> String = {
    score >= 80 then {
        "pass"
    } else {
        "retry"
    }
}

fun option_or_zero: (value: Option<Int32>) -> Int32 = {
    value match {
        Some(score) => { score }
        None => { 0 }
    }
}
```

## Precedence Notes

The practical precedence order for v0.0.1 examples is:

1. grouped expressions, literals, variables, field access, and grouped OSV calls
2. unary `!`
3. `*`, `/`, `%`
4. `+`, `-`
5. comparison operators
6. equality operators
7. `&&`
8. `||`
9. pipe `|>`
10. `then`/`else` and `match`

Use parentheses when mixing grouped OSV calls with arithmetic or pipe chains.
The compiler rejects function-first calls such as `add(1, 2)`, so parentheses
around arguments are not a traditional call signal in Restrict.
