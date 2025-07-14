#!/bin/bash
# Verify Restrict Language installation

set -e

# Colors
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m'

echo "🔍 Verifying Restrict Language Installation"
echo "=========================================="
echo ""

# Check commands
check_command() {
    local cmd=$1
    local name=$2
    
    if command -v "$cmd" >/dev/null 2>&1; then
        echo -e "${GREEN}✓${NC} $name found"
        $cmd --version
        return 0
    else
        echo -e "${RED}✗${NC} $name not found"
        return 1
    fi
}

# Check installations
ERRORS=0

echo "Checking core tools:"
check_command "restrict_lang" "Restrict Language Compiler" || ((ERRORS++))
check_command "warder" "Warder Package Manager" || ((ERRORS++))

echo ""
echo "Checking optional tools:"
check_command "code" "VS Code" || echo "  (Optional - for IDE support)"

# Test compilation
echo ""
echo "Testing compilation:"

TEMP_DIR=$(mktemp -d)
cd "$TEMP_DIR"

cat > test.rl << 'EOF'
fn main() {
    "Installation verified!" |> println
}
EOF

if restrict_lang compile test.rl >/dev/null 2>&1; then
    echo -e "${GREEN}✓${NC} Compilation works"
else
    echo -e "${RED}✗${NC} Compilation failed"
    ((ERRORS++))
fi

# Test Warder
echo ""
echo "Testing Warder:"

if warder --help >/dev/null 2>&1; then
    echo -e "${GREEN}✓${NC} Warder works"
else
    echo -e "${RED}✗${NC} Warder failed"
    ((ERRORS++))
fi

# Cleanup
cd /
rm -rf "$TEMP_DIR"

# Summary
echo ""
echo "=========================================="
if [ $ERRORS -eq 0 ]; then
    echo -e "${GREEN}✅ All checks passed!${NC}"
    echo ""
    echo "You're ready to start developing with Restrict Language!"
    echo ""
    echo "Next steps:"
    echo "  warder new my-project"
    echo "  cd my-project"
    echo "  warder run"
else
    echo -e "${RED}❌ Some checks failed${NC}"
    echo ""
    echo "Please ensure Restrict Language is properly installed and in your PATH."
    echo "Installation guide: https://docs.restrict-lang.org/getting-started/installation"
fi