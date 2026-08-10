# Language Server Protocol

The compiler can run a Language Server Protocol server over stdio:

```bash
restrict_lang --lsp
```

Editor integrations should start that command as a stdio language server from
the workspace where Restrict files live.

## Current Role

The LSP is a development aid for source editing. It should be treated as a thin
frontend over the compiler's lexer, parser, and type checker rather than a
separate language implementation. The language specification and compiler tests
remain the source of truth.

The current server implements:

- live lexer, parser, type, affine-use, import, and release-surface diagnostics
- semantic tokens, hover, and document symbols
- completion restricted to bindings and APIs available at the cursor, including
  import visibility, qualified enum variants, record fields, and receiver types
- cross-file definitions and workspace references that preserve imported source
  URIs and ranges
- reference CodeLens links on definitions
- inlay hints for inferred binding, return, and lambda parameter types plus OSV
  call parameter names

Completion and definition requests traverse the active file's import closure.
References and CodeLens search `.rl` files under the Restrict workspace. Open
documents override disk content, so navigation follows unsaved edits when the
imported file is open.

## Local Setup

Build the compiler first:

```bash
mise exec -- cargo build --release
```

Then point the editor at:

```text
target/release/restrict_lang --lsp
```

During active compiler development, a debug build is also fine:

```text
target/debug/restrict_lang --lsp
```

## Troubleshooting

If the editor shows no diagnostics, check:

- the compiler binary path is correct
- the editor starts the server with stdio, not TCP
- the workspace root contains the `.rl` files being edited
- imported modules are saved under the same paths used by source imports
- the same source parses with the CLI

Use the CLI to separate editor setup problems from compiler problems:

```bash
mise exec -- cargo run --bin restrict_lang examples/release_readiness.rl
```
