# Keywords Reference

Restrict reserves a compact set of words for current syntax and future design
space. A reserved word is not always implemented syntax in v0.0.1; when a word
is reserved but not current, this page says so explicitly.

## Current Declaration Keywords

| Keyword | Use |
| --- | --- |
| `fun` | Define a function. |
| `val` | Define an immutable binding. |
| `mut` | Mark a binding mutable as `mut val`. |
| `record` | Define a record type. |
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
| `None` | Built-in `Option<T>` absent constructor. |
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

`Ok` and `Err` are built-in `Result<T, E>` constructors. They are constructor
names rather than reserved keywords, but they are part of the current source
surface:

```restrict
fun checked_score: (score: Int32) -> Result<Int32, String> = {
    score >= 0 then {
        Ok(score)
    } else {
        Err("negative")
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
| `enum` | Reserved. User-defined enum/ADT declarations are not implemented. |
| `temporal` | Reserved for Temporal Affine Types, outside the default gate. |
| `within` | Reserved for temporal/lifetime syntax. |
| `where` | Reserved for future type constraints. |
| `lifetime` | Reserved for lifetime syntax. |
| `await` | Reserved for async design. |
| `spawn` | Reserved for concurrency design. |

Do not use reserved words as identifiers. If documentation or examples need a
feature from this table, mark it as future or experimental instead of presenting
it as current v0.0.1 syntax.
