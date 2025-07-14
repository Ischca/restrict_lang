#!/bin/bash
# Restrict Language & Warder Installation Script

set -e

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Configuration
REPO_URL="https://github.com/restrict-lang/restrict_lang"
INSTALL_PREFIX="${INSTALL_PREFIX:-$HOME/.restrict-lang}"
BIN_DIR="$INSTALL_PREFIX/bin"

echo -e "${BLUE}================================================${NC}"
echo -e "${BLUE}    Restrict Language & Warder Installer${NC}"
echo -e "${BLUE}================================================${NC}"
echo ""

# Detect OS
OS="Unknown"
if [[ "$OSTYPE" == "linux-gnu"* ]]; then
    OS="Linux"
elif [[ "$OSTYPE" == "darwin"* ]]; then
    OS="macOS"
elif [[ "$OSTYPE" == "msys" || "$OSTYPE" == "cygwin" ]]; then
    OS="Windows"
fi

echo -e "${GREEN}Detected OS:${NC} $OS"

# Check dependencies
echo -e "\n${YELLOW}Checking dependencies...${NC}"

check_command() {
    if ! command -v "$1" &> /dev/null; then
        echo -e "${RED}✗${NC} $1 not found"
        return 1
    else
        echo -e "${GREEN}✓${NC} $1"
        return 0
    fi
}

MISSING_DEPS=0
check_command "curl" || MISSING_DEPS=1
check_command "git" || MISSING_DEPS=1

if [ $MISSING_DEPS -eq 1 ]; then
    echo -e "\n${RED}Missing dependencies. Please install them first.${NC}"
    exit 1
fi

# Installation method selection
echo -e "\n${YELLOW}Select installation method:${NC}"
echo "1) Download pre-built binaries (recommended)"
echo "2) Build from source"
echo -n "Choice [1]: "
read -r INSTALL_METHOD
INSTALL_METHOD=${INSTALL_METHOD:-1}

