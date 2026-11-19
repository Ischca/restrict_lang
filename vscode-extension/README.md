# Restrict Language for Visual Studio Code

This extension provides language support for Restrict Language (`.rl`) files. It uses the compiler's native `--lsp` mode and falls back to direct compiler commands when the language server is disabled or unavailable.

## Features

- Live lexer, parser, type, affine-use, import, and release-surface diagnostics
- Cursor-aware completion limited to lexical bindings, imported public APIs, qualified enum constructors, record fields, and receiver-compatible OSV verbs
- Hover information, cross-file go to definition, workspace references, and document symbols
- Reference CodeLens links on definitions and inferred type, return, lambda-parameter, and call-parameter inlay hints
- Semantic highlighting with UTF-16-correct positions plus a TextMate fallback grammar
- Current Restrict syntax snippets, including OSV calls, forms, adoptions, and dotted imports
- Commands to compile the current document to WAT, type check it, inspect its AST, and restart the language server
- Bracket matching, comment toggling, indentation, folding, file icons, and optional dark/light themes

The server intentionally does not advertise rename, formatting, workspace symbols, code actions, signature help, or semantic-token range requests until those operations have real implementations.
Cross-file navigation follows saved source imports and their public export lists. Unsaved edits in open imported files take precedence over disk content.

## Requirements

The extension does not bundle a native compiler. Install or build `restrict_lang`, then either put it on `PATH` or configure its path in VS Code.

```bash
mise exec -- cargo build --bin restrict_lang
./target/debug/restrict_lang --version
```

When the extension is launched from this repository, it automatically finds `target/debug/restrict_lang` or `target/release/restrict_lang`.

## Build and install a VSIX

Node.js 22.13 or later and pnpm 11.2.1 are required. Run `mise install pnpm`
from the repository root to install the pinned package manager.

```bash
cd vscode-extension
pnpm install --frozen-lockfile
pnpm compile
pnpm test:lsp
pnpm package
code --install-extension restrict-language-0.1.0.vsix
```

For extension development, open `vscode-extension` in VS Code and press F5. The included launch configuration compiles the TypeScript and opens an Extension Development Host.

## Configuration

```json
{
    "restrict.compilerPath": "/absolute/path/to/restrict_lang",
    "restrict.languageServerPath": "",
    "restrict.enableLSP": true,
    "restrict.enableTypeChecking": true
}
```

- `restrict.compilerPath`: Compiler used by CLI fallbacks and, when no dedicated server path is set, by the LSP client.
- `restrict.languageServerPath`: Optional compiler executable used only for `--lsp` mode.
- `restrict.enableLSP`: Enables live language features. Run **Restrict: Restart Language Server** after changing paths or this setting.
- `restrict.enableTypeChecking`: Runs a CLI check on save only when the LSP is disabled or unavailable. The LSP publishes live diagnostics independently.

If neither path is configured, the extension checks for a repository development build and then resolves `restrict_lang` from `PATH`.

## Commands

| Command | Default shortcut | Result |
| --- | --- | --- |
| Restrict: Compile Restrict File | `Ctrl+Shift+B` | Writes a sibling `.wat` file |
| Restrict: Type Check Restrict File | `Ctrl+Shift+T` | Publishes diagnostics without code generation |
| Restrict: Show AST | — | Opens the compiler AST in a temporary editor |
| Restrict: Go to Definition | `F12` | Uses the LSP definition provider |
| Restrict: Find All References | `Shift+F12` | Uses the LSP reference provider |
| Restrict: Restart Language Server | — | Applies updated executable settings |

## Current syntax examples

```restrict
import release.policy.{score}

pub enum ParseError {
    Empty
    Message(String)
}

fun double: (value: Int32) -> Int32 = {
    value * 2
}

fun main: () -> () = {
    val answer = 21 |> double
    answer |> println

    val error = "invalid" |> ParseError::Message
    val maybe: Option<Int32> = () Option::None
}
```

Restrict uses `val`, OSV call order, qualified value constructors such as `Option::Some`, and `condition then { ... } else { ... }`. The language specification in the repository root is authoritative.

## Troubleshooting

- Open **Output → Restrict Language Server** to see the resolved command and protocol failures.
- Run `restrict_lang --lsp` manually to verify the configured executable accepts LSP stdio mode.
- Imported modules require a saved `file:` document so their paths can be resolved. Unsaved documents without imports still receive LSP features.
- Syntax highlighting remains available if the native server cannot start; compile, type-check, and AST commands use the CLI fallback.

## License

MIT. See [LICENSE](LICENSE).
