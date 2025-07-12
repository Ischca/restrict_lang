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

The extension integrates with a Language Server Protocol (LSP) implementation. The LSP server should be built separately and made available as:

1. `restrict_lsp` executable in PATH, or
2. Configured via `restrict.languageServerPath` setting, or
3. The main compiler with `--lsp` flag support

### Expected LSP Features

The Language Server should implement:

- **textDocument/hover**: Type information on hover
- **textDocument/completion**: Auto-completion
- **textDocument/definition**: Go to definition
- **textDocument/references**: Find all references
- **textDocument/rename**: Symbol renaming
- **textDocument/formatting**: Document formatting
- **textDocument/documentSymbol**: Document outline
- **workspace/symbol**: Workspace symbol search
- **textDocument/codeAction**: Code actions (quick fixes)
- **textDocument/signatureHelp**: Function signature help
- **textDocument/publishDiagnostics**: Error and warning diagnostics

### LSP Server Command Line

The LSP server should accept:

```bash
restrict_lang --lsp                    # Start LSP server mode
restrict_lang --lsp --stdio           # Use stdio for communication
restrict_lang --lsp --port 8080       # Use TCP port for communication
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
    "restrict.languageServerPath": "restrict_lsp",
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

1. Verify the LSP server executable exists and is executable
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
- [ ] LSP features work (hover, go to definition, etc.)
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

- Semantic highlighting
- Inlay hints for type annotations
- Code lens for references count
- Integrated debugging support
- Project management features
- Build task integration