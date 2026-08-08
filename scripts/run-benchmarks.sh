#!/usr/bin/env bash
set -euo pipefail

MODE="${1:-full}"
OUTPUT="${2:-target/benchmark-results/restrict-baseline.json}"
COMPARE=false

case "$MODE" in
  smoke)
    SMOKE_FLAG="--smoke"
    ;;
  full)
    SMOKE_FLAG=""
    ;;
  gate)
    SMOKE_FLAG=""
    COMPARE=true
    ;;
  *)
    echo "Usage: bash scripts/run-benchmarks.sh [smoke|full|gate] [output.json]" >&2
    exit 2
    ;;
esac

mise exec -- cargo build -j 1 --locked --release \
  --bin restrict_lang --bin restrict_bench --bin restrict_bench_compare

if [[ -n "$SMOKE_FLAG" ]]; then
  ./target/release/restrict_bench --smoke --output "$OUTPUT"
else
  ./target/release/restrict_bench --output "$OUTPUT"
fi

if [[ "$COMPARE" == true ]]; then
  ./target/release/restrict_bench_compare \
    --baseline benchmarks/baselines/core-wasm-v0.0.1.json \
    --candidate "$OUTPUT" \
    --policy benchmarks/regression-policy.json
fi
