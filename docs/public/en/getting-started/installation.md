# Installation

This page describes the source-build path for the current repository. It does
not assume a release channel, package registry, installer, or published version
identity.

## Requirements

- Rust toolchain
- Git
- [mise](https://mise.jdx.dev/) for the repository toolchain
- Wasmtime or Wasmer when using `warder run`

## Build From Source

Clone the repository:

```bash
git clone https://github.com/restrict-lang/restrict_lang.git
cd restrict_lang
```

Build the compiler and Warder from the workspace:

```bash
mise exec -- cargo build --release
```

The workspace build writes binaries under:

```text
target/release/
```

If you build Warder from its package directory separately, also check:

```text
warder/target/release/
```

Add the appropriate directories to your shell `PATH`:

```bash
export PATH="$PWD/target/release:$PWD/warder/target/release:$PATH"
```

Use only the directories that exist for your build layout.

## Verify Installation

```bash
restrict_lang --version
warder --version
warder doctor
```

`warder doctor` reports whether the compiler is visible and whether a WASM
runtime is available.

## WASM Runtime

`warder run` executes the generated `.wasm` through Wasmtime first, then Wasmer
when Wasmtime is not available.

Install one runtime using its upstream instructions, then verify it is on your
`PATH`:

```bash
wasmtime --version
# or
wasmer --version
```

## Project Commands

Create and build a project:

```bash
warder new hello-world
cd hello-world
warder build
warder run
warder test
```

The default build outputs are:

```text
dist/hello-world-0.1.0.wat
dist/hello-world-0.1.0.wasm
dist/hello-world-0.1.0.rgc
```

The output stem is `<package-name>-<package-version>` from
`package.rl.toml`.

## Language Server

The compiler can start the language server over stdio:

```bash
restrict_lang --lsp
```

Configure editors that support LSP to start that command as a stdio language
server.

## Troubleshooting

**Command not found**

Check that the release binary directory from your source build is on `PATH`.
For workspace builds this is usually `target/release`; for separate Warder
builds it may be `warder/target/release`.

**Warder cannot find the compiler**

Set `RESTRICT_LANG_BIN` or add the compiler binary directory to `PATH`:

```bash
export RESTRICT_LANG_BIN="$PWD/target/release/restrict_lang"
```

**Program does not run**

Install Wasmtime or Wasmer, then rerun:

```bash
warder doctor
warder run
```

## Next Steps

- [Quick Start](./quick-start.md)
- [Warder guide](../guide/warder.md)
- [Language syntax](../guide/syntax.md)
