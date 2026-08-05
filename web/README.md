# Restrict Language Web Compiler

A web-based compiler for the Restrict Language that runs entirely in the browser using WebAssembly.

## Features

- **Browser Compiler Flow**: Lexing, parsing, type checking, and WASM-oriented output through the bundled WebAssembly module
- **Interactive Interface**: Compile, lex, and parse actions from a browser editor
- **Restrict Syntax Highlighting**: The source editor highlights keywords, types, literals, comments, strings, numbers, and OSV operators
- **Step-by-Step Analysis**: View tokens, AST, and compilation errors separately
- **Example Programs**: Built-in examples using the current public syntax, including Forms and Display
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
2. **Run**: Click "Run" to compile and execute a zero-argument `main` function locally
3. **Compile**: Click "Compile" when you only want to inspect the generated WebAssembly text
4. **Analyze**: Use "Tokenize" or "Parse" for step-by-step analysis
5. **View Results**: Switch between tabs to see program output, WebAssembly, tokens, AST, or errors
6. **Try Examples**: Choose an example program to load it

## Architecture

The web compiler consists of:

- **Rust Backend**: The core compiler and WAT assembler compiled to WebAssembly
- **JavaScript Frontend**: Web interface that calls the compiler, instantiates generated WebAssembly, captures WASI stdout/stderr, and keeps the source editor highlight layer in sync
- **HTML/CSS**: User interface and styling

## Supported Syntax Surface

The web demo examples use the current public syntax surface. Some planned or
experimental language features may be reserved by the parser or documented in
the language specification, but they are not guaranteed to compile through the
browser demo.

- Function declarations with `fun name: (...) -> Type =`
- Basic types such as `Int32`, `Float64`, and `Boolean`
- Variable declarations with `val` and `mut val`
- OSV calls such as `value |> function` and `(left, right) add`
- Record declarations and literals with colon-delimited fields
- Method-only `form` declarations, concrete record `takes` adoptions, and `of` bounds
- Display-polymorphic `display`, `print`, and `println`
- `then`/`else` expressions, pattern matching, type checking, and WASM-oriented output where implemented

The initial form slice is deliberately static: forms and adoptions are
non-generic, form methods have complete signatures, and each adoption provides
every method body. Associated types, default methods, conditional or generic
adoptions, enum adoptions, and dynamic dispatch are not supported.

## Browser Runtime

`Run` assembles the generated WAT inside the compiler bundle, instantiates the
program with a small browser-side WASI bridge, invokes the generated `_start`
entry point, and displays stdout and stderr in the Output tab. A runnable
program needs a zero-argument `main`; use Display-polymorphic `print` or
`println` to produce visible output. The String-only `eprint` and `eprintln`
write to stderr, while `print_int` and `print_float` remain available for
compatibility. The source and generated program stay in the browser.

## Browser Compatibility

The web compiler requires a modern browser with WebAssembly support:

- Chrome 57+
- Firefox 52+
- Safari 11+
- Edge 16+

## CI/CD and GitHub Pages Deployment

The landing page, playground, blog, and documentation are assembled and deployed to GitHub Pages on every push to `main`.

### Automatic Deployment

The unified workflow (`.github/workflows/deploy-docs.yml`) does the following:

1. Builds WASM using `wasm-pack` for the playground
2. Builds documentation using `mdBook`
3. Deploys to GitHub Pages:
   - Landing page at `/restrict_lang/`
   - Playground at `/restrict_lang/compiler/`
   - Documentation at `/restrict_lang/docs/`
   - Blog at `/restrict_lang/blog/`

### Manual Setup

To enable GitHub Pages for your fork:

1. Go to repository **Settings** > **Pages**
2. Under "Build and deployment", select **GitHub Actions**
3. The site will be deployed automatically on the next push to `main`

### Deployment URLs

After deployment:
- **Landing page**: `https://<username>.github.io/<repository>/`
- **Playground**: `https://<username>.github.io/<repository>/compiler/`
- **Documentation**: `https://<username>.github.io/<repository>/docs/`

## Development

To modify the web compiler:

1. Edit Rust code in `src/web.rs` for WASM bindings
2. Edit JavaScript code in `web/app.js` for the frontend
3. Edit `web/restrict-highlight.js` for shared source highlighting rules
4. Edit HTML/CSS in `web/index.html` for the interface
5. Rebuild with `./build.sh`

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
