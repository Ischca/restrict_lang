#!/bin/bash

# Convert SVG icons to PNG for VS Code extension
# Requires: brew install librsvg (on macOS) or apt-get install librsvg2-bin (on Linux)

echo "Converting icons..."

# VS Code extension icon (128x128)
if command -v rsvg-convert &> /dev/null; then
    rsvg-convert -w 128 -h 128 ../vscode-extension/images/icon.svg -o ../vscode-extension/images/icon.png
    echo "✓ VS Code icon created"
else
    echo "⚠️  rsvg-convert not found. Install with: brew install librsvg"
fi

# Create smaller versions if needed
# rsvg-convert -w 64 -h 64 logo.svg -o icon-64.png
# rsvg-convert -w 32 -h 32 logo.svg -o icon-32.png

echo "Done!"