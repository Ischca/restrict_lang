#!/usr/bin/env bash

set -euo pipefail

OUTPUT_DIR="${1:-target/benchmark-results/evidence}"
RUN_COUNT="${2:-5}"

if [[ ! "$RUN_COUNT" =~ ^[0-9]+$ ]] || (( RUN_COUNT < 5 )); then
  echo "run count must be an integer of at least 5" >&2
  exit 2
fi

mise exec -- cargo build -j 1 --locked --release \
  --bin restrict_lang \
  --bin restrict_bench \
  --bin restrict_bench_compare \
  --bin restrict_bench_stability

mkdir -p "$OUTPUT_DIR"
./target/release/restrict_bench --output "$OUTPUT_DIR/warmup.json"

reports=()
for ((run = 1; run <= RUN_COUNT; run++)); do
  report="$OUTPUT_DIR/run-${run}.json"
  ./target/release/restrict_bench --output "$report"
  ./target/release/restrict_bench_compare \
    --baseline benchmarks/baselines/core-wasm-v0.0.1.json \
    --candidate "$report" \
    --policy benchmarks/regression-policy.json
  reports+=("$report")
done

./target/release/restrict_bench_stability \
  --policy benchmarks/stability-policy.json \
  --output "$OUTPUT_DIR/stability-summary.json" \
  "${reports[@]}"
