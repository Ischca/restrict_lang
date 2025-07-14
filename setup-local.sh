#!/bin/bash
# Local installation setup for Restrict Language

INSTALL_DIR="$HOME/.local/bin"
REPO_DIR="$(pwd)"

echo "🚀 Installing Restrict Language locally"
echo "====================================="
echo ""

# Create install directory
mkdir -p "$INSTALL_DIR"

# Create wrapper scripts
echo "Creating wrapper scripts..."

cat > "$INSTALL_DIR/restrict_lang" << EOF
#!/bin/bash
exec "$REPO_DIR/target/release/restrict_lang" "\$@"
EOF

cat > "$INSTALL_DIR/warder" << EOF
#!/bin/bash
export RESTRICT_LANG_BIN="$REPO_DIR/target/release/restrict_lang"
exec "$REPO_DIR/target/release/warder" "\$@"
EOF

chmod +x "$INSTALL_DIR/restrict_lang"
chmod +x "$INSTALL_DIR/warder"

echo ""
echo "✅ Installation complete!"
echo ""
echo "Add the following to your shell configuration (.bashrc, .zshrc, etc.):"
echo ""
echo "  export PATH=\"\$HOME/.local/bin:\$PATH\""
echo ""
echo "Then reload your shell or run:"
echo "  source ~/.zshrc  # or ~/.bashrc"
echo ""
echo "You can then use:"
echo "  warder new my-project"
echo "  restrict_lang compile file.rl"