# v0.0.1 Release Surface

This page records the public surface that v0.0.1 documentation, examples, and
release-surface tests should agree on. `LANGUAGE_SPECIFICATION.md` remains the
authoritative language specification; this page is the shorter release contract.

## Supported

| Surface | v0.0.1 status |
| --- | --- |
| OSV-only calls | Use `value |> function`, `(arg1, arg2) function`, or `() function`. Traditional `function(args)` calls are outside the surface. |
| `val` / `mut val` bindings | Immutable bindings use `val`; mutable bindings use `mut val`. |
| Built-in generic values | `List<T>`, `Option<T>`, `Result<T, E>`, and concrete `Range<Int32>` are supported. |
| Fixed-length arrays | Source-level `Array<T, N>` is supported. Compiler-internal wildcard lengths are not a public `Array<T, 0>` contract. |
| Internal container behavior | Built-in `map`, `filter`, and `fold` use compiler-supported container behavior for current built-ins. There is no source-level user `form` syntax. |
| Source imports | Dotted source imports, named imports, wildcard imports, and whole-module imports are supported. String imports and aliases are not. |
| Records | Records are supported inside Restrict programs and across source modules. Direct host-visible record exports are not. |
| Scalar function exports | Concrete non-generic `pub fun` and `export fun` are supported when every host-visible parameter and result is scalar. |
| Scalar top-level constants | Immutable top-level literal constants can be exported when their ABI is `Int32`, `Int64`, `Float64`, `Boolean`, `Char`, or `()`. |
| Program entry | Zero-argument `main` is the executable source entry point. The generated `_start` export is a no-result wrapper that calls `$main`, drops any result, and resets the default arena. A parameterized function named `main` remains a normal function and does not emit `_start`. |

## Scalar Host ABI

The current host-visible ABI is intentionally narrow:

```text
Int32   -> i32
Int64   -> i64
Float64 -> f64
Boolean -> i32
Char    -> i32
()      -> no payload
```

Use scalar wrapper functions when a program computes with records, strings,
lists, `Option`, or `Result` internally but needs to expose a stable host entry.

```restrict
record Score {
    base: Int32,
    bonus: Int32
}

fun total_score: (score: Score) -> Int32 = {
    score.base + score.bonus
}

export fun exported_score: () -> Int32 = {
    val score = Score { base: 40, bonus: 2 }
    score |> total_score
}
```

## Current Post-v0.0.1 Addition

The current compiler adds closed user-defined enums without changing the
v0.0.1 host ABI:

| Surface | Current status |
| --- | --- |
| Closed user enums | Supported when non-generic and non-recursive. Every variant has zero or one payload. |
| Constructors and patterns | Use qualified `Type::Variant` names. Constructors use OSV order and matches are exhaustive. |
| Custom `Result` errors | A closed enum can be the error in `Result<T, CustomError>`. Error propagation still uses explicit `match`; postfix `?` remains future work. |
| `pub enum` | Exposes the enum namespace across Restrict source modules only. Enum values have no direct host-visible WebAssembly ABI. |

## Rejected With Explicit Diagnostics

| Surface | Reason |
| --- | --- |
| Traditional calls | Restrict is OSV-only. Use `(1, 2) add` or `value |> add`. |
| String imports and import aliases | v0.0.1 keeps the source module surface dotted and direct. |
| Re-exports | Import declarations stay at the source module boundary. |
| User-defined enums in the v0.0.1 tag | The v0.0.1 release rejected these declarations. This is historical; the current closed enum slice is documented above. |
| Exported generic functions | Host-visible generic ABI rules are still design work. |
| Exported composite host values | Strings, records, lists, `Option`, and `Result` do not have a direct host ABI yet. |
| Computed or mutable exported globals | Exported top-level bindings must be immutable scalar literal constants. |

## Reserved For Later

- Temporal Affine Types
- source-level `form` / `takes` declarations
- generic or recursive user enums and variants with more than one direct payload
- direct host WebAssembly ABI for user enum values
- postfix `?` error propagation
- direct generic or composite host ABI
- WebAssembly Component Model and WIT binding generation

Keeping these boundaries explicit is important: Restrict can evolve without
silently promising unstable host layouts or unfinished syntax.
