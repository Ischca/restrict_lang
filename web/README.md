# Restrict Language Web Compiler

A web-based compiler for the Restrict Language that runs entirely in the browser using WebAssembly.

## Features

- **Full Compilation Pipeline**: Lexing, parsing, type checking, and WASM code generation
- **Interactive Interface**: Real-time compilation with syntax highlighting
- **Step-by-Step Analysis**: View tokens, AST, and compilation errors separately
- **Example Programs**: Built-in examples to get started quickly
- **No Server Required**: Runs entirely in the browser

## Building

### Prerequisites

- [Rust](https://rustup.rs/) (latest stable version)
- [wasm-pack](https://rustwasm.github.io/wasm-pack/installer/)

### Build Instructions

1. Install wasm-pack if you haven't already:
```bash
curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
```

2. Run the build script:
```bash
cd web
chmod +x build.sh
./build.sh
```

3. Serve the web directory:
```bash
# Using Python
python -m http.server 8000

# Using Node.js
npx serve .

# Using any other HTTP server
```

4. Open your browser and navigate to `http://localhost:8000`

## Usage

1. **Write Code**: Enter your Restrict Language code in the text area
2. **Compile**: Click "Compile" to run the full compilation pipeline
3. **Analyze**: Use "Lex Only" or "Parse Only" for step-by-step analysis
4. **View Results**: Switch between tabs to see WASM output, tokens, AST, or errors
5. **Try Examples**: Click on the example programs to load them

## Architecture

The web compiler consists of:

- **Rust Backend**: The core compiler compiled to WebAssembly
- **JavaScript Frontend**: Web interface that calls the WASM module
- **HTML/CSS**: User interface and styling

## Supported Features

All features of the Restrict Language are supported:

- Function definitions with parameters and return types
- Variable declarations (val/mut)
- Pipe operations (|> and |>>)
- Record types with clone/freeze operations
- Context binding with `with` blocks
- Control flow (if/else, while loops)
- Type checking and affine type system
- WASM code generation

## Browser Compatibility

The web compiler requires a modern browser with WebAssembly support:

- Chrome 57+
- Firefox 52+
- Safari 11+
- Edge 16+

## Development

To modify the web compiler:

1. Edit Rust code in `src/web.rs` for WASM bindings
2. Edit JavaScript code in `web/app.js` for the frontend
3. Edit HTML/CSS in `web/index.html` for the interface
4. Rebuild with `./build.sh`

## Troubleshooting

**Build fails with "wasm-pack not found":**
- Install wasm-pack using the installer script above

**Browser shows "Failed to fetch" errors:**
- Make sure you're serving the files over HTTP, not opening them as `file://`
- Use any HTTP server (Python, Node.js, etc.)

**Compilation errors in the browser:**
- Check the browser console for detailed error messages
- Ensure your Restrict Language code follows the correct syntax

## License

This project is licensed under the same terms as the main Restrict Language project.