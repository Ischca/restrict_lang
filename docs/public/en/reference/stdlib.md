# Standard Library Reference

This page documents the v0.0.1 standard-library surface that is currently
registered by the compiler. The files under `std/` are source-adjacent indexes
for readers and tests; the runtime behavior is implemented in the compiler and
WebAssembly code generator.

The current standard library is intentionally small. APIs listed as absent below
are not user-facing v0.0.1 features, even if their names appear in older
examples or design sketches.

## Calling Convention

Standard-library calls use the same OSV syntax as user functions:

```restrict
"hello" |> println
42 |> print_int
(left, right) max
values |> list_count
(maybe_value, fallback) option_unwrap_or
```

Traditional calls such as `println("hello")`, `max(left, right)`, or
`value.method()` are not supported.

Examples in this reference show expression or local-binding forms unless they
explicitly use `pub val`. In v0.0.1, top-level constants are limited to literal
`Int32`, `Int64`, `Float64`, `Boolean`, `Char`, and `()` values. Exported
`String`, lists, records, `Option`, and `Result` values require a composite host
ABI or runtime allocation and are not part of the top-level global export
surface.

## Prelude

The prelude contains generic function helpers and basic boolean/assertion
helpers:

```text
identity: <T>(T) -> T
map: generic container mapping builtin
filter: generic container filtering builtin
fold: generic List reduction builtin
not: (Boolean) -> Boolean
and: (Boolean, Boolean) -> Boolean
or: (Boolean, Boolean) -> Boolean
assert: (Boolean, String) -> ()
panic: (String) -> ()
```

Canonical call shapes:

```restrict
value |> identity
condition |> not
(left, right) and
(condition, "expected condition to hold") assert
```

Helpers such as `xor`, `eq`, `ne`, `when`, and `debug_assert` are not part of
the current compiler-registered surface.

## IO

Current IO functions:

```text
println: (String) -> ()
print: (String) -> ()
print_int: (Int32) -> ()
print_float: (Float64) -> ()
eprint: (String) -> ()
eprintln: (String) -> ()
```

Canonical call shapes:

```restrict
"hello" |> println
"hello" |> print
42 |> print_int
3.14 |> print_float
"error" |> eprintln
```

Stdin and file APIs are outside the v0.0.1 std surface. That includes
`readLine`, `readFile`, `writeFile`, path metadata, directory operations, and
fallible `?`-style IO flows.

## Lists

Current list functions:

```text
list_is_empty: <T>(List<T>) -> Boolean
list_head: <T>(List<T>) -> Option<T>
list_tail: <T>(List<T>) -> Option<List<T>>
list_reverse: <T>(List<T>) -> List<T>
list_prepend: <T>(T, List<T>) -> List<T>
list_append: <T>(List<T>, T) -> List<T>
list_concat: <T>(List<T>, List<T>) -> List<T>
list_count: <T>(List<T>) -> Int32
list_length: <T>(List<T>) -> Int32
list_get: <T>(List<T>, Int32) -> T
```

Canonical call shapes:

```restrict
values |> list_is_empty
values |> list_head
values |> list_tail
values |> list_reverse
(item, values) list_prepend
(values, item) list_append
(left, right) list_concat
values |> list_count
values |> list_length
(values, index) list_get
```

Collection literals use `[1, 2, 3]`. Without an expected type, the literal is a
`List<T>`; in an expected `Array<T, N>` context, the same literal is checked as a
fixed-size array. The bracket-bar array literal form is removed.

Dedicated `list_map`, `list_filter`, and `list_fold_left` helpers are not part
of the current std surface. Use the compiler-registered prelude `map`, `filter`,
and `fold` builtins where the type checker has the required context.

## Math

Current math functions:

```text
abs: (Int32) -> Int32
max: (Int32, Int32) -> Int32
min: (Int32, Int32) -> Int32
pow: (Int32, Int32) -> Int32
factorial: (Int32) -> Int32
abs_f: (Float64) -> Float64
max_f: (Float64, Float64) -> Float64
min_f: (Float64, Float64) -> Float64
```

Canonical call shapes:

```restrict
value |> abs
(left, right) max
(left, right) min
(base, exponent) pow
value |> factorial
value |> abs_f
(left, right) max_f
(left, right) min_f
```

Floating-point trig, logarithm, random-number, SIMD, and wider numeric
conversion helpers are outside the current std surface.

## Option

`Option<T>` is a built-in generic sum type. Current option helpers:

```text
option_is_some: <T>(Option<T>) -> Boolean
option_is_none: <T>(Option<T>) -> Boolean
option_unwrap_or: <T>(Option<T>, T) -> T
```

Source-level constructors:

```restrict
fun choose_score: () -> Int32 = {
    val maybe_score: Option<Int32> = Some(42);
    val no_score: Option<Int32> = None;
    (maybe_score, 0) option_unwrap_or
}
```

Canonical call shapes:

```restrict
maybe_value |> option_is_some
maybe_value |> option_is_none
(maybe_value, fallback) option_unwrap_or
```

Higher-order option helpers such as `option_map`, `option_flatten`,
`option_and_then`, `option_zip`, and `option_to_list` are not part of the current
std surface.

## Result

`Result<T, E>` is a built-in generic sum type with constructor and pattern
support:

```restrict
fun choose_result: (flag: Boolean) -> Result<Int32, String> = {
    flag then {
        Ok(42)
    } else {
        Err("missing")
    }
}
```

There are no dedicated std result helper functions in the v0.0.1 surface.
Use `match` for result handling:

```restrict
fun result_or_zero: (score: Result<Int32, String>) -> Int32 = {
    score match {
        Ok(value) => { value }
        Err(message) => { 0 }
    }
}
```

There is no v0.0.1 error-propagation syntax; use `match` for result handling.

## Strings

Current string operations are expression forms, not std functions:

```text
a + b: concatenate two String values
a == b: compare String contents
a != b: compare String contents and negate the result
```

Canonical expression shapes:

```restrict
first + second
first == second
first != second
```

The code generator lowers these through runtime helpers such as
`string_concat` and `string_eq`. Length, parsing, formatting, case conversion,
splitting, and trimming helpers are outside the current std surface.

## Outside The v0.0.1 Std Surface

The following areas are absent from the compiler-registered v0.0.1 surface:

- File-system and path APIs
- Time and date APIs
- Synchronization primitives
- Borrowing/reference-oriented memory helpers
- Conversion traits
- Hashing traits
- Display/debug formatting traits
- Random-number APIs
- Networking APIs
- Environment and process APIs
- User-defined traits/typeclasses and associated types

When adding std documentation, document only APIs backed by compiler/runtime
behavior and keep design sketches out of current-reference examples.
