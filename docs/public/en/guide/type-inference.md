# Type Inference

Restrict uses local, bidirectional type inference around explicit program
boundaries. Function parameters, exported APIs, and record fields should keep
clear types. Function bodies, local bindings, generic call sites, and lambdas can
then use inference without drifting away from OSV syntax.

v0.0.1 note: this page documents the inference surface that is suitable for the
default release gate. User-defined `enum`/ADT syntax and the WebAssembly ABI for
exported generic functions are still design-decision gaps, so examples here
avoid them. Temporal Affine Types (TAT) remain outside the default v0.0.1 gate.
Exported records are source-level module metadata only and emit no direct
host-visible Wasm export. Built-in `Option` and `Result` remain supported and
are covered below.

## Local Bindings

Use `val` for immutable bindings. The initializer supplies the binding type:

```restrict
fun adjust_score: (score: Int32) -> Int32 = {
    val bonus = 2
    score + bonus
}

fun main: () -> Int32 = {
    40 |> adjust_score
}
```

Mutable bindings use `mut val`. The initializer is inferred the same way:

```restrict
fun seed_counter: () -> Int32 = {
    mut val count = 0
    count
}
```

## Function Returns

Return types can be inferred for local functions when the body has a clear type.
Keep public and exported function signatures explicit when the boundary matters.
Exported generic functions are still rejected by v0.0.1 release-surface
validation until Restrict has a stable concrete WebAssembly ABI for them.

```restrict
fun risk_bonus: (risk: Int32) = {
    risk * 2
}

fun final_score: (base: Int32, risk: Int32) -> Int32 = {
    base + (risk |> risk_bonus)
}
```

## Generic Calls

Generic parameters are inferred from OSV arguments:

```restrict
fun identity_local: <T>(value: T) -> T = {
    value
}

fun choose_first: <T>(value: T, fallback: T) -> T = {
    value
}

fun main: () -> String = {
    val chosen = ("primary", "fallback") choose_first
    chosen |> identity_local
}
```

When a generic function is used as a value, it needs an expected function type
from the surrounding expression:

```restrict
fun main: () -> List<Int32> = {
    val numbers: List<Int32> = [1, 2, 3]
    (numbers, identity) map
}
```

Do not rely on standalone generic function values without an expected type in
v0.0.1.

## Lambda Context

Lambda parameters are inferred from the expected function type. That expected
type usually comes from a function parameter or a prelude combinator:

```restrict
fun apply_int: (f: Int32 -> Int32, value: Int32) -> Int32 = {
    value |> f
}

fun apply_predicate: (f: Int32 -> Boolean, value: Int32) -> Boolean = {
    value |> f
}

fun main: () -> Int32 = {
    val incremented = (|x| x + 1, 41) apply_int
    val positive = (|x| x > 0, incremented) apply_predicate

    positive match {
        true => { incremented }
        false => { 0 }
    }
}
```

Typed lambda parameters can supply that parameter context when no expected
function type is available:

```restrict
fun main: () -> Int32 = {
    val bump = |value: Int32| { value + 1 }
    41 |> bump
}
```

When a lambda has no expected function type, annotate each parameter that cannot
come from context. The lambda return type is still checked from its body or from
the surrounding expected return type.

A contextless local lambda can still infer a concrete function type when local
constraints are strong enough. Body constraints, such as numeric operators, can
contribute to that type, and a later direct OSV call can supply any remaining
parameter context:

```restrict
fun main: () -> Int32 = {
    val add = |left, right| left + right
    (20, 22) add
}
```

The compiler does not treat an unconstrained contextless lambda as a generic
function value. If no later direct use or expected type supplies the missing
parameter type, the binding is rejected:

```restrict
fun main: () -> Int32 = {
    val identity = |value| value
    41
}
```

Local lambdas are allowed to be deferred until a later direct use supplies the
missing type:

```restrict
fun main: () -> Int32 = {
    val choose_left = |left, right| left
    (41, 0) choose_left
}
```

