# Benchmarking

Restrict's in-repository benchmarks are regression tools for the compiler and
generated Core WebAssembly. They are not yet evidence for public claims against
Rust, Grain, MoonBit, or another language.

## Run the suite

Use the short mode while changing the compiler:

```bash
mise run bench-smoke
```

Use the full local baseline when reviewing performance:

```bash
mise run bench
```

Both commands build the compiler and the native `wasmi` benchmark runner in
release mode. Results are written to
`target/benchmark-results/restrict-baseline.json`; generated Wasm artifacts are
stored beside the report under `artifacts/`.

## What is measured

Every workload records:

- compiler process time;
- Wasm runtime compilation and cold instantiation time;
- raw and Zstandard-compressed artifact size;
- warm execution samples plus minimum, median, mean, and maximum; and
- source and Wasm SHA-256 digests, tool versions, host OS and architecture,
  source revision, and dirty-worktree state.

The current corpus covers scalar loops, recursion, records, and a
`map`/`filter`/`fold` collection pipeline. Each workload exports one scalar
`benchmark` function and declares an expected checksum. The runner checks the
result during warm-up and for every measured iteration. A wrong result fails
the run instead of becoming a misleading fast measurement.

Each artifact is compiled in a separate compiler process. Runtime measurements
use one freshly compiled and instantiated module per workload, run the declared
warm-up calls, and then time individual calls on that same instance. The median
is the primary local comparison value; raw nanosecond samples remain in the
JSON report so another summary can be calculated without rerunning the suite.

## Target boundary

The suite compiles with:

```text
--target wasm-core --emit wasm
```

`wasm-core` artifacts must have no imports. Host output such as `print` or
`println` requires `--target wasip1` and is rejected in the benchmark target.
The benchmark manifest also selects an explicit arena capacity per workload so
collection tests are not limited by the former fixed 4 KiB capacity.

This first B0 slice records raw local measurements. Stable public baselines
still require a pinned controlled runner, a documented regression threshold,
and an external cross-language harness.
