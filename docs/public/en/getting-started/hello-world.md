# Hello World

This tutorial builds a small Restrict program with the current v0.0.1 surface:
Warder project creation, OSV calls, scalar output, a smoke-test file, and one
record-based extension.

## Create A Project

```bash
warder new hello-world
cd hello-world
```

Warder creates a project like this:

```text
hello-world/
├── package.rl.toml
├── src/
│   └── main.rl
├── tests/
│   └── main_test.rl
└── README.md
```

Open `src/main.rl`.

## Write The First Program

Use `fun` for functions and `val` for immutable bindings:

```restrict
fun main: () -> () = {
    "Hello, World!" |> println
}
```

The important pieces are:

- `fun main: () -> () = { ... }` defines the source entry point.
- `"Hello, World!"` is a `String` value.
- `|>` is the single-argument OSV pipe.
- `println` consumes the string and prints a line.

Function-first calls are not Restrict syntax. Write `"Hello" |> println`, not
`println("Hello")`.

## Build And Run

Build the project:

```bash
warder build
```

The default output goes under `dist/`:

```text
dist/hello-world-0.1.0.wat
dist/hello-world-0.1.0.wasm
dist/hello-world-0.1.0.rgc
```

Run it:

```bash
warder run
```

You should see:

```text
Hello, World!
```

`warder run` requires Wasmtime or Wasmer on your `PATH`. If it cannot find a
runtime, run:

```bash
warder doctor
```

## Add A Function

Replace `src/main.rl` with a scalar pipeline:

```restrict
fun add_bonus: (base: Int32, bonus: Int32) -> Int32 = {
    base + bonus
}

fun main: () -> () = {
    val total = (21, 4) add_bonus
    total |> print_int
}
```

Multiple arguments use grouped OSV syntax: `(21, 4) add_bonus`.

Run again:

```bash
warder run
```

## Add A Smoke Test

Warder test files use ordinary Restrict functions. Edit `tests/main_test.rl`:

```restrict
fun add_bonus: (base: Int32, bonus: Int32) -> Int32 = {
    base + bonus
}

fun test_add_bonus: () -> Boolean = {
    (21, 4) add_bonus == 25
}
```

Run:

```bash
warder test
```

For v0.0.1, this is a type-checking smoke-test path. There is no separate
`test "name" { ... }` declaration syntax.

## Add A Record

Records use `:` in both declarations and literals:

```restrict
record Score {
    base: Int32,
    bonus: Int32
}

fun total_score: (score: Score) -> Int32 = {
    val Score { base, bonus } = score
    base + bonus
}

fun main: () -> () = {
    val score = Score { base: 21, bonus: 4 }
    val total = score |> total_score
    total |> print_int
}
```

Destructuring the record once is the clearest way to use multiple fields under
affine ownership.

## Try Type Inference

Local bindings and local return types can often be inferred:

```restrict
fun double: (value: Int32) = {
    value * 2
}

fun main: () -> () = {
    val total = 21 |> double
    total |> print_int
}
```

Keep public or exported host-visible functions explicit when the boundary
matters.

## Common Mistakes

Use current syntax:

```text
val value = 42
mut val counter = 0
value |> function
(left, right) combine
RecordName { field: value }
```

Avoid removed or unsupported forms:

```text
let value = 42
val mut counter = 0
function(value)
RecordName { field = value }
```

## What Is Outside This Tutorial

The first runnable path avoids APIs that are not part of the v0.0.1
compiler-registered standard-library surface:

- stdin and interactive prompts
- file-system and network I/O
- direct host ABI for strings, records, lists, `Option`, or `Result`
- Temporal Affine Types
- user-defined `enum`/ADT declarations

Use the [Quick Start](./quick-start.md) for a shorter command reference, then
continue with [OSV Word Order](../guide/osv-order.md), [Type Inference](../guide/type-inference.md),
and [WebAssembly Integration](../advanced/wasm.md).
