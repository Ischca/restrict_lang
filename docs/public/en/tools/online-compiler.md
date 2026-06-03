# Online Compiler

The GitHub Pages site publishes the browser compiler beside this mdBook:

```text
/docs/      mdBook documentation
/compiler/  online compiler
/blog/      implementation notes
```

The compiler page uses the Rust crate's `wasm-bindgen` entry points through a
`wasm-pack` web bundle. It is useful for quick syntax checks, parser inspection,
type-checking, and generated WAT inspection without building the local CLI.

## Build The Bundle

From the repository root:

```bash
wasm-pack build --target web --out-dir web/pkg
```

The Pages workflow runs the same command before assembling `site/dist`.

## Share A Source Example

The compiler reads source code from the `code` query parameter:

```text
compiler/?code=<url-encoded-restrict-source>
```

The mdBook theme uses that route for "Try in Playground" buttons on complete
`restrict` code blocks that contain `fun main`.

## What It Shows

The browser compiler can show:

- generated WebAssembly text
- lexer output
- parsed AST debug output
- type-checking and code-generation diagnostics

It uses the same parser, type checker, and code generator as the Rust crate.
That makes it a good smoke test for examples in docs and blog posts.

## Module Imports

The web entry point supports a module-source map API internally. The default UI
compiles single-file examples unless module sources are supplied by the page.
For documentation examples, prefer self-contained programs:

```restrict
fun increment: (value: Int32) -> Int32 = {
    value + 1
}

fun main: () -> Int32 = {
    41 |> increment
}
```

If an example depends on source imports, explain the required module sources
instead of presenting it as a one-click playground example.

## Current Limits

The online compiler is not a full IDE. It does not provide package resolution,
Warder project builds, Wasmtime execution, or host stdin/file/network APIs. It
is a browser-hosted compiler surface for the current source file and the
explicit module sources supplied to the web API.
