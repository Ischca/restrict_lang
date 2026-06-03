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

Typical editor features can include:

- document diagnostics
- syntax-aware tokenization
- type-checking feedback
- project-file awareness when the editor provides workspace roots

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
- the same source parses with the CLI

Use the CLI to separate editor setup problems from compiler problems:

```bash
mise exec -- cargo run --bin restrict_lang examples/release_readiness.rl
```
