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

Enforce the checked-in deterministic baseline from a clean worktree:

```bash
mise run bench-gate
```

Record five full reports and assess within-run timing stability:

```bash
mise run bench-evidence
```

The smoke, full, and deterministic-gate commands build the compiler and the
native `wasmi` benchmark runner in release mode. Results are written to
`target/benchmark-results/restrict-baseline.json`; generated Wasm artifacts are
stored beside the report under `artifacts/`.

`benchmarks/baselines/core-wasm-v0.0.1.json` is the reviewed baseline and
`benchmarks/regression-policy.json` is the machine-readable policy. The gate
requires an exact compiler/runtime/validator/compressor toolchain, workload
sources and inputs, Wasm hashes, and memory observations. It also rejects any
raw, release, or instrumented artifact growth. An intended compiler or corpus
change therefore requires an explicit baseline review instead of silently
moving the reference.

Repeated evidence is stored under `target/benchmark-results/evidence/`.
`benchmarks/stability-policy.json` requires the reports to come from the same
clean source revision, host, toolchain, target, mode, and workload set. The
generated `stability-summary.json` records the median absolute deviation and
the full relative range for execution median, optimized compiler time, runtime
compilation, and cold instantiation.

## What is measured

Every workload records:

- separate compiler process times and SHA-256 digests for raw and `--release`
  artifacts;
- Wasm runtime compilation and cold instantiation time;
- raw and Zstandard-compressed sizes for both artifacts;
- warm execution samples plus minimum, median, mean, and maximum; and
- peak live arena bytes, allocation count, completed reset count, and live
  bytes after a host call from a separate instrumented artifact; and
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
calculated without rerunning the suite. Benchmark report schema 4 stores the
timing artifacts under `rawArtifact` and `optimizedArtifact`, the memory probe
under `instrumentedArtifact`, and its observations under `memory`. It also
records exact runner, validator, encoder, compressor, Rust, and compiler-owned
optimizer identities.

## Target boundary

The suite compiles with:

```text
--target wasm-core --emit wasm
```

The optimized artifact adds `--release`. The raw artifact remains the control
measurement, while the release artifact is the executable used for runtime
timing. A third artifact adds both `--release` and `--instrument-memory`. The
runner resets its counters and executes two identical verified calls, requiring
the same peak/allocation result, exactly one completed arena reset, and zero
live bytes after each call. Because this bookkeeping changes generated code,
the instrumented artifact is never used for runtime timing. External
`wasm-opt` output is not part of the current baseline.

`wasm-core` artifacts must have no imports. Host output such as `print` or
`println` requires `--target wasip1` and is rejected in the benchmark target.
The benchmark manifest also selects an explicit arena capacity per workload so
collection tests are not limited by the former fixed 4 KiB capacity.

Selected allocation-heavy workloads also declare a deliberately undersized
arena probe. That call must trap after recording arena error code `1`, a
non-zero requested allocation size, and the configured capacity. This verifies
that exhaustion is explicit and attributable without making it a recoverable
Restrict error.

This B0 slice records attributable raw and compiler-release local measurements.
The `Core Wasm benchmark evidence` workflow pins `ubuntu-24.04` and Rust
1.94.1, warms the runtime and filesystem caches, records five full reports, and
uploads every raw report and Wasm artifact on a weekly schedule, release tags,
and manual dispatches. The report records the actual CPU model because the
managed runner label does not guarantee identical physical hardware.

Timing is deliberately `informational` in the current policy. The 15% execution
and 25% compile/instantiation thresholds are implemented but are activated only
after a reviewed report from one repeatable CPU/runner is promoted with
`timingStatus: controlled`. Until then, artifact and memory regressions fail the
gate while timing evidence is retained without making a false stability claim.
Public cross-language comparisons still require the separate harness to pin
every comparison compiler and run equivalent workloads on the same controlled
host.

The separate stability policy is also informational on the shared GitHub
runner. Threshold failures are preserved in the summary but do not fail that
evidence workflow. A controlled promotion requires a dedicated repeatable
runner, non-unknown CPU identity, repeated passing evidence, and an explicit
review that changes both stability and regression timing policies to
`enforced`. One quiet run on shared hardware is not promotion evidence.

## Controlled timing promotion

A runner may be promoted only through this reviewable sequence:

1. Reserve one dedicated machine or VM class with a stable CPU model, OS image,
   and power policy. Register it as a repository runner with the custom
   `restrict-benchmark` label, then set the repository variable
   `RESTRICT_CONTROLLED_BENCHMARK_ENABLED` to `true`. Shared GitHub-hosted
   hardware remains evidence collection only.
2. At one clean compiler revision, collect at least three independent evidence
   sets of five full reports each. Run the sets in separate scheduling windows
   so one temporarily quiet period cannot qualify the runner.
3. Require every deterministic gate and every within-set stability threshold
   to pass. Do not delete or replace outliers after observing them.
4. Use the median of all 15 or more accepted samples for each workload's four
   timing references. Preserve every raw report and stability summary with the
   promotion pull request.
5. Change the baseline `timingHost` and `timingStatus` to the reviewed runner
   and `controlled`, set the stability policy runner class to that dedicated
   class, and change both stability and regression timing statuses to
   `enforced` in the same pull request.
6. Demote timing to `informational` and requalify after changing the CPU, OS
   image, Rust/compiler/runtime toolchain, power policy, or runner isolation.

Artifact hashes, sizes, checksums, and memory observations remain enforced
regardless of timing promotion state.

The conditional `Dedicated runner qualification evidence` job uses the
informational `stability-policy.dedicated-candidate.json` until promotion. It
is deliberately absent from `pull_request` events: do not broaden its triggers
while this public repository can receive untrusted fork code. Schedule, release
tag, and explicit manual dispatch are the allowed qualification entry points.
If the repository variable is absent or not `true`, the job is skipped and no
self-hosted runner is required.
