# Restrict Language Extension for Visual Studio Code

This extension provides rich language support for Restrict Language (.rl files) in Visual Studio Code.

## Features

### ✨ Syntax Highlighting
- Full syntax highlighting for Restrict Language constructs
- Support for keywords, operators, types, and comments
- Special highlighting for lambda expressions and pattern matching
- Dark and light theme variants

### 🔧 Language Features
- **Auto-completion** with code snippets
- **Bracket matching** and auto-closing pairs
- **Comment toggling** (Ctrl+/ for line, Shift+Alt+A for block)
- **Code folding** support
- **Indentation rules** for proper formatting

### 🚀 Compiler Integration
- **Compile command** (Ctrl+Shift+B) to generate WebAssembly
- **Type checking** (Ctrl+Shift+T) with error diagnostics
- **AST viewer** to inspect parsed syntax trees
- **Automatic type checking** on file save

### 📝 Code Snippets
Quick insertion of common Restrict Language patterns:
- `fun` - Function definition
- `lambda` - Lambda expression
- `val` - Variable binding
- `record` - Record definition
- `match` - Pattern matching
- `if` - Conditional expression
- And many more!

## Installation

### From VSIX (Recommended)
1. Download the latest `.vsix` file from the releases
2. Open VS Code
3. Go to Extensions (Ctrl+Shift+X)
4. Click the "..." menu and select "Install from VSIX..."
5. Select the downloaded `.vsix` file

### From Source
1. Clone the repository
2. Navigate to the `vscode-extension` directory
3. Run `npm install` to install dependencies
4. Run `npm run compile` to build the extension
5. Press F5 to launch a new VS Code window with the extension loaded

## Requirements

- **Restrict Language Compiler**: You need the `restrict_lang` compiler installed and available in your PATH
- **Node.js**: Required for extension development (if building from source)
- **TypeScript**: Required for extension development (if building from source)

## Configuration

The extension can be configured through VS Code settings:

```json
{
    "restrict.compilerPath": "restrict_lang",
    "restrict.enableTypeChecking": true,
    "restrict.showWarnings": true
}
```

### Settings

- `restrict.compilerPath`: Path to the Restrict Language compiler (default: "restrict_lang")
- `restrict.enableTypeChecking`: Enable automatic type checking on save (default: true)
- `restrict.showWarnings`: Show compiler warnings in problems panel (default: true)

## Usage

### Creating a New File
1. Create a new file with `.rl` extension
2. Start typing - you'll get syntax highlighting and auto-completion
3. Use snippets for quick code generation

### Compiling Code
1. Open a `.rl` file
2. Press `Ctrl+Shift+B` or use Command Palette → "Restrict: Compile Restrict File"
3. The compiled WebAssembly will be saved as `.wat` file

### Type Checking
1. Open a `.rl` file
2. Press `Ctrl+Shift+T` or use Command Palette → "Restrict: Type Check Restrict File"
3. Errors will appear in the Problems panel and as red underlines

### Viewing AST
1. Open a `.rl` file
2. Use Command Palette → "Restrict: Show AST"
3. A new document will open showing the parsed syntax tree

## Example Code

```restrict
// Function with lambda and pattern matching
fun process_numbers = numbers:List<Int> -> Int {
    numbers
        |> filter(|x| x > 0)        // Keep positive numbers
        |> map(|x| x * x)           // Square each number
        |> fold(0, |acc, x| acc + x) // Sum them up
}

// Record with methods
record User {
    name: String,
    age: Int,
}

impl User {
    fun is_adult = self:User -> Boolean {
        self.age >= 18
    }
}

fun main = {
    val numbers = [1, -2, 3, -4, 5];
    val result = process_numbers(numbers);
    
    val user = User { name: "Alice", age: 25 };
    val adult = user.is_adult();
    
    result
}
```

## Color Themes

The extension includes two custom color themes optimized for Restrict Language:

- **Restrict Dark**: Dark theme with syntax-aware colors
- **Restrict Light**: Light theme variant

To use these themes:
1. Go to File → Preferences → Color Theme
2. Select "Restrict Dark" or "Restrict Light"

## Keyboard Shortcuts

| Shortcut | Command | Description |
|----------|---------|-------------|
| `Ctrl+Shift+B` | `restrict.compile` | Compile current file |
| `Ctrl+Shift+T` | `restrict.typeCheck` | Type check current file |
| `Ctrl+/` | `editor.action.commentLine` | Toggle line comment |
| `Shift+Alt+A` | `editor.action.blockComment` | Toggle block comment |

## Syntax Highlighting

The extension provides comprehensive syntax highlighting for:

- **Keywords**: `fun`, `val`, `record`, `match`, etc.
- **Types**: `Int32`, `String`, `Boolean`, `Option`, `List`
- **Operators**: `|>`, `=>`, `==`, `!=`, arithmetic operators
- **Lambda expressions**: Special highlighting for `|param|`
- **Comments**: Both single-line `//` and multi-line `/* */`
- **Literals**: Numbers, strings, booleans
- **Pattern matching**: Wildcards and destructuring

## Error Diagnostics

The extension integrates with the Restrict Language compiler to provide:

- **Real-time error checking** on file save
- **Inline error messages** with red underlines
- **Problems panel integration** showing all errors and warnings
- **Type mismatch detection** with helpful error messages
- **Affine type violation** warnings

## Contributing

We welcome contributions! To contribute to the VS Code extension:

1. Fork the repository
2. Create a feature branch
3. Make your changes in the `vscode-extension` directory
4. Test the extension by pressing F5 in VS Code
5. Submit a pull request

### Development Setup

```bash
cd vscode-extension
npm install
npm run compile
```

Press F5 in VS Code to launch the Extension Development Host.

## Troubleshooting

### Compiler Not Found
If you get "Compiler not found" errors:
1. Make sure the Restrict Language compiler is installed
2. Check that `restrict_lang` is in your PATH
3. Or set the full path in settings: `"restrict.compilerPath": "/path/to/restrict_lang"`

### Syntax Highlighting Not Working
1. Make sure the file has `.rl` extension
2. Check that the language mode is set to "Restrict Language"
3. Reload the VS Code window (Ctrl+Shift+P → "Developer: Reload Window")

### Extension Not Loading
1. Check VS Code's output panel for error messages
2. Make sure you have a compatible VS Code version (1.74.0+)
3. Try disabling other extensions to check for conflicts

## Release Notes

### 0.1.0
- Initial release
- Basic syntax highlighting
- Compiler integration
- Code snippets
- Error diagnostics
- Custom themes

## License

This extension is licensed under the MIT License - see the LICENSE file for details.

---

**Enjoy coding with Restrict Language!** 🚀