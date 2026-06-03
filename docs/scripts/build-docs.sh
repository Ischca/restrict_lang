#!/bin/bash

# Build documentation with code includes

echo "📚 Building documentation with shared code examples..."

# Process English documentation
echo "Processing English docs..."
for file in docs/public/en/**/*-template.md; do
    if [ -f "$file" ]; then
        output="${file%-template.md}.md"
        node docs/scripts/include-code.js "$file" "$output"
    fi
done

# Process Japanese documentation
echo "Processing Japanese docs..."
for file in docs/public/ja/**/*-template.md; do
    if [ -f "$file" ]; then
        output="${file%-template.md}.md"
        node docs/scripts/include-code.js "$file" "$output"
    fi
done

echo "✅ Documentation build complete!"
