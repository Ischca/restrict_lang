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

bash scripts/record-benchmark-evidence.sh \
  "$OUTPUT_DIR" \
  "$RUN_COUNT" \
  benchmarks/stability-policy.json
