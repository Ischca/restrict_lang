# Context Binding

Context binding is current v0.0.1 syntax. It uses the same `with` expression
shape as explicit arenas:

```restrict
with ContextName { field: value } {
    body
}
```

The first block binds context fields. The second block is the expression body.
Field bindings use record field syntax with `:`.

## Declaring A Context

A context declares the field names and types that a `with` expression may bind.

```restrict
context Config {
    limit: Int32
}

fun main: () -> Int32 = {
    val source = 41
    with Config { limit: source } {
        limit + 1
    }
}
```

The bound field is available only inside the body block. Unknown fields are
rejected, and field values are checked against the context declaration.

## Expected Types

Context field types provide expected types to ambiguous expressions. This is
useful for empty collections:

```restrict
context Bucket {
    items: List<Int32>
}

fun main: () -> List<Int32> = {
    with Bucket { items: [] } {
        items
    }
}
```

The context declaration tells the checker that `[]` is a `List<Int32>`.

## Arena Is A Built-In Context

`Arena` is built in. It does not require a user `context Arena` declaration, and
it normally has no field bindings:

```restrict
fun main: () -> Int32 = {
    with Arena { } {
        val values = [1, 2, 3]
        values |> list_count
    }
}
```

Heap-backed values created in that explicit arena cannot escape the arena body.
Return a scalar summary such as an `Int32`, `Boolean`, `Char`, `Float64`,
`Int64`, or `()` when the result must leave the block.

```restrict
fun invalid: () -> List<Int32> = {
    with Arena { } {
        [1, 2, 3]
    }
}
```

## Current Scope

Context binding is not Temporal Affine Type syntax. Function-level context
annotations and lifetime forms are outside the v0.0.1 default gate. Use
`with Context { bindings } { body }` for current code.
