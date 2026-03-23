#!/usr/bin/env bash
# sync_samples.sh — Generate web/examples.js from samples/ directory.
#
# This script reads samples/manifest.json and the .rl files it references,
# then generates a JavaScript module that the playground can import.
#
# Usage:
#   ./scripts/sync_samples.sh          # from repo root
#   ./scripts/sync_samples.sh --check  # verify examples.js is up-to-date

set -euo pipefail

REPO_ROOT="$(cd "$(dirname "$0")/.." && pwd)"
SAMPLES_DIR="$REPO_ROOT/samples"
MANIFEST="$SAMPLES_DIR/manifest.json"
OUTPUT="$REPO_ROOT/web/examples.js"

if [ ! -f "$MANIFEST" ]; then
    echo "Error: $MANIFEST not found" >&2
    exit 1
fi

generate() {
    cat <<'HEADER'
// Auto-generated from samples/ — do not edit manually.
// Run: ./scripts/sync_samples.sh
//
// Each key maps to the contents of the corresponding .rl file in samples/.

export const examples = {
HEADER

    # Parse manifest.json with lightweight approach (no jq dependency)
    # Extract id and file fields
    local ids=()
    local files=()

    while IFS= read -r line; do
        if echo "$line" | grep -q '"id"'; then
            id=$(echo "$line" | sed 's/.*"id"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
            ids+=("$id")
        fi
        if echo "$line" | grep -q '"file"'; then
            file=$(echo "$line" | sed 's/.*"file"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/')
            files+=("$file")
        fi
    done < "$MANIFEST"

    for i in "${!ids[@]}"; do
        local id="${ids[$i]}"
        local file="${files[$i]}"
        local path="$SAMPLES_DIR/$file"

        if [ ! -f "$path" ]; then
            echo "Error: $path not found (referenced in manifest.json)" >&2
            exit 1
        fi

        # Read the file and escape for JS template literal
        local content
        content=$(cat "$path")

        # Escape backticks and ${...} for template literal
        content=$(echo "$content" | sed 's/\\/\\\\/g; s/`/\\`/g; s/\${/\\${/g')

        echo "    '$id': \`$content\`,"
        echo ""
    done

    echo "};"
}

if [ "${1:-}" = "--check" ]; then
    # Check mode: compare generated output with existing file
    expected=$(generate)
    if [ -f "$OUTPUT" ]; then
        actual=$(cat "$OUTPUT")
        if [ "$expected" = "$actual" ]; then
            echo "web/examples.js is up-to-date."
            exit 0
        else
            echo "web/examples.js is out of date. Run: ./scripts/sync_samples.sh" >&2
            exit 1
        fi
    else
        echo "web/examples.js does not exist. Run: ./scripts/sync_samples.sh" >&2
        exit 1
    fi
else
    generate > "$OUTPUT"
    echo "Generated $OUTPUT from $(ls "$SAMPLES_DIR"/*.rl | wc -l) sample files."
fi
