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

fn stability_policy(status: &str) -> Value {
    json!({
        "schemaVersion": 1,
        "status": status,
        "runnerClass": "test-runner",
        "requiredReportSchemaVersion": 4,
        "requiredTarget": "wasm-core",
        "requiredMode": "full",
        "minimumReports": 5,
        "requireCleanReports": true,
        "requireExactSourceRevision": true,
        "requireExactHost": true,
        "requireExactToolchain": true,
        "metrics": {
            "executionMedian": {
                "maximumRelativeMadPercent": 5.0,
                "maximumRelativeRangePercent": 15.0
            },
            "optimizedCompile": {
                "maximumRelativeMadPercent": 10.0,
                "maximumRelativeRangePercent": 30.0
            },
            "runtimeCompile": {
                "maximumRelativeMadPercent": 10.0,
                "maximumRelativeRangePercent": 40.0
            },
            "coldInstantiation": {
                "maximumRelativeMadPercent": 10.0,
                "maximumRelativeRangePercent": 40.0
            }
        }
    })
}

fn stability_report(
    execution_median_ns: u64,
    optimized_compile_ns: u64,
    runtime_compile_ns: u64,
    cold_instantiation_ns: u64,
) -> Value {
    let mut report = candidate_report(execution_median_ns, "release-sha");
    report["workloads"][0]["optimizedArtifact"]["compileNs"] = json!(optimized_compile_ns);
    report["workloads"][0]["runtimeCompileNs"] = json!(runtime_compile_ns);
    report["workloads"][0]["coldInstantiationNs"] = json!(cold_instantiation_ns);
    report
}

fn run_stability(reports: Vec<Value>, policy: Value) -> (Output, Option<Value>) {
    let directory = tempdir().expect("temporary stability evidence directory should be created");
    let policy_path = directory.path().join("policy.json");
    let summary_path = directory.path().join("summary.json");
    fs::write(&policy_path, serde_json::to_vec(&policy).unwrap()).unwrap();

    let mut report_paths = Vec::new();
    for (index, report) in reports.into_iter().enumerate() {
        let path = directory.path().join(format!("report-{}.json", index + 1));
        fs::write(&path, serde_json::to_vec(&report).unwrap()).unwrap();
        report_paths.push(path);
    }

    let output = Command::new(env!("CARGO_BIN_EXE_restrict_bench_stability"))
        .args(["--policy"])
        .arg(policy_path)
        .args(["--output"])
        .arg(&summary_path)
        .args(&report_paths)
        .output()
        .expect("benchmark stability assessor should run");
    let summary = fs::read_to_string(summary_path)
        .ok()
        .map(|contents| serde_json::from_str(&contents).unwrap());
    (output, summary)
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
fn informational_stability_policy_records_passing_evidence_without_promotion() {
    let reports = [98, 99, 100, 101, 102]
        .into_iter()
        .map(|sample| stability_report(sample, sample, sample, sample))
        .collect();
    let (output, summary) = run_stability(reports, stability_policy("informational"));
    assert!(
        output.status.success(),
        "stable informational evidence should succeed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary = summary.expect("stability summary should be written");
    assert_eq!(summary["thresholdsPassed"], true);
    assert_eq!(summary["promotionEligible"], false);
    assert_eq!(summary["reportCount"], 5);
}

#[test]
fn informational_stability_policy_preserves_unstable_evidence() {
    let reports = [100, 100, 100, 100, 200]
        .into_iter()
        .map(|sample| stability_report(sample, 100, 100, 100))
        .collect();
    let (output, summary) = run_stability(reports, stability_policy("informational"));
    assert!(
        output.status.success(),
        "informational instability should be recorded without failing the workflow: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let summary = summary.expect("stability summary should be written");
    assert_eq!(summary["thresholdsPassed"], false);
    assert_eq!(summary["promotionEligible"], false);
    assert!(summary["violations"][0]
        .as_str()
        .unwrap()
        .contains("execution median"));
}

#[test]
fn enforced_stability_policy_rejects_unstable_evidence() {
    let reports = [100, 100, 100, 100, 200]
        .into_iter()
        .map(|sample| stability_report(sample, 100, 100, 100))
        .collect();
    let (output, summary) = run_stability(reports, stability_policy("enforced"));
    assert!(!output.status.success());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("benchmark stability policy failed"),
        "enforced instability should identify the policy failure: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert_eq!(summary.unwrap()["promotionEligible"], false);
}

#[test]
fn stability_policy_rejects_mixed_hosts() {
    let mut reports = [100, 100, 100, 100, 100]
        .into_iter()
        .map(|sample| stability_report(sample, 100, 100, 100))
        .collect::<Vec<_>>();
    reports[4]["host"]["cpu"] = json!("Different CPU");
    let (output, summary) = run_stability(reports, stability_policy("informational"));
    assert!(!output.status.success());
    assert!(summary.is_none());
    assert!(
        String::from_utf8_lossy(&output.stderr).contains("host differs"),
        "mixed-host evidence should identify the mismatched host: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

#[test]
fn checked_in_benchmark_contract_matches_manifest() {
    let manifest = read_repository_json("benchmarks/manifest.json");
    let baseline = read_repository_json("benchmarks/baselines/core-wasm-v0.0.1.json");
    let policy = read_repository_json("benchmarks/regression-policy.json");
    let stability_policy = read_repository_json("benchmarks/stability-policy.json");

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
    assert_eq!(stability_policy["schemaVersion"], 1);
    assert_eq!(stability_policy["status"], "informational");
    assert_eq!(stability_policy["runnerClass"], "local-session");
    assert_eq!(stability_policy["minimumReports"], 5);
}

#[test]
fn benchmark_timing_policy_is_local_only() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    assert!(
        !root.join(".github/workflows/benchmark.yml").exists(),
        "timing evidence should not depend on a hosted or self-hosted workflow"
    );
    let guide = fs::read_to_string(root.join("docs/en/tools/benchmarking.md"))
        .expect("benchmarking guide should be readable");

    for required in [
        "same-session local run",
        "Interleave the language order",
        "Keep every raw sample",
        "rerun all comparison targets",
        "does not become a persistent regression gate",
    ] {
        assert!(
            guide.contains(required),
            "benchmarking guide should contain `{required}`"
        );
    }
}

#[test]
fn benchmark_evidence_scripts_share_one_recording_path() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let local_script = fs::read_to_string(root.join("scripts/run-benchmark-evidence.sh"))
        .expect("local benchmark evidence script should be readable");
    let recording_script = fs::read_to_string(root.join("scripts/record-benchmark-evidence.sh"))
        .expect("shared benchmark recording script should be readable");

    assert!(local_script.contains("bash scripts/record-benchmark-evidence.sh"));
    for required in [
        "./target/release/restrict_bench --output",
        "./target/release/restrict_bench_compare",
        "./target/release/restrict_bench_stability",
        "benchmarks/regression-policy.json",
    ] {
        assert!(
            recording_script.contains(required),
            "shared benchmark recording script should contain `{required}`"
        );
    }
}
