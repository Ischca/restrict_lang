#!/bin/bash
# Translation synchronization helper script

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo "🌐 Restrict Language Documentation Translation Sync Tool"
echo "======================================================"

# Function to check if file exists
check_file() {
    if [ -f "$1" ]; then
        echo -e "${GREEN}✓${NC} $1"
        return 0
    else
        echo -e "${RED}✗${NC} $1 (missing)"
        return 1
    fi
}

# Function to compare modification times
check_outdated() {
    en_file=$1
    ja_file=$2
    
    if [ ! -f "$ja_file" ]; then
        echo -e "${RED}Missing:${NC} $ja_file"
        return 1
    fi
    
    if [ "$en_file" -nt "$ja_file" ]; then
        echo -e "${YELLOW}Outdated:${NC} $ja_file (EN version is newer)"
        return 1
    fi
    
    return 0
}

# List all documentation files
echo -e "\n📁 Checking translation status..."
echo "================================"

# Define file mappings
declare -A file_map=(
    ["en/introduction.md"]="ja/introduction.md"
    ["en/getting-started/installation.md"]="ja/getting-started/installation.md"
    ["en/getting-started/hello-world.md"]="ja/getting-started/hello-world.md"
    ["en/guide/syntax.md"]="ja/guide/syntax.md"
    ["en/guide/types.md"]="ja/guide/types.md"
    ["en/guide/osv-order.md"]="ja/guide/osv-order.md"
    ["en/guide/warder.md"]="ja/guide/warder.md"
    ["en/reference/stdlib.md"]="ja/reference/stdlib.md"
)

missing_count=0
outdated_count=0

# Check each file pair
for en_file in "${!file_map[@]}"; do
    ja_file=${file_map[$en_file]}
    
    if [ ! -f "$ja_file" ]; then
        ((missing_count++))
        echo -e "${RED}Missing:${NC} $ja_file"
    elif [ "$en_file" -nt "$ja_file" ]; then
        ((outdated_count++))
        echo -e "${YELLOW}Outdated:${NC} $ja_file"
    else
        echo -e "${GREEN}Up-to-date:${NC} $ja_file"
    fi
done

# Summary
echo -e "\n📊 Summary"
echo "=========="
echo "Missing translations: $missing_count"
echo "Outdated translations: $outdated_count"

# Generate TODO list
if [ $missing_count -gt 0 ] || [ $outdated_count -gt 0 ]; then
    echo -e "\n📝 TODO List"
    echo "============"
    
    for en_file in "${!file_map[@]}"; do
        ja_file=${file_map[$en_file]}
        
        if [ ! -f "$ja_file" ]; then
            echo "- Create: $ja_file"
        elif [ "$en_file" -nt "$ja_file" ]; then
            echo "- Update: $ja_file (check diff with $en_file)"
        fi
    done
fi

# Offer diff viewing
echo -e "\n🔍 View differences? (y/n)"
read -r response
if [[ "$response" =~ ^[Yy]$ ]]; then
    for en_file in "${!file_map[@]}"; do
        ja_file=${file_map[$en_file]}
        
        if [ -f "$ja_file" ] && [ "$en_file" -nt "$ja_file" ]; then
            echo -e "\n${YELLOW}Diff for $ja_file:${NC}"
            echo "=================="
            # Show what changed in English version
            git diff --no-index --word-diff "$ja_file" "$en_file" || true
            echo -e "\nPress Enter to continue..."
            read -r
        fi
    done
fi