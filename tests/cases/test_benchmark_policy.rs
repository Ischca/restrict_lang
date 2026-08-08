use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;
use std::process::{Command, Output};
use tempfile::tempdir;

fn read_repository_json(relative_path: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(relative_path);
    let contents = fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
    serde_json::from_str(&contents)
        .unwrap_or_else(|error| panic!("failed to parse {}: {error}", path.display()))
}

fn workload_ids(document: &Value) -> BTreeSet<&str> {
    document["workloads"]
        .as_array()
        .expect("benchmark document should contain a workloads array")
        .iter()
        .map(|workload| {
            workload["id"]
                .as_str()
                .expect("every benchmark workload should have a string id")
        })
        .collect()
}

fn candidate_report(median_ns: u64, release_sha: &str) -> Value {
    json!({
        "schemaVersion": 4,
        "sourceRevision": "candidate",
        "sourceDirty": false,
        "target": "wasm-core",
        "mode": "full",
        "host": { "os": "linux", "architecture": "x86_64", "cpu": "Test CPU" },
        "tools": {
            "compiler": "restrict_lang 0.0.1",
            "runner": "restrict_bench 0.0.1 (wasmi 1.1.0)",
            "validator": "wasmparser 0.252.0",
            "encoder": "wat 1.252.0",
            "optimizer": "restrict release reachability-dce v1 (no external wasm-opt)",
            "compression": "zstd 0.13.3 level 19",
            "rustc": "rustc 1.94.1 (test)"
        },
        "workloads": [{
            "id": "scalar",
            "source": "corpus/scalar.rl",
            "sourceSha256": "source-sha",
            "export": "benchmark",
            "input": 10,
            "expected": 45,
            "arenaBytes": 4096,
            "rawArtifact": {
                "wasmSha256": "raw-sha", "compileNs": 100, "wasmBytes": 1000, "zstdBytes": 500
            },
            "optimizedArtifact": {
                "wasmSha256": release_sha, "compileNs": 100, "wasmBytes": 400, "zstdBytes": 200
            },
            "instrumentedArtifact": {
                "wasmSha256": "memory-sha", "compileNs": 100, "wasmBytes": 500, "zstdBytes": 250
            },
            "releaseReproducible": true,
            "memory": {
                "peakBytes": 0,
                "allocationCount": 0,
                "resetCount": 1,
                "liveBytesAfterCall": 0,
                "verifiedIterations": 2,
                "exhaustion": null
            },
            "runtimeCompileNs": 100,
            "coldInstantiationNs": 100,
            "warmupIterations": 5,
            "iterations": 30,
            "execution": { "minNs": 90, "medianNs": median_ns, "meanNs": 100.0, "maxNs": 110 },
            "executionSamplesNs": [90, median_ns, 110]
        }]
    })
}

fn baseline(timing_status: &str) -> Value {
    json!({
        "schemaVersion": 1,
        "reportSchemaVersion": 4,
        "target": "wasm-core",
        "mode": "full",
        "toolchain": {
            "compiler": "restrict_lang 0.0.1",
            "runner": "restrict_bench 0.0.1 (wasmi 1.1.0)",
            "validator": "wasmparser 0.252.0",
            "encoder": "wat 1.252.0",
            "optimizer": "restrict release reachability-dce v1 (no external wasm-opt)",
            "compression": "zstd 0.13.3 level 19",
            "rustc": "rustc 1.94.1 (test)"
        },
        "provenance": {
            "timingHost": { "os": "linux", "architecture": "x86_64", "cpu": "Test CPU" },
            "timingStatus": timing_status
        },
        "workloads": [{
            "id": "scalar",
            "source": "corpus/scalar.rl",
            "sourceSha256": "source-sha",
            "export": "benchmark",
            "input": 10,
            "expected": 45,
            "arenaBytes": 4096,
            "rawArtifact": { "wasmSha256": "raw-sha", "wasmBytes": 1000, "zstdBytes": 500 },
            "optimizedArtifact": { "wasmSha256": "release-sha", "wasmBytes": 400, "zstdBytes": 200 },
            "instrumentedArtifact": { "wasmSha256": "memory-sha", "wasmBytes": 500, "zstdBytes": 250 },
            "releaseReproducible": true,
            "memory": {
                "peakBytes": 0,
                "allocationCount": 0,
                "resetCount": 1,
                "liveBytesAfterCall": 0,
                "verifiedIterations": 2,
                "exhaustion": null
            },
            "warmupIterations": 5,
            "iterations": 30,
            "timingReference": {
                "optimizedCompileNs": 100,
                "runtimeCompileNs": 100,
                "coldInstantiationNs": 100,
                "medianNs": 100
            }
        }]
    })
}

