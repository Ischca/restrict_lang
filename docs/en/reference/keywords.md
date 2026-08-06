# Keywords Reference

Restrict reserves a compact set of words for current syntax and future design
space. Some words were reserved but not implemented in v0.0.1; this page also
records current post-v0.0.1 additions explicitly.

## Current Declaration Keywords

| Keyword | Use |
| --- | --- |
| `fun` | Define a function. |
| `val` | Define an immutable binding. |
| `mut` | Mark a binding mutable as `mut val`. |
| `record` | Define a record type. |
| `enum` | Define a closed, non-generic, non-recursive user enum. |
| `context` | Define a context binding shape. |
| `pub` | Expose a supported top-level declaration from a source module. |
| `import` | Import from dotted source modules. |
| `export` | Expose a host-visible function or supported scalar constant. |
| `impl` | Define type-directed functions for a receiver type. |
| `as` | Reserved for import/type naming positions; aliases are outside v0.0.1 source imports. |

Examples:

```restrict
record Score {
    value: Int32
}

fun bump: (score: Int32) -> Int32 = {
    score + 1
}

export fun exported_score: () -> Int32 = {
    41 |> bump
}
```

User enum variants have zero or one payload and stay qualified under the enum
name:

```restrict
pub enum ParseError {
    Empty
    Message(String)
}

fun make_error: (message: String) -> ParseError = {
    message |> ParseError::Message
}
```

`pub enum` exposes the type to Restrict source modules only; it does not define
a host enum ABI. Generic or recursive enums and `?` propagation remain future
work.

Mutable bindings place `mut` before `val`:

```restrict
fun next_counter: () -> Int32 = {
    mut val counter = 0
    counter = counter + 1
    counter
}
```

## Current Expression Keywords

| Keyword | Use |
| --- | --- |
| `then` | Introduce the true branch of a conditional expression. |
| `else` | Introduce the false branch. |
| `match` | Pattern-match the value on its left. |
| `true` | Boolean true literal. |
| `false` | Boolean false literal. |
| `Some` | Built-in `Option<T>` present constructor. |
| `None` | Built-in `Option<T>` absent match pattern; construct with `() Option::None`. |
| `with` | Bind a context value for a block. |

```restrict
fun label: (score: Int32) -> String = {
    score >= 80 then {
        "pass"
    } else {
        "retry"
    }
}

fun option_score: (value: Option<Int32>) -> Int32 = {
    value match {
        Some(score) => { score }
        None => { 0 }
    }
}
```

`Result::Ok` and `Result::Err` are qualified built-in `Result<T, E>`
constructors. The unqualified names remain match patterns; value construction
uses the qualified namespace:

```restrict
fun checked_score: (score: Int32) -> Result<Int32, String> = {
    score >= 0 then {
        score Result::Ok
    } else {
        "negative" Result::Err
    }
}
```

## Prototype And Ownership Keywords

| Keyword | Use |
| --- | --- |
| `clone` | Record clone/update operation as `.clone { ... }`. |
| `freeze` | Create an immutable prototype-style value. |
| `fatal` | Reserved for fatal diagnostics and termination-oriented design. |

```restrict
record Reading {
    value: Int32,
    valid: Boolean
}

fun mark_valid: (reading: Reading) -> Reading = {
    reading.clone { valid: true }
}
```

## Reserved Or Experimental

| Keyword | v0.0.1 status |
| --- | --- |
| `temporal` | Reserved for Temporal Affine Types, outside the default gate. |
| `within` | Reserved for temporal/lifetime syntax. |
| `where` | Reserved for future type constraints. |
| `lifetime` | Reserved for lifetime syntax. |
| `await` | Reserved for async design. |
| `spawn` | Reserved for concurrency design. |

Do not use reserved words as identifiers. If documentation or examples need a
feature from this table, mark it as future or experimental instead of
presenting it as current syntax.
