# Installation

This guide will walk you through installing Restrict Language and setting up your development environment.

## System Requirements

Restrict Language supports the following platforms:

- **macOS** (x86_64, ARM64)
- **Linux** (x86_64, ARM64)
- **Windows** (x86_64, via WSL2 recommended)

### Prerequisites

- **Rust** 1.70 or later (for building from source)
- **Git** (for version control)
- **A text editor** (VS Code recommended)

## Installation Methods

### Method 1: Using mise (Recommended)

[mise](https://mise.jdx.dev/) is a polyglot runtime manager that makes it easy to install and manage Restrict Language.

```bash
# Install mise if you haven't already
curl https://mise.run | sh

# Add mise to your shell
echo 'eval "$(~/.local/bin/mise activate bash)"' >> ~/.bashrc
# For zsh users:
# echo 'eval "$(~/.local/bin/mise activate zsh)"' >> ~/.zshrc

# Install Restrict Language
mise use restrict_lang@latest
```

### Method 2: From Source

Clone and build from the official repository:

```bash
# Clone the repository
git clone https://github.com/restrict-lang/restrict_lang.git
cd restrict_lang

# Build with mise
mise run build-release

# Or build with cargo directly
cargo build --release
cd warder && cargo build --release
```

After building, add the binaries to your PATH:

```bash
# Add to ~/.bashrc or ~/.zshrc
export PATH="$PATH:$HOME/restrict_lang/target/release"
```

### Method 3: Pre-built Binaries

Download pre-built binaries from the releases page:

```bash
# macOS (Intel)
curl -L https://github.com/restrict-lang/restrict_lang/releases/latest/download/restrict_lang-darwin-x86_64.tar.gz | tar xz

# macOS (Apple Silicon)
curl -L https://github.com/restrict-lang/restrict_lang/releases/latest/download/restrict_lang-darwin-aarch64.tar.gz | tar xz

# Linux (x86_64)
curl -L https://github.com/restrict-lang/restrict_lang/releases/latest/download/restrict_lang-linux-x86_64.tar.gz | tar xz

# Move binaries to a location in your PATH
sudo mv restrict_lang warder /usr/local/bin/
```

## Verify Installation

Check that both the compiler and package manager are installed correctly:

```bash
# Check Restrict Language compiler
restrict_lang --version
# Expected output: restrict_lang 0.1.0

# Check Warder package manager
warder --version
# Expected output: warder 0.1.0
```

## Install WebAssembly Runtime

Restrict Language compiles to WebAssembly, so you'll need a WASM runtime to execute programs:

### Option 1: Wasmtime (Recommended)

```bash
# Install wasmtime
curl https://wasmtime.dev/install.sh -sSf | bash
```

### Option 2: Wasmer

```bash
# Install wasmer
curl https://get.wasmer.io -sSfL | sh
```

## IDE Setup

### VS Code Extension

For the best development experience, install the official VS Code extension:

1. Open VS Code
2. Go to Extensions (Cmd+Shift+X on macOS, Ctrl+Shift+X on Windows/Linux)
3. Search for "Restrict Language"
4. Click Install

The extension provides:
- Syntax highlighting
- Auto-completion
- Error checking
- Go to definition
- Find references
- Format on save

### Alternative Editors

For other editors, you can use the Language Server Protocol (LSP):

```bash
# Start the language server
restrict_lang lsp
```

Configure your editor to connect to the language server on the default port (7777).

## Development Tools

Install additional development tools:

```bash
# Install all development dependencies
mise run setup

# Or install manually
cargo install cargo-watch cargo-audit cargo-tarpaulin
```

## Configuration

Create a global configuration file:

```bash
mkdir -p ~/.config/restrict_lang
cat > ~/.config/restrict_lang/config.toml << EOF
[compiler]
optimization_level = 2
target = "wasm32-wasi"

[warder]
registry = "https://wardhub.io"
cache_dir = "~/.cache/warder"

[editor]
format_on_save = true
lint_on_save = true
EOF
```

## Troubleshooting

### Common Issues

**Command not found**
- Ensure the binaries are in your PATH
- Restart your terminal or run `source ~/.bashrc`

**Permission denied**
- Make the binaries executable: `chmod +x restrict_lang warder`
- Use `sudo` when moving to system directories

**Build failures**
- Ensure Rust is up to date: `rustup update`
- Check system dependencies are installed

### Getting Help

- Check the [FAQ](../appendix/faq.md)
- Join our [Discord community](https://discord.gg/restrict-lang)
- Report issues on [GitHub](https://github.com/restrict-lang/restrict_lang/issues)

## Next Steps

Now that you have Restrict Language installed, you're ready to:

- [Write your first program](./hello-world.md)
- [Learn about Warder package manager](./warder.md)
- [Explore the language guide](../guide/syntax.md)