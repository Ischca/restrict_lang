# Single Ownership Rule

Restrict tracks one owner for affine values. A heap-backed value can be used
zero or one time from a binding. A use that passes, returns, assigns, or
destructures the value moves it.

Primitive scalar values copy instead of move. The copyable scalar set is
`Int32`, `Int64`, `Float64`, `Boolean`, `Char`, and `()`.

## Values That Move

`String`, `List<T>`, `Array<T, N>`, records, function values, and other
heap-backed values are affine.

```restrict
fun accept_message: (message: String) -> Int32 = {
    1
}

fun main: () -> Int32 = {
    val message = "ready"
    message |> accept_message
}
```

The `message` binding is consumed by the OSV call. A second use of `message`
after that call is rejected.

Lists and arrays follow the same rule:

```restrict
fun count_items: (items: List<Int32>) -> Int32 = {
    items |> list_count
}

fun main: () -> Int32 = {
    val items = [1, 2, 3]
    items |> count_items
}
```

## Values That Copy

Scalars can be read more than once because each read copies the value:

```restrict
fun main: () -> Int32 = {
    val score = 40
    score >= 40 then {
        score + score
    } else {
        0
    }
}
```

This is why arithmetic over `Int32` values does not consume `score` after the
first read.

## Records

Record fields and literals use `:`. Record values are affine, so destructure a
record once when several fields are needed:

```restrict
record Reading {
    celsius: Float64,
    threshold: Float64
}

fun delta: (reading: Reading) -> Float64 = {
    val Reading { celsius, threshold } = reading
    celsius - threshold
}
```

The destructuring move consumes `reading` and creates new bindings for the
fields.

## Mutable Slots

Use `mut val` when a local slot must change:

```restrict
fun main: () -> Int32 = {
    mut val total = 0
    total = total + 1
    total = total + 2
    total
}
```

`mut val` changes the binding slot. It is not a clone operation for heap-backed
values.

## Arena Interaction

Single ownership decides how often a binding may be used. Arenas decide where
heap-backed values live.

```restrict
fun main: () -> Int32 = {
    val count = with Arena { } {
        val items = [1, 2, 3]
        items |> list_count
    }
    count
}
```

The `items` binding moves into `list_count`. The list allocation stays in the
explicit arena. Only the scalar `count` escapes.
