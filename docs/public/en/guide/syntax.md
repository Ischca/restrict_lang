# Syntax Reference

This page summarizes the v0.0.1 syntax surface. The authoritative source is
`LANGUAGE_SPECIFICATION.md`; this guide keeps examples practical and release
oriented.

Temporal Affine Types are experimental and outside the default v0.0.1 gate.
`enum` is reserved, but user-defined enum declarations are not supported yet.

## Comments

```restrict
// Single-line comment

/*
   Multi-line comment
*/
```

## Literals

Literal forms follow the language specification. Use `()` for Unit values and
the Unit type; `Unit` is not a supported type name.

```restrict
val decimal: Int32 = 42
val hex: Int32 = 0xFF
val grouped: Int32 = 1_000_000

val ratio: Float64 = 3.14
val exponent: Float64 = 1.5e10
val negative_exponent: Float64 = 3.14E-2

val message: String = "ready\nnext"
val quote: String = "say \"ready\""
val mark: Char = 'a'
val newline: Char = '\n'

val enabled: Boolean = true
val disabled: Boolean = false
val done: () = ()
```

## Bindings

Use `val` for immutable bindings:

```restrict
val score = 42
val label: String = "ready"
val reading: Float64 = 21.5
```

Mutable bindings use `mut val`, with `mut` before `val`:

```restrict
mut val counter = 0
counter = counter + 1
```

Patterns can bind structured values:

```restrict
record Point {
    x: Int32,
    y: Int32
}

fun sum_point: (point: Point) -> Int32 = {
    val Point { x, y } = point
    x + y
}
```

## Functions

Functions use `fun name: (...) -> Type = { ... }`.

```restrict
fun add: (left: Int32, right: Int32) -> Int32 = {
    left + right
}

fun identity: <T>(value: T) -> T = {
    value
}

fun main: () -> Int32 = {
    (20, 22) add
}
```

Return types can be inferred for local functions when the body is clear:

```restrict
fun double: (value: Int32) = {
    value * 2
}
```

## Public Declarations

Use `pub` to expose a top-level function or supported constant from the
generated module:

```restrict
pub fun public_score: (value: Int32) -> Int32 = {
    value + 1
}

pub val release_bias: Int32 = 3
pub val no_payload: () = ()
```

For v0.0.1, top-level constants are limited to literal `Int32`, `Int64`,
`Float64`, `Boolean`, `Char`, and `()` values. Exported `String`, records,
lists, `Option`, and `Result` values require a composite host ABI or runtime
allocation and are rejected by release-surface validation; create them inside
functions instead.

## OSV Calls

Restrict uses object-subject-verb call order. Arguments come before the
function.

```restrict
42 |> double
(10, 20) add
() make_default
```

Function-first calls such as `add(10, 20)` are not supported.

## Lambdas

Lambda parameters are inferred from the expected function type:

```restrict
fun apply_int: (f: Int32 -> Int32, value: Int32) -> Int32 = {
    value |> f
}

fun main: () -> Int32 = {
    (|x| x + 1, 41) apply_int
}
```

Standalone lambdas can provide parameter context with annotations:

```restrict
fun main: () -> Int32 = {
    val bump = |value: Int32| value + 1
    41 |> bump
}
```

## Expressions

Supported operators include arithmetic, comparison, equality, and boolean
operators:

```restrict
val total = 1 + 2 * 3
val ready = total >= 7 && total != 0
val inverted = !ready
```

Conditionals use `then`:

```restrict
score >= 80 then {
    "pass"
} else {
    "retry"
}
```

Pattern matching follows the value:

```restrict
maybe_score match {
    Some(score) => { score }
    None => { 0 }
}
```

## Records

Record declarations and literals use `:` for fields:

```restrict
record Reading {
    celsius: Float64,
    ok: Boolean
}

fun main: () -> Float64 = {
    val reading = Reading { celsius: 21.5, ok: true }
    reading.celsius
}
```

Record updates use `.clone { ... }`:

```restrict
fun mark_ok: (reading: Reading) -> Reading = {
    reading.clone { ok: true }
}
```

`freeze` is available for prototype-style immutable copies:

```restrict
fun frozen_reading: (reading: Reading) -> Reading = {
    reading freeze
}
```

## Lists, Option, And Result

```restrict
fun count_scores: () -> Int32 = {
    val numbers: List<Int32> = [1, 2, 3];
    val maybe_score: Option<Int32> = Some(42);
    val no_score: Option<Int32> = None;
    val success: Result<Int32, String> = Ok(42);
    val failure: Result<Int32, String> = Err("invalid");
    numbers |> list_count
}
```

Empty `[]` and `None` need context from an annotation, expected return type, or
neighboring generic argument.

The example above uses local bindings. In v0.0.1, list, `Option`, and `Result`
literals are not supported as top-level constants.

## Context Blocks

Context declarations define available fields, and `with Context { ... } { ... }`
binds values for a body:

```restrict
context Policy {
    minimum_score: Int32
}

fun main: () -> Int32 = {
    with Policy { minimum_score: 80 } {
        minimum_score
    }
}
```

## Imports

Imports use dotted source-module paths. Named and wildcard imports are current
v0.0.1 syntax; string paths and aliases are reserved for a later module-design
pass.

```restrict
import release.policy.{score}
import release.policy.*
```

## Precedence

The practical precedence order is:

1. Field access and clone/freeze postfix forms
2. Unary `-` and `!`
3. `*`, `/`, `%`
4. `+`, `-`
5. `<`, `<=`, `>`, `>=`
6. `==`, `!=`
7. `&&`
8. `||`
9. OSV calls and pipes
10. Assignment in mutable bindings

Prefer parentheses when combining OSV calls with arithmetic or nested function
values.
