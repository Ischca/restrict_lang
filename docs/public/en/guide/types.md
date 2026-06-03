# Type System

Restrict combines static typing, affine ownership, local type inference, and
WASM-friendly data layouts. This page describes the v0.0.1 type surface. TAT is
experimental and excluded from the default release gate.

## Built-In Types

The current primitive types are:

- `Int32`
- `Int64`
- `Float64`
- `Boolean`
- `Char`
- `String`
- `()` for unit

```restrict
val count: Int32 = 42
val large: Int64 = 9_000_000_000
val ratio: Float64 = 0.75
val ready: Boolean = true
val marker: Char = 'A'
val label: String = "release"
val unit: () = ()
```

`Int32`, `Boolean`, `Float64`, `Char`, and `()` are copyable. `String`, records,
collections, function values, and other heap-backed values follow affine
ownership.

## Affine Ownership

Affine values may be used at most once unless they are copyable or mutable.

```restrict
fun consume: (message: String) -> Int32 = {
    1
}

fun main: () -> Int32 = {
    val message = "hello"
    message |> consume
}
```

Copyable primitives can be used repeatedly:

```restrict
fun main: () -> Int32 = {
    val score = 40
    score + score
}
```

Mutable bindings are declared with `mut val`:

```restrict
fun main: () -> Int32 = {
    mut val counter = 0
    counter = counter + 1
    counter
}
```

## Function Types

Function types use `->`. Multi-parameter function types use parentheses.

```restrict
Int32 -> Int32
(Int32, Float64) -> Boolean
Int32 -> ()
() -> ()
```

Values of function type can be passed through OSV calls:

```restrict
fun apply_int: (f: Int32 -> Int32, value: Int32) -> Int32 = {
    value |> f
}

fun main: () -> Int32 = {
    (|x| x + 1, 41) apply_int
}
```

Unit-returning function values also use explicit expected function types:

```restrict
fun record_event: (code: Int32) -> () = {
    ()
}

fun run_event: (handler: Int32 -> (), code: Int32) -> () = {
    code |> handler
}

fun main: () -> () = {
    (record_event, 7) run_event
}
```

## Generic and Built-In Collection Types

The built-in generic and collection type forms are:

```restrict
List<T>
Array<T, N>
Option<T>
Result<T, E>
Range<Int32>
```

Common examples:

```restrict
val numbers: List<Int32> = [1, 2, 3]
val fixed: Array<Int32, 3> = [1, 2, 3]
val indexes: Range<Int32> = [1..10]
val maybe_score: Option<Int32> = Some(42)
val no_score: Option<Int32> = None
val success: Result<Int32, String> = Ok(42)
val failure: Result<Int32, String> = Err("missing")
```

Collection literals use `[ ... ]` for both lists and fixed arrays. Without an
expected type, `[1, 2, 3]` is a `List<Int32>`. In an expected `Array<T, N>`
context, the same literal is checked as an array and its length must match `N`.
The old bracket-bar array literal syntax is not supported.

Range literals use `[start..end]` and currently support `Range<Int32>` only. The
literal stores the two endpoints; iteration behavior and non-Int32 endpoint
types are intentionally not part of the v0.0.1 surface yet.

These built-ins are part of the v0.0.1 support surface. They do not imply that
user-defined generic ADTs or exported generic ABIs are complete.

Empty collection literals need context:

```restrict
fun main: () -> List<Int32> = {
    val values: List<Int32> = []
    values
}
```

The empty literal `[]` is intentionally ambiguous without an expected
`List<T>` or `Array<T, N>` type. A sibling element, branch, argument, or return
context can supply that type; otherwise add an annotation.

## Generic Functions

Generic functions use type parameters before the parameter list.

```restrict
fun identity: <T>(value: T) -> T = {
    value
}

fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> Int32 = {
    (42, 0) choose_first
}
```

Generic function values need an expected function type. This keeps inference
local and avoids hidden global guessing:

