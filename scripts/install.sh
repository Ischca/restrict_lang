#!/bin/bash
# Restrict Language Installation Script
# Usage: curl -sSf https://install.restrict-lang.org | sh

set -e

# Configuration
RESTRICT_LANG_VERSION="${RESTRICT_LANG_VERSION:-latest}"
INSTALL_DIR="${INSTALL_DIR:-$HOME/.restrict-lang}"
REPO_URL="https://github.com/restrict-lang/restrict_lang"
BINARY_URL="https://github.com/restrict-lang/restrict_lang/releases/download"

# Colors
if [[ -t 1 ]]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    BLUE='\033[0;34m'
    BOLD='\033[1m'
    NC='\033[0m'
else
    RED=''
    GREEN=''
    YELLOW=''
    BLUE=''
    BOLD=''
    NC=''
fi

# Helper functions
info() {
    echo -e "${BLUE}info:${NC} $1"
}

success() {
    echo -e "${GREEN}success:${NC} $1"
}

warning() {
    echo -e "${YELLOW}warning:${NC} $1"
}

error() {
    echo -e "${RED}error:${NC} $1" >&2
}

# Detect architecture and OS
detect_platform() {
    local os arch

    # Detect OS
    case "$(uname -s)" in
        Linux*)
            os="linux"
            ;;
        Darwin*)
            os="darwin"
            ;;
        MINGW* | MSYS* | CYGWIN*)
            os="windows"
            ;;
        *)
            error "Unsupported operating system: $(uname -s)"
            return 1
            ;;
    esac

    # Detect architecture
    case "$(uname -m)" in
        x86_64 | amd64)
            arch="x86_64"
            ;;
        aarch64 | arm64)
            arch="aarch64"
            ;;
        *)
            error "Unsupported architecture: $(uname -m)"
            return 1
            ;;
    esac

    echo "${os}-${arch}"
}

# Download and extract binary
download_binary() {
    local platform="$1"
    local version="$2"
    local temp_dir
    
    temp_dir=$(mktemp -d)
    cd "$temp_dir"

    # Construct download URL
    if [ "$version" = "latest" ]; then
        # Get latest release version
        info "Fetching latest version..."
        version=$(curl -sSf https://api.github.com/repos/restrict-lang/restrict_lang/releases/latest | grep '"tag_name":' | sed -E 's/.*"v([^"]+)".*/\1/')
        if [ -z "$version" ]; then
            error "Failed to fetch latest version"
            return 1
        fi
        info "Latest version: v$version"
    fi

    local filename="restrict-lang-v${version}-${platform}.tar.gz"
    local url="${BINARY_URL}/v${version}/${filename}"

    info "Downloading Restrict Language v$version for $platform..."
    info "URL: $url"

    if ! curl -sSfL "$url" -o "$filename"; then
        error "Failed to download from $url"
        error "This might mean binaries aren't available for your platform yet."
        error "Try building from source instead."
        return 1
    fi

    info "Extracting..."
    tar -xzf "$filename"

    # Install binaries
    mkdir -p "$INSTALL_DIR/bin"
    cp restrict_lang "$INSTALL_DIR/bin/"
    cp warder "$INSTALL_DIR/bin/"
    chmod +x "$INSTALL_DIR/bin/restrict_lang"
    chmod +x "$INSTALL_DIR/bin/warder"

    # Cleanup
    cd /
    rm -rf "$temp_dir"

    success "Binaries installed to $INSTALL_DIR/bin"
}

# Build from source
build_from_source() {
    info "Building from source..."

    # Check for Rust
    if ! command -v cargo >/dev/null 2>&1; then
        error "Rust is required to build from source"
        info "Install Rust from https://rustup.rs/"
        return 1
    fi

    local temp_dir
    temp_dir=$(mktemp -d)
    cd "$temp_dir"

    # Clone repository
    info "Cloning repository..."
    git clone --depth 1 "$REPO_URL" restrict_lang
    cd restrict_lang

    # Build
    info "Building Restrict Language compiler..."
    cargo build --release

    info "Building Warder package manager..."
    cd warder
    cargo build --release
    cd ..

    # Install
    mkdir -p "$INSTALL_DIR/bin"
    cp target/release/restrict_lang "$INSTALL_DIR/bin/"
    cp warder/target/release/warder "$INSTALL_DIR/bin/"
    chmod +x "$INSTALL_DIR/bin/restrict_lang"
    chmod +x "$INSTALL_DIR/bin/warder"

    # Cleanup
    cd /
    rm -rf "$temp_dir"

    success "Built and installed from source"
}

# Configure shell
configure_shell() {
    local shell_name shell_rc added=0

    case "$SHELL" in
        */bash)
            shell_name="bash"
            shell_rc="$HOME/.bashrc"
            [ -f "$HOME/.bash_profile" ] && shell_rc="$HOME/.bash_profile"
            ;;
        */zsh)
            shell_name="zsh"
            shell_rc="$HOME/.zshrc"
            ;;
        */fish)
            shell_name="fish"
            shell_rc="$HOME/.config/fish/config.fish"
            ;;
        *)
            warning "Unknown shell: $SHELL"
            warning "Please add $INSTALL_DIR/bin to your PATH manually"
            return
            ;;
    esac

    local export_line="export PATH=\"\$HOME/.restrict-lang/bin:\$PATH\""
    if [ "$shell_name" = "fish" ]; then
        export_line="set -gx PATH \$HOME/.restrict-lang/bin \$PATH"
    fi

    if [ -f "$shell_rc" ]; then
        if ! grep -q "restrict-lang/bin" "$shell_rc"; then
            info "Adding Restrict Language to $shell_rc..."
            {
                echo ""
                echo "# Restrict Language"
                echo "$export_line"
            } >> "$shell_rc"
            added=1
        fi
    fi

    if [ $added -eq 1 ]; then
        success "Added to PATH in $shell_rc"
        info "Run 'source $shell_rc' or restart your shell"
    else
        info "Restrict Language already in PATH"
    fi
}

