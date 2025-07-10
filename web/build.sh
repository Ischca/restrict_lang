#!/bin/bash

# Build script for the Restrict Language Web Compiler
echo "🔨 Building Restrict Language Web Compiler..."

# Check if wasm-pack is installed
if ! command -v wasm-pack &> /dev/null; then
    echo "❌ wasm-pack is not installed. Please install it with:"
    echo "curl https://rustwasm.github.io/wasm-pack/installer/init.sh -sSf | sh"
    exit 1
fi

# Navigate to the project root
cd "$(dirname "$0")/.."

# Build the WASM package
echo "📦 Building WASM package..."
wasm-pack build --target web --out-dir web/pkg

# Check if build was successful
if [ $? -eq 0 ]; then
    echo "✅ Build successful!"
    echo "📁 Files generated in web/pkg/"
    echo "🌐 You can now serve the web directory with any HTTP server"
    echo ""
    echo "To serve locally, you can use:"
    echo "  cd web && python -m http.server 8000"
    echo "  or"
    echo "  cd web && npx serve ."
    echo ""
    echo "Then open http://localhost:8000 in your browser"
else
    echo "❌ Build failed!"
    exit 1
fi