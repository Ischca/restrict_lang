# Forms and Static Polymorphism

Forms are Restrict's explicit behavioral contracts. They let a generic
function require named behavior while preserving OSV calls, affine ownership,
and direct WebAssembly calls.

The v0.0.1 slice is intentionally small:

- a form is non-generic and contains required method signatures only;
- every method is fully typed and starts with `self: Self`;
- a concrete, non-generic record adopts a form with `takes`;
- generic type parameters list requirements with `<T of A + B>`; and
- the compiler monomorphizes every resolved call without a vtable or runtime
  form dictionary.

## Declare and adopt a form

```restrict
pub form Labelled {
    fun label: (self: Self) -> String
}

record Badge {
    text: String
}

Badge takes Labelled {
    fun label: (self: Badge) -> String = {
        self.text
    }
}

fun read_label: <T of Labelled>(value: T) -> String = {
    value |> label
}

fun main: () -> () = {
    Badge { text: "release candidate" } |> read_label |> println
}
```

Conformance is explicit. A record with an ordinary `impl` method named
`label` does not satisfy `Labelled` until it has a matching `takes`
declaration. A `takes` block must provide exactly the form's methods with the
same positional parameter and return types, replacing `Self` with the record
type. Parameter names may differ from the contract.

`pub form` makes the contract importable by another Restrict source module.
The `takes` declaration itself cannot be marked `pub`; export the form and
nominal record instead.

## Multiple bounds

`+` means that the concrete type must satisfy every listed form:

```text
fun inspect: <T of Display + Labelled>(value: T) -> String = { ... }
```

Ordinary `impl` methods and form adoptions for one concrete type share one
selector namespace in this initial slice, so a type cannot expose the same
selector through two declarations. Multiple generic bounds that expose the
same selector are also ambiguous. The compiler reports an error instead of
assigning a priority.

## Affine receivers

A form method is an ordinary affine function. Passing a non-Copy record as
`self` consumes it. Resolution is part of type inference and does not borrow,
copy, or replay the expression.

```restrict
fun consume_badge: (badge: Badge) -> String = {
    badge |> label
}
```

Copy scalars remain reusable according to the existing structural Copy rules.
Adopting a form never makes a type Copy.

## Display and output

The compiler provides the following form:

```restrict
pub form Display {
    fun display: (self: Self) -> String
}
```

`String`, `Int32`, `Int64`, `Float64`, `Boolean`, `Char`, and `()` have
compiler-provided Display adoptions. Records opt in explicitly:

```restrict
record Notice {
    text: String
}

Notice takes Display {
    fun display: (self: Notice) -> String = {
        self.text
    }
}

fun main: () -> () = {
    42 |> print
    " · " |> print
    Notice { text: "records too" } |> println
}
```

`print` and `println` accept any Display value. `eprint` and `eprintln` remain
String-only; `print_int` and `print_float` remain compatibility helpers.
`display`, `print`, and `println` are compiler-reserved direct call targets;
they cannot be declared as top-level source functions or ordinary/custom-form
method selectors. The builtins themselves cannot yet be captured as
first-class function values. The `display` method inside
`RecordName takes Display` is the one method-name exception.

## Deliberate limits

The initial slice does not include associated types, generic forms,
generic or conditional `takes`, default methods, enum adoptions, negative
bounds, selective adoption imports, existential form values, or dynamic
dispatch. The compiler-internal Container projections used by collection
builtins do not make `Container` or associated-type syntax source-visible.
