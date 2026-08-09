#!/usr/bin/env bash

set -euo pipefail

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
mode="${1:-}"

cd "$repo_root"

case "$mode" in
  core)
    cargo test --workspace --locked --lib --examples --quiet
    exec cargo test --workspace --locked --doc --quiet
    ;;
  warder)
    exec cargo test -p warder --locked --quiet -- --test-threads=1
    ;;
  integration)
    shard_index="${2:-}"
    shard_count="${3:-}"
    if [[ ! "$shard_index" =~ ^[0-9]+$ || ! "$shard_count" =~ ^[1-9][0-9]*$ ]]; then
      echo "Usage: bash scripts/ci-test-shard.sh integration INDEX COUNT" >&2
      exit 2
    fi
    if ((shard_index >= shard_count)); then
      echo "INDEX must be smaller than COUNT" >&2
      exit 2
    fi

    test_files=()
    while IFS= read -r test_file; do
      test_files+=("$test_file")
    done < <(find tests -maxdepth 1 -type f -name '*.rs' -print | LC_ALL=C sort)

    cargo_args=(cargo test -p restrict_lang --locked --quiet)
    selected_targets=()
    for index in "${!test_files[@]}"; do
      if ((index % shard_count != shard_index)); then
        continue
      fi
      target="${test_files[$index]#tests/}"
      target="${target%.rs}"
      cargo_args+=(--test "$target")
      selected_targets+=("$target")
    done

    if ((${#selected_targets[@]} == 0)); then
      echo "Integration shard $shard_index/$shard_count selected no tests" >&2
      exit 2
    fi

    echo "Integration shard $((shard_index + 1))/$shard_count: ${#selected_targets[@]} targets"
    printf '  %s\n' "${selected_targets[@]}"
    cargo_args+=(-- --test-threads=1)
    exec "${cargo_args[@]}"
    ;;
  *)
    echo "Usage: bash scripts/ci-test-shard.sh core|warder|integration INDEX COUNT" >&2
    exit 2
    ;;
esac
