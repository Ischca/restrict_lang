#!/bin/bash
# Development setup script for Restrict Language

set -e

echo "🚀 Restrict Language Development Setup"
echo "====================================="
echo ""

# Check Rust
if ! command -v cargo &> /dev/null; then
    echo "❌ Rust not found. Please install from https://rustup.rs/"
    exit 1
fi

if ! command -v mise &> /dev/null; then
    echo "mise not found. Please install it from https://mise.jdx.dev/"
    exit 1
fi

echo "✅ Rust found: $(rustc --version)"

# Build
echo ""
echo "Building Restrict Language compiler..."
mise exec -- cargo build --workspace --locked --release

# Create convenient scripts
echo ""
echo "Creating convenience scripts..."

mkdir -p bin

cat > bin/restrict_lang << 'EOF'
#!/bin/bash
exec "$(dirname "$0")/../target/release/restrict_lang" "$@"
EOF

cat > bin/warder << 'EOF'
#!/bin/bash
exec "$(dirname "$0")/../target/release/warder" "$@"
EOF

chmod +x bin/restrict_lang bin/warder

# Test
echo ""
echo "Testing installation..."

VERIFY_DIR=$(mktemp -d)
cat > "$VERIFY_DIR/main.rl" << 'EOF'
fun main: () -> () = {
    val message = "Development setup verified!"
    message |> println
}
EOF

./bin/restrict_lang "$VERIFY_DIR/main.rl" "$VERIFY_DIR/main.wat"
test -s "$VERIFY_DIR/main.wat"
rm -rf "$VERIFY_DIR"
./bin/warder --version

echo ""
echo "✅ Setup complete!"
echo ""
echo "To use Restrict Language, add this to your shell config:"
echo ""
echo "  export PATH=\"$PWD/bin:\$PATH\""
echo ""
echo "Or use the binaries directly:"
echo "  ./bin/warder new my-project"
echo "  ./bin/restrict_lang file.rl file.wat"
echo ""
