# Pattern Matching

Pattern matching destructures values and checks that every possible case is
handled. Restrict keeps the matched value first: `value match { ... }`.

## Basic Syntax

```restrict
value match {
    pattern_one => { result_one }
    pattern_two => { result_two }
    _ => { default_result }
}
```

Each branch body is a block expression.

## Literal Patterns

```restrict
fun describe_number: (number: Int32) -> String = {
    number match {
        0 => { "zero" }
        1 => { "one" }
        42 => { "the answer" }
        _ => { "other" }
    }
}
```

Boolean patterns use `Boolean` values:

```restrict
fun describe_flag: (enabled: Boolean) -> String = {
    enabled match {
        true => { "enabled" }
        false => { "disabled" }
    }
}
```

## Variable Binding

A bare identifier binds the matched value.

```restrict
fun double_any: (value: Int32) -> Int32 = {
    value match {
        x => { x * 2 }
    }
}
```

## Option Patterns

```restrict
fun value_or_zero: (maybe: Option<Int32>) -> Int32 = {
    maybe match {
        Some(value) => { value }
        None => { 0 }
    }
}

fun main: () -> Int32 = {
    val maybe: Option<Int32> = Some(42)
    maybe |> value_or_zero
}
```

`None` needs type context from an annotation, expected return type, sibling
branch, or argument position.

## List Patterns

```restrict
fun list_score: (numbers: List<Int32>) -> Int32 = {
    numbers match {
        [] => { 0 }
        [single] => { single }
        [first, second] => { first + second }
        [head | tail] => { head }
    }
}
```

The `[head | tail]` pattern handles non-empty lists beyond the exact-length
patterns listed before it.

## Record Patterns

Record patterns use the same `:` field spelling as record declarations and
literals.

```restrict
record Point {
    x: Int32,
    y: Int32
}

fun point_score: (point: Point) -> Int32 = {
    point match {
        Point { x: 0, y: 0 } => { 0 }
        Point { x, y } => { x + y }
    }
}
```

Fields can be rebound to different local names:

```restrict
fun point_product: (point: Point) -> Int32 = {
    point match {
        Point { x: px, y: py } => { px * py }
    }
}
```

## Nested Patterns

```restrict
record Person {
    name: String,
    age: Int32
}

record Company {
    name: String,
    employees: List<Person>
}

fun first_employee_name: (company: Company) -> String = {
    company match {
        Company { employees: [], ..._ } => {
            "No employees"
        }
        Company { employees: [Person { name, ..._ } | rest], ..._ } => {
            name
        }
    }
}
```

Nested patterns consume only the branch bindings they use. Unneeded fields can
be ignored with `..._`.

## Exhaustiveness

The type checker rejects non-exhaustive matches. Cover all known cases or add a
wildcard branch.

```restrict
fun exhaustive_option: (score: Option<Int32>) -> Int32 = {
    score match {
        Some(value) => { value }
        None => { 0 }
    }
}
```

For infinite domains such as `Int32` or `String`, include `_` unless every
possible value is represented by another pattern.

## Important Notes

- Branch bodies must be wrapped in `{ }`.
- The matched expression comes before `match`.
- `Int32`, `Boolean`, `Float64`, `Char`, and `()` are copyable in patterns.
- Heap-backed branch bindings such as `String`, `List<T>`, records, and function
  values remain affine.
- Pattern guards and tuple patterns are outside the v0.0.1 guide surface.
