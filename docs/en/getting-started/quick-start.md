# Quick Start Guide

Get up and running with Restrict Language in minutes!

## Installation

### macOS/Linux (Homebrew)

```bash
# Add Restrict Language tap
brew tap restrict-lang/tap

# Install Warder and the compiler
brew install warder
```

### macOS/Linux/Windows (Installer)

```bash
# Using curl
curl -sSf https://install.restrict-lang.org | sh

# Using wget
wget -qO- https://install.restrict-lang.org | sh
```

### From Source (Cargo)

```bash
# Install from crates.io
cargo install warder
cargo install restrict_lang

# Or install from git
cargo install --git https://github.com/restrict-lang/restrict_lang warder
```

## Verify Installation

```bash
# Check compiler version
restrict_lang --version

# Check Warder version
warder --version
```

## Create Your First Project

### 1. Create a New Project

```bash
warder new hello-world
cd hello-world
```

This creates:
```
hello-world/
├── package.rl.toml      # Project configuration
├── src/
│   └── main.rl         # Entry point
├── tests/
│   └── main_test.rl    # Example test
└── .gitignore
```

### 2. Explore the Project Structure

`package.rl.toml`:
```toml
[package]
name = "hello-world"
version = "0.1.0"
authors = ["Your Name <you@example.com>"]

[dependencies]
# Add dependencies here
```

`src/main.rl`:
{{#include ../../includes/hello-world-main.md}}

### 3. Run Your Project

```bash
# Run in development mode
warder run

# Build for release
warder build --release

# Run tests
warder test
```

## Add Dependencies

```bash
# Add a dependency from WardHub
warder add http

# Add a specific version
warder add json@1.0.0

# Add from git
warder add --git https://github.com/user/package
```

## Common Development Tasks

### Watch Mode

```bash
# Auto-rebuild on file changes
warder watch
```

### Format Code

```bash
# Format all Restrict files
warder fmt
```

### Lint Code

```bash
# Run linter
warder lint
```

### Generate Documentation

```bash
# Generate and open docs
warder doc --open
```

## IDE Setup

### VS Code

```bash
# Install the extension
code --install-extension restrict-lang.restrict-lang
```

Features:
- Syntax highlighting
- Auto-completion
- Error checking
- Go to definition
- Format on save

### Other Editors

- **Vim/Neovim**: Install `restrict-lang/vim-restrict`
- **Emacs**: Install `restrict-lang-mode`
- **IntelliJ**: Install from Plugin Marketplace

## Example: Web Server

Create a simple web server:

```restrict
use std::net::{TcpListener, TcpStream};
use std::io::{read, write};

fn handleClient(stream: TcpStream) {
    let request = stream |> read
    
    let response = "HTTP/1.1 200 OK\r\n\r\nHello from Restrict!"
    response |> stream.write
}

fn main() {
    let listener = TcpListener::bind("127.0.0.1:8080")?
    "Server running on http://localhost:8080" |> println
    
    for stream in listener.incoming() {
        stream? |> handleClient
    }
}
```

Run it:
```bash
warder run
# Visit http://localhost:8080
```

## Deployment

### Build for WebAssembly

```bash
# Build WASM module
warder build --target wasm32-wasi

# Output: target/wasm32-wasi/release/hello-world.wasm
```

### Create a Cage (Package)

```bash
# Package your project
warder package

# Output: target/release/hello-world-0.1.0.rgc
```

### Publish to WardHub

```bash
# Login to WardHub
warder login

# Publish your package
warder publish
```

## Troubleshooting

### Command Not Found

If `warder` or `restrict_lang` commands are not found:

```bash
# Add to PATH manually
export PATH="$HOME/.restrict-lang/bin:$PATH"

# Make permanent (bash)
echo 'export PATH="$HOME/.restrict-lang/bin:$PATH"' >> ~/.bashrc

# Make permanent (zsh)
echo 'export PATH="$HOME/.restrict-lang/bin:$PATH"' >> ~/.zshrc
```

### Build Errors

```bash
# Clean build artifacts
warder clean

# Update dependencies
warder update

# Check project health
warder doctor
```

## Next Steps

- Read the [Language Guide](../guide/README.md)
- Explore [Standard Library](../reference/stdlib.md)
- Join our [Community](https://discord.gg/restrict-lang)

Happy coding with Restrict Language! 🦀