# Building the Restrict Language VS Code Extension

## Prerequisites

- Node.js 16.x or later
- npm or yarn
- Visual Studio Code 1.74.0 or later
- VSCE (Visual Studio Code Extension Manager)

## Installation

### Install VSCE

```bash
npm install -g vsce
```

### Install Dependencies

```bash
cd vscode-extension
npm install
```

## Development

### Building

```bash
# Compile TypeScript
npm run compile

# Watch for changes during development
npm run watch
```

### Testing

1. Open VS Code in the `vscode-extension` directory
2. Press `F5` to launch a new Extension Development Host window
3. Open a `.rl` file to test the extension
4. Make changes to the source code and reload the extension window

### Debugging

- Use `console.log()` statements in TypeScript code
- Check the VS Code Developer Console (Help → Toggle Developer Tools)
- Check the Output panel for Language Server communication

## Packaging

### Create VSIX Package

```bash
# From the vscode-extension directory
vsce package

# This creates restrict-language-0.1.0.vsix
```

### Install Locally

```bash
# Install the packaged extension
code --install-extension restrict-language-0.1.0.vsix
```

## Publishing (Future)

### Prepare for Publishing

1. Update version in `package.json`
2. Update `CHANGELOG.md`
3. Ensure all examples and documentation are up to date
4. Test thoroughly

### Publish to VS Code Marketplace

```bash
# Login to marketplace (one-time setup)
vsce login your-publisher-name

# Publish the extension
vsce publish
```

## Language Server Integration

The extension integrates with the compiler's Language Server Protocol (LSP) implementation. The LSP server runs as the main compiler in stdio mode:

1. `restrict_lang` executable in PATH, or
2. A compiler executable configured via the `restrict.languageServerPath` setting

The extension starts the configured command with the `--lsp` flag.

### v0.0.1 LSP Features

The Language Server currently implements and advertises:

- **textDocument/hover**: Type information on hover
- **textDocument/completion**: Auto-completion
- **textDocument/definition**: Go to definition
- **textDocument/references**: Find all references
- **textDocument/documentSymbol**: Document outline
- **textDocument/semanticTokens/full**: Semantic highlighting
- **textDocument/publishDiagnostics**: Error and warning diagnostics

Rename, document formatting, workspace symbols, code actions, signature help,
code lens, and semantic-token range requests are not advertised in v0.0.1.

### LSP Server Command Line

The LSP server should accept:

```bash
restrict_lang --lsp                    # Start LSP stdio server mode
```

## File Structure

```
vscode-extension/
├── src/
│   ├── extension.ts          # Main extension entry point
│   └── languageServer.ts     # LSP client implementation
├── syntaxes/
│   └── restrict.tmLanguage.json  # TextMate grammar
├── themes/
│   ├── restrict-dark.json    # Dark theme
│   └── restrict-light.json   # Light theme
├── snippets/
│   └── restrict.json         # Code snippets
├── examples/
│   └── hello.rl             # Example file
├── package.json             # Extension manifest
├── tsconfig.json            # TypeScript configuration
├── language-configuration.json  # Language configuration
├── README.md                # Extension documentation
├── CHANGELOG.md             # Version history
└── .vscodeignore           # Files to exclude from package
```

## Configuration

The extension can be configured through VS Code settings:

```json
{
    "restrict.compilerPath": "restrict_lang",
    "restrict.languageServerPath": "restrict_lang",
    "restrict.enableTypeChecking": true,
    "restrict.enableLSP": true,
    "restrict.showWarnings": true
}
```

## Troubleshooting

### Extension Not Loading

1. Check VS Code version compatibility
2. Check TypeScript compilation errors
3. Look at VS Code's Developer Console for errors

### Language Server Not Starting

1. Verify the compiler executable exists and is executable
2. Check the `restrict.languageServerPath` setting
3. Look at the "Restrict Language Server" output channel

### Syntax Highlighting Issues

1. Verify file has `.rl` extension
2. Check that language mode is set to "Restrict Language"
3. Reload VS Code window

### IntelliSense Not Working

1. Ensure LSP server is running (check output channel)
2. Verify document is saved (LSP requires saved documents)
3. Check for syntax errors that might prevent analysis

## Testing Checklist

Before releasing:

- [ ] Syntax highlighting works for all language constructs
- [ ] Code snippets insert correctly
- [ ] Compiler integration (compile, type check, show AST) works
- [ ] v0.0.1 LSP features work (hover, completion, definitions, references, document symbols, diagnostics)
- [ ] Themes display correctly
- [ ] Auto-completion triggers appropriately
- [ ] Error diagnostics show up correctly
- [ ] Extension loads without errors
- [ ] All commands are accessible via Command Palette
- [ ] Keyboard shortcuts work as expected

## Performance Considerations

- LSP communication should be efficient (avoid frequent requests)
- Syntax highlighting should not cause lag on large files
- Auto-completion should be responsive
- Type checking should not block the UI

## Future Enhancements

- Inlay hints for type annotations
- Rename support
- Document formatting
- Code actions and quick fixes
- Signature help
- Workspace symbol search
- Code lens for references count
- Integrated debugging support
- Project management features
- Build task integration
