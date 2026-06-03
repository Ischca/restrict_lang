# Ownership and Arenas

Restrict v0.0.1 uses two simple ownership rules:

- heap-backed values live in the current arena
- affine values move when used, while primitive values copy

This is the current v0.0.1 model. Temporal Affine Types are experimental and
outside the default gate.

## Current Arena

Heap-backed values are allocated in the current arena. That includes `String`,
`List<T>`, `Array<T, N>`, records, and other non-scalar values as they apply to
the current implementation.

The host entry point, normally `main`, gets a default arena automatically:

```restrict
fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3, 4, 5]
    numbers
}
```

Helper functions do not create their own default arena. They inherit the
current arena from their caller unless they enter an explicit arena.

## Explicit Arena

Use `with Arena { } { ... }` when temporary heap-backed values should be freed
at the end of a block. The first block is the context binding block. `Arena`
does not need fields, so the binding block is empty.

```restrict
fun main: () -> Int32 = {
    val count = with Arena { } {
        val numbers = [1, 2, 3, 4, 5]
        numbers |> list_count
    }
    count
}
```

The list is created inside the explicit arena. The `Int32` count is a scalar,
so it can leave the arena block safely.

## Escape Rules

Scalars can escape an explicit arena:

```restrict
fun main: () -> Int32 = {
    val result = with Arena { } {
        42
    }
    result
}
```

The scalar escape set is `Int32`, `Int64`, `Float64`, `Boolean`, `Char`, and
`()`.

Heap-backed values cannot be returned from an explicit arena:

```restrict
fun invalid_list: () -> List<Int32> = {
    with Arena { } {
        [1, 2, 3]
    }
}

fun invalid_text: () -> String = {
    with Arena { } {
        "arena text"
    }
}
```

Records are non-scalar for the explicit arena escape check:

```restrict
record Report {
    id: Int32,
    title: String
}

fun invalid_report: () -> Report = {
    with Arena { } {
        Report { id: 7, title: "daily" }
    }
}
```

Move heap-backed data out before entering the arena, or return a scalar summary
from the arena block.

## Affine Movement

Heap-backed values are affine. Passing one to a function moves it:

```restrict
fun count_items: (items: List<Int32>) -> Int32 = {
    items |> list_count
}

fun main: () -> Int32 = {
    val items = [1, 2, 3]
    items |> count_items
}
```

After `items |> count_items`, the `items` binding has been consumed.

Primitive values copy, so repeated reads are allowed:

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

Use `mut val` when a local slot must be reassigned:

```restrict
fun main: () -> Int32 = {
    mut val total = 0
    total = total + 1
    total = total + 2
    total
}
```
