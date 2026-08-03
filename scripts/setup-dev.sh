#!/bin/bash
# Development Environment Setup Script for Restrict Language

set -e

echo "🔧 Setting up Restrict Language development environment..."

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Check if running on macOS, Linux, or Windows (Git Bash/WSL)
if [[ "$OSTYPE" == "darwin"* ]]; then
    OS="macos"
elif [[ "$OSTYPE" == "linux-gnu"* ]]; then
    OS="linux"
elif [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" ]]; then
    OS="windows"
else
    echo -e "${RED}❌ Unsupported operating system: $OSTYPE${NC}"
    exit 1
fi

echo -e "${BLUE}🖥️  Detected OS: $OS${NC}"

# Function to check if command exists
command_exists() {
    command -v "$1" >/dev/null 2>&1
}

# Function to install tool if not present
install_if_missing() {
    local tool=$1
    local install_cmd=$2
    
    if command_exists "$tool"; then
        echo -e "${GREEN}✅ $tool is already installed${NC}"
    else
        echo -e "${YELLOW}📦 Installing $tool...${NC}"
        eval "$install_cmd"
        if command_exists "$tool"; then
            echo -e "${GREEN}✅ $tool installed successfully${NC}"
        else
            echo -e "${RED}❌ Failed to install $tool${NC}"
            exit 1
        fi
    fi
}

# Check and install Rust
if command_exists "rustc"; then
    echo -e "${GREEN}✅ Rust is already installed${NC}"
    rustc --version
else
    echo -e "${YELLOW}📦 Installing Rust...${NC}"
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    echo -e "${GREEN}✅ Rust installed successfully${NC}"
fi

# Update Rust to latest stable
echo -e "${YELLOW}🔄 Updating Rust to latest stable...${NC}"
rustup update stable
rustup default stable

# Install required Rust components
echo -e "${YELLOW}🔧 Installing Rust components...${NC}"
rustup component add rustfmt clippy

# Install cargo tools
echo -e "${YELLOW}📦 Installing cargo tools...${NC}"
cargo install --force cargo-watch cargo-audit cargo-tarpaulin just

# Check for Node.js (needed for VS Code extension)
if [[ "$OS" == "macos" ]]; then
    install_if_missing "node" "brew install node"
elif [[ "$OS" == "linux" ]]; then
    install_if_missing "node" "curl -fsSL https://deb.nodesource.com/setup_lts.x | sudo -E bash - && sudo apt-get install -y nodejs"
elif [[ "$OS" == "windows" ]]; then
    if ! command_exists "node"; then
        echo -e "${YELLOW}⚠️  Please install Node.js manually from https://nodejs.org/${NC}"
    fi
fi

# Install wasmtime for running WASM
echo -e "${YELLOW}📦 Installing wasmtime...${NC}"
if [[ "$OS" == "macos" ]]; then
    install_if_missing "wasmtime" "brew install wasmtime"
elif [[ "$OS" == "linux" ]]; then
    if ! command_exists "wasmtime"; then
        curl https://wasmtime.dev/install.sh -sSf | bash
        export PATH="$HOME/.wasmtime/bin:$PATH"
    fi
elif [[ "$OS" == "windows" ]]; then
    if ! command_exists "wasmtime"; then
        echo -e "${YELLOW}⚠️  Please install wasmtime manually from https://wasmtime.dev/${NC}"
    fi
fi

# Install wasm-pack for WASM builds
echo -e "${YELLOW}📦 Installing wasm-pack...${NC}"
if ! command_exists "wasm-pack"; then
    curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh
fi

# Build the project
echo -e "${YELLOW}🔨 Building Restrict Language compiler...${NC}"
cargo build --release

echo -e "${YELLOW}🔨 Building Warder package manager...${NC}"
cd warder
cargo build --release
cd ..

# Install VS Code extension dependencies
if [[ -d "vscode-extension" ]]; then
    echo -e "${YELLOW}📦 Installing VS Code extension dependencies...${NC}"
    cd vscode-extension
    npm install
    cd ..
fi

# Set up git hooks
echo -e "${YELLOW}🪝 Setting up git hooks...${NC}"
mkdir -p .git/hooks

cat > .git/hooks/pre-commit << 'EOF'
#!/bin/bash
# Pre-commit hook for Restrict Language

echo "Running pre-commit checks..."

# Format code
echo "Formatting code..."
cargo fmt
cd warder && cargo fmt && cd ..

# Lint code
echo "Linting code..."
cargo clippy -- -D warnings
cd warder && cargo clippy -- -D warnings && cd ..

# Run tests
echo "Running tests..."
cargo test
cd warder && cargo test && cd ..

echo "Pre-commit checks passed!"
EOF

chmod +x .git/hooks/pre-commit

# Create development configuration
echo -e "${YELLOW}⚙️  Creating development configuration...${NC}"
cat > .env.local << 'EOF'
# Development environment configuration
RUST_LOG=debug
RESTRICT_LANG_STD_PATH=./std
warder_CACHE_DIR=.warder/cache
WARDER_REGISTRY_URL=file://./local-registry
EOF

# Print setup summary
echo -e "\n${GREEN}🎉 Development environment setup complete!${NC}\n"

echo -e "${BLUE}📋 Summary of installed tools:${NC}"
echo -e "  ✅ Rust $(rustc --version | cut -d' ' -f2)"
echo -e "  ✅ Cargo tools: watch, audit, tarpaulin, just"
if command_exists "node"; then
    echo -e "  ✅ Node.js $(node --version)"
fi
if command_exists "wasmtime"; then
    echo -e "  ✅ Wasmtime $(wasmtime --version | head -n1)"
fi
if command_exists "wasm-pack"; then
    echo -e "  ✅ wasm-pack $(wasm-pack --version)"
fi

echo -e "\n${BLUE}🚀 Quick start commands:${NC}"
echo -e "  Build:         ${YELLOW}mise run build${NC}"
echo -e "  Test:          ${YELLOW}mise run test${NC}"
echo -e "  Watch:         ${YELLOW}mise run watch${NC}"
echo -e "  Clean:         ${YELLOW}mise run clean${NC}"
echo -e "  Full CI:       ${YELLOW}mise run ci${NC}"
echo -e "  Warder build:  ${YELLOW}mise run build-warder${NC}"

echo -e "\n${BLUE}📚 Documentation:${NC}"
echo -e "  Main README:   ${YELLOW}README.md${NC}"
echo -e "  Tutorial:      ${YELLOW}TUTORIAL.md${NC}"
echo -e "  Reference:     ${YELLOW}REFERENCE.md${NC}"
echo -e "  API docs:      ${YELLOW}API.md${NC}"

echo -e "\n${GREEN}🎯 Ready to start developing!${NC}"