if [ "$INSTALL_METHOD" == "1" ]; then
    # Download pre-built binaries
    echo -e "\n${YELLOW}Downloading Restrict Language & Warder...${NC}"
    
    # Determine platform
    PLATFORM=""
    case "$OS" in
        "Linux")
            PLATFORM="linux-x86_64"
            ;;
        "macOS")
            if [[ $(uname -m) == "arm64" ]]; then
                PLATFORM="darwin-aarch64"
            else
                PLATFORM="darwin-x86_64"
            fi
            ;;
        "Windows")
            PLATFORM="windows-x86_64"
            ;;
        *)
            echo -e "${RED}Unsupported platform${NC}"
            exit 1
            ;;
    esac
    
    # Create installation directory
    mkdir -p "$BIN_DIR"
    
    # Download latest release
    LATEST_RELEASE=$(curl -s https://api.github.com/repos/restrict-lang/restrict_lang/releases/latest | grep '"tag_name":' | sed -E 's/.*"([^"]+)".*/\1/')
    
    if [ -z "$LATEST_RELEASE" ]; then
        echo -e "${YELLOW}No pre-built releases found. Falling back to source build...${NC}"
        INSTALL_METHOD="2"
    else
        DOWNLOAD_URL="https://github.com/restrict-lang/restrict_lang/releases/download/${LATEST_RELEASE}/restrict-lang-${LATEST_RELEASE}-${PLATFORM}.tar.gz"
        
        echo -e "Downloading from: $DOWNLOAD_URL"
        curl -L "$DOWNLOAD_URL" | tar -xz -C "$BIN_DIR"
        
        echo -e "${GREEN}✓ Downloaded successfully${NC}"
    fi
fi

if [ "$INSTALL_METHOD" == "2" ]; then
    # Build from source
    echo -e "\n${YELLOW}Building from source...${NC}"
    
    # Check Rust
    if ! check_command "cargo"; then
        echo -e "${YELLOW}Rust not found. Installing via rustup...${NC}"
        curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
        source "$HOME/.cargo/env"
    fi
    
    # Clone repository
    TEMP_DIR=$(mktemp -d)
    cd "$TEMP_DIR"
    
    echo -e "${YELLOW}Cloning repository...${NC}"
    git clone "$REPO_URL" restrict_lang
    cd restrict_lang
    
    # Build
    echo -e "${YELLOW}Building Restrict Language compiler...${NC}"
    cargo build --release
    
    echo -e "${YELLOW}Building Warder package manager...${NC}"
    cd warder
    cargo build --release
    cd ..
    
    # Install binaries
    mkdir -p "$BIN_DIR"
    cp target/release/restrict_lang "$BIN_DIR/"
    cp warder/target/release/warder "$BIN_DIR/"
    
    # Cleanup
    cd /
    rm -rf "$TEMP_DIR"
    
    echo -e "${GREEN}✓ Built and installed successfully${NC}"
fi

# Install VS Code extension (optional)
echo -e "\n${YELLOW}Install VS Code extension?${NC} [y/N]: "
read -r INSTALL_VSCODE
if [[ "$INSTALL_VSCODE" =~ ^[Yy]$ ]]; then
    if command -v code &> /dev/null; then
        echo -e "${YELLOW}Installing VS Code extension...${NC}"
        code --install-extension restrict-lang.restrict-lang
        echo -e "${GREEN}✓ VS Code extension installed${NC}"
    else
        echo -e "${YELLOW}VS Code not found. Skipping extension installation.${NC}"
    fi
fi

# Setup PATH
echo -e "\n${YELLOW}Setting up PATH...${NC}"

add_to_path() {
    local shell_rc="$1"
    local export_line="export PATH=\"$BIN_DIR:\$PATH\""
    
    if [ -f "$shell_rc" ]; then
        if ! grep -q "$BIN_DIR" "$shell_rc"; then
            echo "" >> "$shell_rc"
            echo "# Restrict Language" >> "$shell_rc"
            echo "$export_line" >> "$shell_rc"
            echo -e "${GREEN}✓ Added to $shell_rc${NC}"
        else
            echo -e "${YELLOW}Already in $shell_rc${NC}"
        fi
    fi
}

# Detect shell and update appropriate rc file
if [ -n "$BASH_VERSION" ]; then
    add_to_path "$HOME/.bashrc"
    [ -f "$HOME/.bash_profile" ] && add_to_path "$HOME/.bash_profile"
elif [ -n "$ZSH_VERSION" ]; then
    add_to_path "$HOME/.zshrc"
elif [ -n "$FISH_VERSION" ]; then
    mkdir -p "$HOME/.config/fish/conf.d"
    echo "set -gx PATH $BIN_DIR \$PATH" > "$HOME/.config/fish/conf.d/restrict-lang.fish"
    echo -e "${GREEN}✓ Added to fish config${NC}"
fi

# Verify installation
echo -e "\n${YELLOW}Verifying installation...${NC}"
export PATH="$BIN_DIR:$PATH"

if "$BIN_DIR/restrict_lang" --version &> /dev/null; then
    echo -e "${GREEN}✓ Restrict Language compiler installed${NC}"
    "$BIN_DIR/restrict_lang" --version
else
    echo -e "${RED}✗ Restrict Language compiler installation failed${NC}"
fi

if "$BIN_DIR/warder" --version &> /dev/null; then
    echo -e "${GREEN}✓ Warder package manager installed${NC}"
    "$BIN_DIR/warder" --version
else
    echo -e "${RED}✗ Warder installation failed${NC}"
fi

# Final instructions
echo -e "\n${GREEN}================================================${NC}"
echo -e "${GREEN}    Installation Complete!${NC}"
echo -e "${GREEN}================================================${NC}"
echo ""
echo -e "${YELLOW}To start using Restrict Language:${NC}"
echo ""
echo "1. Reload your shell or run:"
echo -e "   ${BLUE}export PATH=\"$BIN_DIR:\$PATH\"${NC}"
echo ""
echo "2. Create a new project:"
echo -e "   ${BLUE}warder new my-project${NC}"
echo -e "   ${BLUE}cd my-project${NC}"
echo ""
echo "3. Write your first program in src/main.rl:"
echo -e "   ${BLUE}fn main() {${NC}"
echo -e "   ${BLUE}    \"Hello, World!\" |> println${NC}"
echo -e "   ${BLUE}}${NC}"
echo ""
echo "4. Run your program:"
echo -e "   ${BLUE}warder run${NC}"
echo ""
echo -e "${YELLOW}Documentation:${NC} https://docs.restrict-lang.org"
echo -e "${YELLOW}Getting Started:${NC} https://docs.restrict-lang.org/getting-started"
echo ""