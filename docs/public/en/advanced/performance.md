# Performance Tips

Restrict is designed for deterministic WebAssembly output, affine ownership,
and no WebAssembly GC dependency. v0.0.1 performance guidance is therefore
mostly about keeping ownership clear, avoiding unstable host ABI assumptions,
and writing examples the compiler can lower predictably.

## Keep Host Boundaries Scalar

Scalar exports are the stable host-facing path:

```restrict
export fun exported_score: (base: Int32, bonus: Int32) -> Int32 = {
    base + bonus
}
```

When a computation needs records or lists, build them inside the function and
return a scalar summary. That avoids depending on an unfinished composite host
layout.

## Prefer Straight Pipelines

OSV pipelines keep data flow explicit:

```restrict
fun normalize: (score: Int32) -> Int32 = {
    score > 100 then {
        100
    } else {
        score
    }
}

fun add_bonus: (score: Int32) -> Int32 = {
    score + 5
}

fun main: () -> Int32 = {
    96 |> add_bonus |> normalize
}
```

For multi-argument operations, use grouped OSV calls so evaluation stays
obvious:

```restrict
fun weighted: (score: Int32, weight: Int32) -> Int32 = {
    score * weight
}

fun main: () -> Int32 = {
    (21, 2) weighted
}
```

## Destructure Records Once

Records are affine values. If multiple fields are needed, destructure once
instead of repeatedly accessing the same record:

```restrict
record Reading {
    value: Int32,
    weight: Int32
}

fun weighted_reading: (reading: Reading) -> Int32 = {
    val Reading { value, weight } = reading
    value * weight
}
```

This also makes ownership intent clearer to readers and tests.

## Give Ambiguous Values Context

Empty lists, `None`, `Ok`, and `Err` often need expected type context:

```restrict
fun empty_scores: () -> List<Int32> = {
    []
}

fun no_score: () -> Option<Int32> = {
    None
}
```

Supplying the context early reduces inference work and improves diagnostics.

## Build Modes

For compiler development:

```bash
mise exec -- cargo build
mise exec -- cargo test
```

For the browser compiler:

```bash
wasm-pack build --target web --out-dir web/pkg
```

For Pages:

```bash
mdbook build docs
wasm-pack build --target web --out-dir web/pkg
bash scripts/build-pages.sh
```

## Current Non-Goals

Do not write performance-sensitive code that depends on:

- direct host access to composite memory layout
- WebAssembly Component Model lowering
- WIT-generated adapters
- Temporal Affine Type cleanup
- user-defined ADT layout

Those areas are reserved for later design and should not be treated as current
performance contracts.
