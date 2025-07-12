# Change Log

All notable changes to the "restrict-language" extension will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2025-01-XX

### Added
- Initial release of Restrict Language VS Code extension
- Complete syntax highlighting for .rl files
- Language configuration with auto-closing pairs and brackets
- Code snippets for common language constructs
- Compiler integration with build commands
- Type checking with error diagnostics
- AST viewer for debugging
- Custom dark and light color themes
- Auto-completion support
- Comment toggling support
- Code folding capabilities

### Features
- **Syntax Highlighting**: Full support for Restrict Language syntax
  - Keywords: fun, val, record, match, etc.
  - Types: Int32, String, Boolean, Option, List
  - Operators: |>, =>, ==, !=, arithmetic operators
  - Lambda expressions with special highlighting
  - Comments: single-line (//) and multi-line (/* */)
  - String and character literals
  - Pattern matching constructs

- **Compiler Integration**: 
  - Compile command (Ctrl+Shift+B)
  - Type check command (Ctrl+Shift+T)
  - Show AST command
  - Automatic type checking on save
  - Error diagnostics in Problems panel

- **Code Snippets**: Quick insertion for:
  - Function definitions
  - Lambda expressions
  - Variable bindings
  - Record definitions and instances
  - Pattern matching
  - Control flow constructs
  - Arena allocation blocks

- **Themes**: Custom color themes optimized for Restrict Language
  - Restrict Dark: Dark theme with syntax-aware colors
  - Restrict Light: Light theme variant

- **Configuration**: Customizable settings
  - Compiler path configuration
  - Auto type checking toggle
  - Warning display options

### Technical Details
- Built with TypeScript
- Uses TextMate grammar for syntax highlighting
- Integrates with VS Code's language server protocol
- Supports all VS Code language features (folding, commenting, etc.)

### Requirements
- VS Code 1.74.0 or later
- Restrict Language compiler in PATH or configured path

### Known Issues
- None at initial release

### Documentation
- Complete README with usage instructions
- Example files demonstrating language features
- Configuration documentation