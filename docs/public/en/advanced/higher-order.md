# Higher-Order Functions and Collection Transforms

Higher-order functions receive or return function values. In Restrict, they
still follow OSV order: ordinary values and function values appear before the
function being called.

Collection transforms add one more useful reading. A higher-order verb can
open its final function parameter as a lexical scope, so the callback body can
follow the verb without turning Restrict into function-first syntax.

## Function Values Are Ordinary Arguments

A named function or lambda can be passed as an ordinary argument:

```restrict
fun double: (value: Int32) -> Int32 = {
    value * 2
}

fun apply_twice: (value: Int32, transform: Int32 -> Int32) -> Int32 = {
    value |> transform |> transform
}

fun main: () -> Int32 = {
    (5, double) apply_twice
}
```

An inline lambda uses the same grouped OSV call:

```restrict
fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3];
    (numbers, |value| value * 2) map
}
```

The container, callback, and any earlier arguments are objects of the call;
they therefore remain before `map`, `filter`, or `fold`.

## Current Collection Operations

The v0.0.1 compiler registers these higher-order operations in the prelude:

| Operation | Current input | Callback | Result |
| --- | --- | --- | --- |
| `map` | `List<T>` | `T -> U` | `List<U>` |
| `map` | `Option<T>` | `T -> U` | `Option<U>` |
| `filter` | `List<T>` | `T -> Boolean` | `List<T>` |
| `filter` | `Option<T>` | `T -> Boolean` | `Option<T>` |
| `fold` | `List<T>` plus an initial `U` | `(U, T) -> U` | `U` |

`map` transforms every list item, or the payload of `Some`. `None` remains
`None`. `filter` keeps list items for which its predicate is true. For an
`Option`, it keeps a matching `Some` or produces `None`. `fold` visits a list
from left to right, passing the accumulated value and current item to its
reducer.

`Array` and `Result` do not implement the compiler's current container
behavior. `fold` is List-only. The internal `Container.Item` and
`Container.Mapped<U>` machinery is not a source-visible form and cannot yet be
adopted by user-defined types.

## Scoped Verb Clauses

When the final remaining parameter has a function type, the verb may open that
function body as a trailing scope:

```restrict
fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3];
    numbers map {
        it * 2
    }
}
```

The unheaded block introduces one contextual binding named `it`. The compiler
uses the container item type as its expected parameter type and elaborates the
clause through an ordinary lambda:

```restrict
numbers map { it * 2 }
// Equivalent callback model:
(numbers, |it| { it * 2 }) map
```

This form is not special syntax hard-coded only for collections. Any callable
whose final remaining argument is a function can open a scope:

```restrict
fun apply: (value: Int32, transform: Int32 -> Int32) -> Int32 = {
    value |> transform
}

fun main: () -> Int32 = {
    41 apply {
        it + 1
    }
}
```

The complete `41 apply { ... }` clause produces a value just like any other
OSV call.

## Implicit and Explicit Binders

Use the implicit `it` form for a short unary callback:

```restrict
values filter {
    it > 0
}
```

Name the parameter explicitly when the body is longer or the name carries
meaning:

```restrict
values map { |value|
    val shifted = value + 1;
    shifted * 2
}
```

Callbacks with multiple parameters require explicit binders. A fold reducer
receives the accumulator first and the current item second:

```restrict
(values, 0) fold { |total, value|
    total + value
}
```

A zero-parameter scope uses an explicit empty header, `{ || ... }`. It is
useful for higher-order functions whose final parameter has type `() -> T`,
but does not apply to `map`, `filter`, or `fold`.

## Chaining Complete Clauses

Scoped verb clauses associate from left to right. Each complete clause becomes
the object of the next verb:

```restrict
fun main: () -> Int32 = {
    val values = [1, 2, 3]
    val selected = values map {
        it + 1
    } filter {
        it > 2
    }
    (selected, 0) fold { |total, value|
        total + value
    }
}
```

The program is read as:

1. map `[1, 2, 3]` to `[2, 3, 4]`;
2. filter the complete mapped result to `[3, 4]`;
3. fold that result to `7`.

A following pipe also receives the complete clause result:

```restrict
values map { it + 1 } |> list_count
```

## Type Inference

Higher-order calls provide type information in both directions:

- the container type determines the callback item type;
- the `map` body determines the mapped item type;
- a declared result type can constrain the callback result;
- `filter` requires a `Boolean` result;
- the initial value and reducer result determine the `fold` accumulator type;
- named generic functions such as `identity` can specialize from the expected
  callback type.

```restrict
fun main: () -> List<Int32> = {
    val numbers = [1, 2, 3]
    (numbers, identity) map
}
```

Empty collections and absent options can still need an annotation or another
concrete use. Restrict reports an unresolved type instead of choosing a
fallback type.

## Option Transforms

The same scoped form works for the current `Option` container behavior:

```restrict
fun increment_if_present: (value: Option<Int32>) -> Option<Int32> = {
    value map {
        it + 1
    }
}

fun keep_positive: (value: Option<Int32>) -> Option<Int32> = {
    value filter {
        it > 0
    }
}
```

This is container mapping and filtering, not a separate `option_map` API.
`fold` remains List-only in v0.0.1.

## Affine Values and Captures

Scoped callbacks use the same affine rules as ordinary lambdas and calls:

- passing a non-Copy collection binding to `map`, `filter`, or `fold` uses that
  binding in the call;
- callback parameters and captured bindings keep their normal Copy or affine
  behavior;
- braces do not grant captured affine values extra uses;
- a callback body may run zero, one, or many times according to the receiving
  function's contract.

Two nested implicit scopes would both call their focus `it`, so Restrict
rejects that form. Name at least one binder:

```restrict
groups map { |group|
    group map {
        it + 1
    }
}
```

The explicit outer `group` makes both the data flow and capture intent clear.

## Choosing a Form

Use an ordinary function argument when the callback already has a useful name
or is selected as a value:

```restrict
(numbers, normalize) map
```

Use a scoped verb clause when the callback is local to one transformation:

```restrict
numbers map {
    it + 1
}
```

Use explicit scoped binders for multiple parameters, nested scopes, or longer
bodies:

```restrict
(numbers, 0) fold { |total, number|
    total + number
}
```

All three are the same higher-order function model. The forms differ only in
how the callback is introduced and how clearly the local data flow reads.

## See Also

- [Functions](../guide/functions.md) - Function declarations, values, and types
- [OSV Word Order](../guide/osv-order.md) - Clause-level OSV composition
- [Type Inference](../guide/type-inference.md) - Bidirectional callback inference
- [Standard Library](../reference/stdlib.md) - Current prelude surface
