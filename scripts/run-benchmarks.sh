#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-full}"
OUTPUT="${2:-target/benchmark-results/restrict-baseline.json}"

case "$MODE" in
  smoke)
    SMOKE_FLAG="--smoke"
    ;;
  full)
    SMOKE_FLAG=""
    ;;
  *)
    echo "Usage: bash scripts/run-benchmarks.sh [smoke|full] [output.json]" >&2
    exit 2
    ;;
esac

mise exec -- cargo build -j 1 --locked --release --bin restrict_lang --bin restrict_bench

if [[ -n "$SMOKE_FLAG" ]]; then
  ./target/release/restrict_bench --smoke --output "$OUTPUT"
else
  ./target/release/restrict_bench --output "$OUTPUT"
fi
