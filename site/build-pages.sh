#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SITE_DIR="$ROOT_DIR/site"
DIST_DIR="${PAGES_DIST_DIR:-$SITE_DIR/dist}"

fail() {
    echo "error: $*" >&2
    exit 1
}

require_file() {
    local path="$1"
    local hint="${2:-}"

    if [ -f "$path" ]; then
        return
    fi

    if [ -n "$hint" ]; then
        fail "$path is missing; $hint"
    fi

    fail "$path is missing"
}

require_dir() {
    local path="$1"
    local hint="${2:-}"

    if [ -d "$path" ]; then
        return
    fi

    if [ -n "$hint" ]; then
        fail "$path is missing; $hint"
    fi

    fail "$path is missing"
}

require_file "$SITE_DIR/index.html"
require_file "$SITE_DIR/styles.css"
require_file "$SITE_DIR/restrict-highlight.js"
require_file "$SITE_DIR/restrict-code-blocks.js"
require_file "$SITE_DIR/favicon.svg"
require_file "$SITE_DIR/404.html"
require_file "$SITE_DIR/robots.txt"
require_file "$SITE_DIR/sitemap.xml"
require_file "$SITE_DIR/blog/index.html"
require_file "$SITE_DIR/blog/type-inference-v001.html"
require_file "$SITE_DIR/blog/runtime-dogfood.html"
require_file "$SITE_DIR/tools/highlight-theme-lab.html"
require_file "$ROOT_DIR/docs/book/index.html" "run mdbook build docs before assembling Pages"
require_file "$ROOT_DIR/web/index.html"
require_file "$ROOT_DIR/web/app.js"
require_file "$ROOT_DIR/web/restrict-highlight.js"
require_dir "$ROOT_DIR/web/pkg" "run wasm-pack build --target web --out-dir web/pkg before assembling Pages"
require_file "$ROOT_DIR/web/pkg/restrict_lang.js" "the wasm-pack JavaScript glue is required by /compiler/app.js"

if ! find "$ROOT_DIR/web/pkg" -maxdepth 1 -type f -name '*.wasm' -print -quit | grep -q .; then
    fail "$ROOT_DIR/web/pkg does not contain a .wasm bundle"
fi

mkdir -p "$(dirname "$DIST_DIR")"
TMP_DIR="$(mktemp -d "${DIST_DIR}.tmp.XXXXXX")"

cleanup() {
    rm -rf "$TMP_DIR"
}

trap cleanup EXIT

mkdir -p "$TMP_DIR"

cp "$SITE_DIR/index.html" "$TMP_DIR/index.html"
cp "$SITE_DIR/styles.css" "$TMP_DIR/styles.css"
cp "$SITE_DIR/restrict-highlight.js" "$TMP_DIR/restrict-highlight.js"
cp "$SITE_DIR/restrict-code-blocks.js" "$TMP_DIR/restrict-code-blocks.js"
cp "$SITE_DIR/favicon.svg" "$TMP_DIR/favicon.svg"
cp "$SITE_DIR/404.html" "$TMP_DIR/404.html"
cp "$SITE_DIR/robots.txt" "$TMP_DIR/robots.txt"
cp "$SITE_DIR/sitemap.xml" "$TMP_DIR/sitemap.xml"

mkdir -p "$TMP_DIR/blog"
cp "$SITE_DIR/blog/"*.html "$TMP_DIR/blog/"

mkdir -p "$TMP_DIR/tools"
cp "$SITE_DIR/tools/"*.html "$TMP_DIR/tools/"

mkdir -p "$TMP_DIR/docs"
cp -R "$ROOT_DIR/docs/book/." "$TMP_DIR/docs/"

mkdir -p "$TMP_DIR/compiler"
cp "$ROOT_DIR/web/index.html" "$TMP_DIR/compiler/index.html"
cp "$ROOT_DIR/web/app.js" "$TMP_DIR/compiler/app.js"
cp "$ROOT_DIR/web/restrict-highlight.js" "$TMP_DIR/compiler/restrict-highlight.js"
cp -R "$ROOT_DIR/web/pkg" "$TMP_DIR/compiler/pkg"

touch "$TMP_DIR/.nojekyll"

require_file "$TMP_DIR/index.html"
require_file "$TMP_DIR/restrict-highlight.js"
require_file "$TMP_DIR/restrict-code-blocks.js"
require_file "$TMP_DIR/favicon.svg"
require_file "$TMP_DIR/404.html"
require_file "$TMP_DIR/blog/index.html"
require_file "$TMP_DIR/tools/highlight-theme-lab.html"
require_file "$TMP_DIR/docs/index.html"
require_file "$TMP_DIR/compiler/index.html"
require_file "$TMP_DIR/compiler/app.js"
require_file "$TMP_DIR/compiler/restrict-highlight.js"
require_file "$TMP_DIR/compiler/pkg/restrict_lang.js"
require_file "$TMP_DIR/.nojekyll"

rm -rf "$DIST_DIR"
mv "$TMP_DIR" "$DIST_DIR"
trap - EXIT

echo "Built Pages site at $DIST_DIR"
