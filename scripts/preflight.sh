#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mode="${1:-full}"
started_at=$SECONDS

case "$mode" in
  quick | full | pages) ;;
  *)
    echo "Usage: bash scripts/preflight.sh [quick|full|pages]" >&2
    exit 2
    ;;
esac

cd "$repo_root"
export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$repo_root/target}"

run_step() {
  local label="$1"
  shift
  local step_started=$SECONDS

  echo
  echo "==> $label"
  "$@"
  echo "<== $label ($((SECONDS - step_started))s)"
}

run_step "Check patch whitespace" git diff --check
run_step "Check generated playground samples" bash scripts/sync_samples.sh --check
run_step "Check Rust formatting" cargo fmt --all -- --check
run_step "Lint workspace" cargo clippy --workspace --locked -- -D warnings
run_step "Run high-signal regression gates" \
  cargo test -p restrict_lang --locked --quiet \
    --test test_codegen_boundaries \
    --test test_docs_hygiene \
    --test test_method_resolution \
    --test test_release_example_hygiene \
    --test test_release_surface_matrix \
    --test test_release_surface_validator \
    --test test_std_source_hygiene \
    --test test_wasm_records_runtime \
    --test test_web_hygiene

if [[ "$mode" == "quick" ]]; then
  echo
  echo "Quick preflight passed in $((SECONDS - started_at))s"
  exit 0
fi

run_step "Run complete workspace test suite" cargo test --workspace --locked --quiet
run_step "Compile standalone release examples through the CLI" \
  cargo test -p restrict_lang --locked \
    --test test_release_example_hygiene \
    standalone_release_examples_compile_through_cli \
    -- --ignored --exact
run_step "Compile VS Code release examples through the CLI" \
  cargo test -p restrict_lang --locked \
    --test test_release_example_hygiene \
    vscode_release_examples_compile_through_cli \
    -- --ignored --exact

if [[ "$mode" == "pages" ]]; then
  run_step "Check mdBook" mdbook --version
  run_step "Check wasm-pack" wasm-pack --version
  run_step "Build mdBook" mdbook build docs
  run_step "Build the online compiler" wasm-pack build --target web --out-dir web/pkg
  run_step "Assemble the Pages artifact" bash scripts/build-pages.sh
fi

run_step "Smoke test the checked-in web runtime" node scripts/smoke-web-runtime.mjs
run_step "Recheck patch whitespace" git diff --check

echo
echo "${mode^} preflight passed in $((SECONDS - started_at))s"