`then` and `match` branches can also produce a local callable when each branch
ends in a lambda. The callable can be stored first and resolved later from a
pipe, grouped OSV call, or container operation:

```restrict
fun add_int: (total: Int32, value: Int32) -> Int32 = {
    total + value
}

fun main: (urgent: Boolean, bonus: Option<Int32>) -> Int32 = {
    val adjust = urgent then {
        val boost = 2
        |score| score + boost
    } else {
        val factor = 2
        |score| score * factor
    }
    val normalize = bonus match {
        Some(value) => {
            val doubled = value * 2
            |score| score + doubled
        }
        None => {
            val doubled = 0
            |score| score + doubled
        }
    }
    val scores = [10, 20]
    val adjusted = (scores, adjust) map
    val normalized = (adjusted, normalize) map

    (normalized, 0, add_int) fold
}
```

Branch callable prefixes are intentionally conservative in v0.0.1. They may use
simple immutable `val` bindings whose values are replay-safe and Copy-typed.
Mutable bindings, complex patterns, and non-Copy values such as `String`,
records, lists, and functions are rejected in these prefixes.

## Unit-Returning Function Values

Function types can return `()`, such as `() -> ()` or `Int32 -> ()`. Keep these
types explicit at the parameter or binding that receives the function value,
because `()` carries no payload for inference to recover later.

Unit-returning named functions can be passed as runtime function values when an
expected function type is present:

```restrict
fun record_event: (code: Int32) -> () = {
    ()
}

fun run_event: (handler: Int32 -> (), code: Int32) -> () = {
    code |> handler
}

fun main: () -> () = {
    val handler: Int32 -> () = record_event
    (handler, 7) run_event
}
```

## Containers

Collection literals, `Ok`, `Err`, `None`, and higher-order container operations
can infer their generic types from explicit result types, sibling arguments,
sibling branches, or the surrounding expected type.

The `Container` behavior behind `map` and `filter` is compiler-internal in
v0.0.1. User-defined `form`, `takes`, `of`, and associated-type declarations are
future design work, not current source syntax.

```restrict
fun choose_list: <T>(value: List<T>, fallback: List<T>) -> List<T> = {
    value
}

fun choose_option: <T>(value: Option<T>, fallback: Option<T>) -> Option<T> = {
    value
}

fun main: () -> List<Int32> = {
    val numbers = ([], [1, 2, 3]) choose_list
    val maybe_limit = (None, Some(10)) choose_option

    maybe_limit match {
        Some(limit) => { (numbers, |n| n + limit) map }
        None => { numbers }
    }
}
```

The ambiguous forms `Ok(value)`, `Err(error)`, `None`, and `[]` require one of
those sources of context. Used alone in a local binding, they are rejected
because the compiler will not guess the missing generic type:

```restrict
fun main: () -> Int32 = {
    val result = Ok(1)
    0
}
```

Sibling branches can provide the missing type:

```restrict
fun choose_result: (flag: Boolean) -> Result<Int32, String> = {
    flag then {
        Ok(1)
    } else {
        Err("missing")
    }
}
```

## Records

Record fields use `:` in both declarations and literals. Field access preserves
the field type from the record declaration.

```restrict
record Score {
    value: Int32,
    risk: Int32
}

fun score_total: (score: Score) -> Int32 = {
    score.value
}

fun main: () -> Int32 = {
    val score = Score { value: 10, risk: 3 }
    score |> score_total
}
```

Records are supported inside Restrict programs. Exported records are supported
as source-level module metadata, but they do not emit direct host-visible Wasm
exports in v0.0.1.

## Syntax Hygiene

Current Restrict examples should use:

- `val` for immutable bindings
- `mut val`, never `val mut`
- OSV calls such as `value |> transform` and `(left, right) combine`
- `fun name: (...) -> Type = { ... }`
- record fields written with `:`
- no function-first call style
- no semicolons in guide examples
