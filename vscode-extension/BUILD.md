# Building the Restrict Language VS Code extension

## Prerequisites

- Node.js 22 or later
- npm
- Visual Studio Code 1.74 or later
- A native Restrict compiler built from this repository

The VSCE packaging command is a locked development dependency; no global `vsce` installation is needed.

## Build

From the repository root:

```bash
mise exec -- cargo build --bin restrict_lang
cd vscode-extension
npm ci
npm run compile
```

Open `vscode-extension` as the VS Code workspace and press F5 to start the included Extension Development Host configuration.

## Automated checks

```bash
# TypeScript client
npm run compile

# Real JSON-RPC exchange with target/debug/restrict_lang --lsp
npm run test:lsp

# Rust LSP unit and protocol-service tests (from the repository root)
mise exec -- cargo test lsp --lib
```

`scripts/smoke-lsp.mjs` initializes the real server process against a temporary two-file project and checks imported-API completion, cross-file definitions and references, definition CodeLens, inlay hints, semantic tokens, AST execution, and clean shutdown. Pass another compiler path as its first argument when needed:

```bash
node scripts/smoke-lsp.mjs /absolute/path/to/restrict_lang
```

## Package

```bash
npm run package
code --install-extension restrict-language-0.0.1.vsix
```

The VSIX includes the bundled extension client, TextMate grammar, snippets, themes, and icons. `esbuild` bundles the production JavaScript dependencies into `out/extension.js`; the package deliberately does not include `node_modules` or a platform-specific Restrict compiler binary.

## Server resolution

The extension starts the selected executable with `--lsp`. Resolution order is:

1. `restrict.languageServerPath`
2. `restrict.compilerPath`
3. `target/debug/restrict_lang` or `target/release/restrict_lang` when running from this repository
4. `restrict_lang` from `PATH`

CLI fallback commands prefer `restrict.compilerPath`, then `restrict.languageServerPath`, the development build, and finally `PATH`.

The extension runs in the workspace extension host so remote workspaces use the compiler installed on the remote side. It is disabled in untrusted workspaces because it starts a configured native executable.

## Implemented LSP surface

- `textDocument/hover`
- `textDocument/completion`
- `textDocument/definition`
- `textDocument/references`
- `textDocument/codeLens`
- `textDocument/inlayHint`
- `textDocument/documentSymbol`
- `textDocument/semanticTokens/full`
- `textDocument/publishDiagnostics`
- `workspace/executeCommand` for compile, type check, and AST inspection

Unsupported operations are not advertised. Keep the server capabilities, extension README, and protocol smoke test aligned when adding another operation.
Definitions keep their original module URI and range in a source index separate from compiler import splicing. Reference and CodeLens requests search the Restrict workspace, while latency-sensitive completion and definition requests load only the current import closure.

## Release checklist

```bash
mise exec -- cargo test lsp --lib
cd vscode-extension
npm ci
npm run compile
npm run test:lsp
npm run package
```

Then install the generated VSIX in a clean Extension Development Host and verify diagnostics, scoped completion, cross-file definitions and references, reference CodeLens, inlay hints, hover, outline symbols, semantic highlighting, compile output, type checking, and AST inspection on representative `.rl` files.