fn policy(status: &str) -> Value {
    json!({
        "schemaVersion": 1,
        "requireCleanCandidate": true,
        "requireExactToolchain": true,
        "requireExactSourcesAndInputs": true,
        "requireExactArtifactHashes": true,
        "maximumWasmGrowthBytes": 0,
        "maximumCompressedGrowthBytes": 0,
        "requireExactMemory": true,
        "timing": {
            "status": status,
            "requireExactHost": true,
            "minimumIterations": 30,
            "maximumMedianRegressionPercent": 15.0,
            "maximumOptimizedCompileRegressionPercent": 25.0,
            "maximumRuntimeCompileRegressionPercent": 25.0,
            "maximumColdInstantiationRegressionPercent": 25.0
        }
    })
}

fn run_compare(candidate: Value, baseline: Value, policy: Value) -> Output {
    let directory = tempdir().expect("temporary benchmark policy directory should be created");
    let candidate_path = directory.path().join("candidate.json");
    let baseline_path = directory.path().join("baseline.json");
    let policy_path = directory.path().join("policy.json");
    fs::write(&candidate_path, serde_json::to_vec(&candidate).unwrap()).unwrap();
    fs::write(&baseline_path, serde_json::to_vec(&baseline).unwrap()).unwrap();
    fs::write(&policy_path, serde_json::to_vec(&policy).unwrap()).unwrap();

    Command::new(env!("CARGO_BIN_EXE_restrict_bench_compare"))
        .args(["--candidate"])
        .arg(candidate_path)
        .args(["--baseline"])
        .arg(baseline_path)
        .args(["--policy"])
        .arg(policy_path)
        .output()
        .expect("benchmark comparator should run")
}

#[test]
fn deterministic_baseline_accepts_matching_report_with_informational_timing() {
    let output = run_compare(
        candidate_report(1_000, "release-sha"),
        baseline("informational"),
        policy("informational"),
    );
    assert!(
        output.status.success(),
        "matching deterministic report should pass: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn deterministic_baseline_rejects_artifact_hash_drift() {
    let output = run_compare(
        candidate_report(100, "changed-release-sha"),
        baseline("informational"),
        policy("informational"),
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("release artifact SHA-256"),
        "failure should identify the drifted artifact: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn controlled_timing_policy_rejects_median_regression() {
    let output = run_compare(
        candidate_report(116, "release-sha"),
        baseline("controlled"),
        policy("enforced"),
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("execution median regressed"),
        "failure should identify the timing regression: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn benchmark_policy_rejects_unknown_baseline_timing_status() {
    let output = run_compare(
        candidate_report(100, "release-sha"),
        baseline("unstable"),
        policy("informational"),
    );
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("unsupported baseline timingStatus"),
        "failure should identify the invalid baseline timing status: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn checked_in_benchmark_contract_matches_manifest() {
    let manifest = read_repository_json("benchmarks/manifest.json");
    let baseline = read_repository_json("benchmarks/baselines/core-wasm-v0.0.1.json");
    let policy = read_repository_json("benchmarks/regression-policy.json");

    assert_eq!(manifest["schemaVersion"], 1);
    assert_eq!(baseline["schemaVersion"], 1);
    assert_eq!(baseline["reportSchemaVersion"], 4);
    assert_eq!(policy["schemaVersion"], 1);
    assert_eq!(
        workload_ids(&manifest),
        workload_ids(&baseline),
        "checked-in baseline workloads should exactly match the benchmark manifest"
    );
    assert_eq!(baseline["target"], manifest["target"]);
    assert_eq!(policy["requireCleanCandidate"], true);
    assert_eq!(policy["requireExactToolchain"], true);
    assert_eq!(policy["requireExactArtifactHashes"], true);
    assert_eq!(policy["requireExactMemory"], true);
    assert_eq!(policy["timing"]["status"], "informational");
    assert_eq!(baseline["provenance"]["timingStatus"], "informational");
}

#[test]
fn benchmark_evidence_workflow_pins_the_software_environment() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workflow = fs::read_to_string(root.join(".github/workflows/benchmark.yml"))
        .expect("controlled benchmark workflow should be readable");

    for required in [
        "runs-on: ubuntu-24.04",
        "uses: dtolnay/rust-toolchain@1.94.1",
        "for run in 1 2 3 4 5; do",
        "./target/release/restrict_bench_compare",
        "retention-days: 90",
        "Timing remains informational",
    ] {
        assert!(
            workflow.contains(required),
            "benchmark evidence workflow should contain `{required}`"
        );
    }
}