```restrict
fun main: () -> List<Int32> = {
    val numbers: List<Int32> = [1, 2, 3]
    (numbers, identity) map
}
```

## Records

Records group named fields. Field declarations and literals use `:`.

```restrict
record Reading {
    celsius: Float64,
    threshold: Float64
}

fun above_threshold: (reading: Reading) -> Boolean = {
    reading.celsius > reading.threshold
}
```

Record values are affine. Use destructuring when multiple fields are needed:

```restrict
fun delta: (reading: Reading) -> Float64 = {
    val Reading { celsius, threshold } = reading
    celsius - threshold
}
```

Record updates use `.clone { ... }`:

```restrict
fun lower_threshold: (reading: Reading) -> Reading = {
    reading.clone { threshold: 18.0 }
}
```

Generic records are supported inside Restrict programs:

```restrict
record Box<T> {
    value: T
}

fun unwrap: <T>(box: Box<T>) -> T = {
    box.value
}

fun main: () -> Int32 = {
    Box { value: 1 } |> unwrap
}
```

Generic record literals infer their type arguments from fields, expected
parameter types, or explicit annotations. Exporting records across the
WebAssembly host ABI is still outside the v0.0.1 release surface. Exported
records are source-level module metadata only and do not emit direct
host-visible Wasm exports.

## Option And Result

`Option` and `Result` are built-in generic sum types with constructor and
pattern syntax.

```restrict
fun score_or_zero: (score: Option<Int32>) -> Int32 = {
    score match {
        Some(value) => { value }
        None => { 0 }
    }
}

fun result_or_zero: (score: Result<Int32, String>) -> Int32 = {
    score match {
        Ok(value) => { value }
        Err(message) => { 0 }
    }
}
```

The constructors `Some(value)`, `Ok(value)`, and `Err(error)` carry payload
types, but `Ok` and `Err` still need the full expected `Result<T, E>` shape to
fill the opposite side. `None` carries no payload, so it needs an expected
`Option<T>` unless a sibling `Some` or branch supplies the type.

```restrict
fun choose_result: (flag: Boolean) -> Result<Int32, String> = {
    flag then {
        Ok(42)
    } else {
        Err("missing")
    }
}

fun choose_option: (flag: Boolean) -> Option<Int32> = {
    flag then {
        None
    } else {
        Some(42)
    }
}
```

User-defined `enum`/ADT declarations are reserved but not implemented in the
v0.0.1 gate. This is an intentional design-decision gap, not a statement that
sum types are absent from Restrict. Use built-in `Option` and `Result` for
sum-type workflows today while the user-defined ADT syntax, exhaustiveness
rules, and export ABI are designed.

## Type Inference

Restrict infers local bindings, function returns, generic call sites, and lambda
parameters when there is enough local context.

```restrict
fun main: () -> Int32 = {
    val add_one = |value: Int32| value + 1
    41 |> add_one
}
```

A contextless local lambda can infer a concrete function type when local
constraints are enough to finish it. Body constraints such as
`|value| value + 1` can contribute, and a later direct OSV call can supply
remaining parameter context.

Unconstrained lambdas such as `|value| value` are rejected unless an expected
function type or a later local direct use supplies the parameter type.

If the compiler cannot infer an empty collection, `Ok`, `Err`, `None`, or a
lambda parameter, add a type annotation at the nearest useful boundary.

## Current Boundaries

These are outside the default v0.0.1 support promise:

- Temporal Affine Type inference
- User-defined `enum`/ADT declarations; built-in `Option` and `Result` remain supported
- Direct WebAssembly ABI for exported generic functions and host-visible record values
- User-defined `form`, `takes`, `of`, traits/typeclasses, and associated-type declarations
- Borrowed slices/references
- Traditional function-first calls

Reserved words remain reserved so the syntax can grow without breaking source
compatibility. Exported generic functions and host-visible record values require
concrete host ABI decisions before they become a WebAssembly codegen feature.