# Main installation
main() {
    echo -e "${BOLD}Installing Restrict Language${NC}"
    echo "=============================="
    echo ""

    # Detect platform
    PLATFORM=$(detect_platform)
    if [ $? -ne 0 ]; then
        exit 1
    fi
    info "Detected platform: $PLATFORM"

    # Try binary download first
    if download_binary "$PLATFORM" "$RESTRICT_LANG_VERSION"; then
        :  # Success
    else
        # Fall back to building from source
        warning "Binary download failed, trying to build from source..."
        if ! build_from_source; then
            error "Installation failed"
            exit 1
        fi
    fi

    # Configure shell
    configure_shell

    # Verify installation
    export PATH="$INSTALL_DIR/bin:$PATH"
    
    echo ""
    if "$INSTALL_DIR/bin/restrict_lang" --version >/dev/null 2>&1; then
        success "Restrict Language compiler installed!"
        "$INSTALL_DIR/bin/restrict_lang" --version
    else
        error "Restrict Language compiler installation verification failed"
    fi

    if "$INSTALL_DIR/bin/warder" --version >/dev/null 2>&1; then
        success "Warder package manager installed!"
        "$INSTALL_DIR/bin/warder" --version
    else
        error "Warder installation verification failed"
    fi

    # Print next steps
    echo ""
    echo -e "${BOLD}Installation complete!${NC}"
    echo ""
    echo "To get started:"
    echo ""
    echo "  1. Reload your shell configuration:"
    echo "     source ~/.bashrc  # or ~/.zshrc"
    echo ""
    echo "  2. Create a new project:"
    echo "     warder new hello-world"
    echo "     cd hello-world"
    echo ""
    echo "  3. Run your project:"
    echo "     warder run"
    echo ""
    echo "Documentation: https://docs.restrict-lang.org"
    echo "Getting Started: https://docs.restrict-lang.org/getting-started"
}

# Run main
main "$@"