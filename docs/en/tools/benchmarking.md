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

- separate compiler process times and SHA-256 digests for raw and `--release`
  artifacts;
- Wasm runtime compilation and cold instantiation time;
- raw and Zstandard-compressed sizes for both artifacts;
- warm execution samples plus minimum, median, mean, and maximum; and
- source and Wasm SHA-256 digests, tool versions, host OS and architecture,
  source revision, and dirty-worktree state.

The runner compiles the `--release` artifact twice and requires exact byte
identity. It validates both raw and release Wasm and rejects imports in both,
then executes the release artifact. A non-reproducible release artifact fails
the run before timing begins.

The current corpus covers scalar loops, recursion, records, and a
`map`/`filter`/`fold` collection pipeline. Each workload exports one scalar
`benchmark` function and declares an expected checksum. The runner checks the
result during warm-up and for every measured iteration. A wrong result fails
the run instead of becoming a misleading fast measurement.

## Frozen B0 workload subset

Cross-language work may use only the Restrict behavior exercised here until a
later milestone deliberately expands the subset:

- `Int32` literals, arithmetic, comparisons, mutable locals, branches, and
  `while` loops;
- monomorphic direct calls, scalar exports, and direct recursion;
- concrete records whose benchmark-observed fields are `Int32`; and
- `List<Int32>`, non-capturing scoped lambdas, and the current
  `map`/`filter`/`fold` paths.

The subset excludes host I/O, generic host exports, composite host ABI values,
temporal features, async, WIT, and platform APIs. Passing the corpus freezes
only these workload forms; it does not imply that every accepted language
feature has a benchmark-stable lowering.

Each raw or release artifact is compiled in a separate compiler process.
Runtime measurements use one freshly compiled and instantiated release module
per workload, run the declared warm-up calls, and then time individual calls on
that same instance. The median is the primary local comparison value; raw
nanosecond samples remain in the JSON report so another summary can be
calculated without rerunning the suite. Benchmark report schema 2 stores the
two artifacts under `rawArtifact` and `optimizedArtifact`.

## Target boundary

The suite compiles with:

```text
--target wasm-core --emit wasm
```

The optimized artifact adds `--release`. The raw artifact remains the control
measurement, while the release artifact is the executable used for runtime
timing. External `wasm-opt` output is not part of the current baseline.

`wasm-core` artifacts must have no imports. Host output such as `print` or
`println` requires `--target wasip1` and is rejected in the benchmark target.
The benchmark manifest also selects an explicit arena capacity per workload so
collection tests are not limited by the former fixed 4 KiB capacity.

This B0 slice records attributable raw and compiler-release local measurements.
Stable public baselines
still require a pinned controlled runner, a documented regression threshold,
and an external cross-language harness.
