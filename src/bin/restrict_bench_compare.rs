use anyhow::{bail, Context, Result};
use serde::Deserialize;
use std::collections::HashMap;
use std::env;
use std::fmt::Debug;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct HostMetadata {
    os: String,
    architecture: String,
    cpu: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ToolMetadata {
    compiler: String,
    runner: String,
    validator: String,
    encoder: String,
    optimizer: String,
    compression: String,
    rustc: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CandidateReport {
    schema_version: u32,
    source_dirty: bool,
    target: String,
    mode: String,
    host: HostMetadata,
    tools: ToolMetadata,
    workloads: Vec<CandidateWorkload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CandidateWorkload {
    id: String,
    source: String,
    source_sha256: String,
    export: String,
    input: i32,
    expected: i32,
    arena_bytes: u32,
    raw_artifact: CandidateArtifact,
    optimized_artifact: CandidateArtifact,
    instrumented_artifact: CandidateArtifact,
    release_reproducible: bool,
    memory: MemoryMetrics,
    runtime_compile_ns: u64,
    cold_instantiation_ns: u64,
    warmup_iterations: usize,
    iterations: usize,
    execution: TimingSummary,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CandidateArtifact {
    wasm_sha256: String,
    compile_ns: u64,
    wasm_bytes: u64,
    zstd_bytes: u64,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct MemoryMetrics {
    peak_bytes: u32,
    allocation_count: u32,
    reset_count: u32,
    live_bytes_after_call: u32,
    verified_iterations: u32,
    exhaustion: Option<ExhaustionResult>,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct ExhaustionResult {
    arena_bytes: u32,
    error_code: u32,
    requested_bytes: u32,
    trapped: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimingSummary {
    median_ns: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkBaseline {
    schema_version: u32,
    report_schema_version: u32,
    target: String,
    mode: String,
    toolchain: ToolMetadata,
    provenance: BaselineProvenance,
    workloads: Vec<BaselineWorkload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BaselineProvenance {
    timing_host: HostMetadata,
    timing_status: String,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BaselineWorkload {
    id: String,
    source: String,
    source_sha256: String,
    export: String,
    input: i32,
    expected: i32,
    arena_bytes: u32,
    raw_artifact: BaselineArtifact,
    optimized_artifact: BaselineArtifact,
    instrumented_artifact: BaselineArtifact,
    release_reproducible: bool,
    memory: MemoryMetrics,
    warmup_iterations: usize,
    iterations: usize,
    timing_reference: TimingReference,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BaselineArtifact {
    wasm_sha256: String,
    wasm_bytes: u64,
    zstd_bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimingReference {
    optimized_compile_ns: u64,
    runtime_compile_ns: u64,
    cold_instantiation_ns: u64,
    median_ns: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct RegressionPolicy {
    schema_version: u32,
    require_clean_candidate: bool,
    require_exact_toolchain: bool,
    require_exact_sources_and_inputs: bool,
    require_exact_artifact_hashes: bool,
    maximum_wasm_growth_bytes: u64,
    maximum_compressed_growth_bytes: u64,
    require_exact_memory: bool,
    timing: TimingPolicy,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct TimingPolicy {
    status: String,
    require_exact_host: bool,
    minimum_iterations: usize,
    maximum_median_regression_percent: f64,
    maximum_optimized_compile_regression_percent: f64,
    maximum_runtime_compile_regression_percent: f64,
    maximum_cold_instantiation_regression_percent: f64,
}

struct Args {
    baseline: PathBuf,
    candidate: PathBuf,
    policy: PathBuf,
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let baseline: BenchmarkBaseline = read_json(&args.baseline)?;
    let candidate: CandidateReport = read_json(&args.candidate)?;
    let policy: RegressionPolicy = read_json(&args.policy)?;
    compare(&baseline, &candidate, &policy)?;
    println!(
        "benchmark regression check passed: {} workloads; timing {}",
        candidate.workloads.len(),
        policy.timing.status
    );
    Ok(())
}

fn parse_args() -> Result<Args> {
    let mut baseline = None;
    let mut candidate = None;
    let mut policy = None;
    let raw = env::args().skip(1).collect::<Vec<_>>();
    let mut index = 0;
    while index < raw.len() {
        let destination = match raw[index].as_str() {
            "--baseline" => &mut baseline,
            "--candidate" => &mut candidate,
            "--policy" => &mut policy,
            "--help" => {
                println!(
                    "Usage: restrict_bench_compare --baseline PATH --candidate PATH --policy PATH"
                );
                std::process::exit(0);
            }
            unknown => bail!("unknown argument '{unknown}'"),
        };
        index += 1;
        *destination =
            Some(PathBuf::from(raw.get(index).with_context(|| {
                format!("{} needs a path", raw[index - 1])
            })?));
        index += 1;
    }
    Ok(Args {
        baseline: baseline.context("--baseline is required")?,
        candidate: candidate.context("--candidate is required")?,
        policy: policy.context("--policy is required")?,
    })
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn compare(
    baseline: &BenchmarkBaseline,
    candidate: &CandidateReport,
    policy: &RegressionPolicy,
) -> Result<()> {
    if baseline.schema_version != 1 {
        bail!(
            "unsupported benchmark baseline schema {}; expected 1",
            baseline.schema_version
        );
    }
    if policy.schema_version != 1 {
        bail!(
            "unsupported benchmark policy schema {}; expected 1",
            policy.schema_version
        );
    }

    let mut errors = Vec::new();
    require_equal(
        &mut errors,
        "candidate report schema",
        &baseline.report_schema_version,
        &candidate.schema_version,
    );
    require_equal(
        &mut errors,
        "benchmark target",
        &baseline.target,
        &candidate.target,
    );
    require_equal(
        &mut errors,
        "benchmark mode",
        &baseline.mode,
        &candidate.mode,
    );
    if policy.require_clean_candidate && candidate.source_dirty {
        errors.push("candidate report was generated from a dirty worktree".to_string());
    }
    if policy.require_exact_toolchain {
        require_equal(
            &mut errors,
            "benchmark toolchain",
            &baseline.toolchain,
            &candidate.tools,
        );
    }
    match baseline.provenance.timing_status.as_str() {
        "informational" | "controlled" => {}
        status => errors.push(format!(
            "unsupported baseline timingStatus '{status}'; expected 'informational' or 'controlled'"
        )),
    }

    let mut candidates = HashMap::new();
    for workload in &candidate.workloads {
        if candidates.insert(workload.id.as_str(), workload).is_some() {
            errors.push(format!(
                "candidate report contains duplicate workload '{}'",
                workload.id
            ));
        }
    }

    for expected in &baseline.workloads {
        let Some(actual) = candidates.remove(expected.id.as_str()) else {
            errors.push(format!(
                "candidate report is missing workload '{}'",
                expected.id
            ));
            continue;
        };
        compare_workload(&mut errors, expected, actual, policy);
    }
    for id in candidates.keys() {
        errors.push(format!(
            "candidate report contains unexpected workload '{id}'"
        ));
    }

    match policy.timing.status.as_str() {
        "informational" => {}
        "enforced" => {
            if baseline.provenance.timing_status != "controlled" {
                errors.push(
                    "timing policy is enforced but baseline timingStatus is not 'controlled'"
                        .to_string(),
                );
            }
            if policy.timing.require_exact_host {
                require_equal(
                    &mut errors,
                    "timing host",
                    &baseline.provenance.timing_host,
                    &candidate.host,
                );
            }
        }
        status => errors.push(format!(
            "unsupported timing policy status '{status}'; expected 'informational' or 'enforced'"
        )),
    }

    if errors.is_empty() {
        Ok(())
    } else {
        bail!(
            "benchmark regression check failed:\n- {}",
            errors.join("\n- ")
        )
    }
}

fn compare_workload(
    errors: &mut Vec<String>,
    baseline: &BaselineWorkload,
    candidate: &CandidateWorkload,
    policy: &RegressionPolicy,
) {
    let prefix = format!("workload '{}'", baseline.id);
    if policy.require_exact_sources_and_inputs {
        require_equal(
            errors,
            &format!("{prefix} source"),
            &baseline.source,
            &candidate.source,
        );
        require_equal(
            errors,
            &format!("{prefix} source SHA-256"),
            &baseline.source_sha256,
            &candidate.source_sha256,
        );
        require_equal(
            errors,
            &format!("{prefix} export"),
            &baseline.export,
            &candidate.export,
        );
        require_equal(
            errors,
            &format!("{prefix} input"),
            &baseline.input,
            &candidate.input,
        );
        require_equal(
            errors,
            &format!("{prefix} checksum"),
            &baseline.expected,
            &candidate.expected,
        );
        require_equal(
            errors,
            &format!("{prefix} arena capacity"),
            &baseline.arena_bytes,
            &candidate.arena_bytes,
        );
        require_equal(
            errors,
            &format!("{prefix} warm-up iterations"),
            &baseline.warmup_iterations,
            &candidate.warmup_iterations,
        );
        require_equal(
            errors,
            &format!("{prefix} measured iterations"),
            &baseline.iterations,
            &candidate.iterations,
        );
    }
    if !baseline.release_reproducible || !candidate.release_reproducible {
        errors.push(format!("{prefix} release artifact is not reproducible"));
    }

    compare_artifact(
        errors,
        &prefix,
        "raw",
        &baseline.raw_artifact,
        &candidate.raw_artifact,
        policy,
    );
    compare_artifact(
        errors,
        &prefix,
        "release",
        &baseline.optimized_artifact,
        &candidate.optimized_artifact,
        policy,
    );
    compare_artifact(
        errors,
        &prefix,
        "instrumented",
        &baseline.instrumented_artifact,
        &candidate.instrumented_artifact,
        policy,
    );
    if policy.require_exact_memory {
        require_equal(
            errors,
            &format!("{prefix} memory metrics"),
            &baseline.memory,
            &candidate.memory,
        );
    }

    if policy.timing.status == "enforced" {
        if candidate.iterations < policy.timing.minimum_iterations {
            errors.push(format!(
                "{prefix} has {} timing iterations; policy requires at least {}",
                candidate.iterations, policy.timing.minimum_iterations
            ));
        }
        check_regression(
            errors,
            &format!("{prefix} execution median"),
            baseline.timing_reference.median_ns,
            candidate.execution.median_ns,
            policy.timing.maximum_median_regression_percent,
        );
        check_regression(
            errors,
            &format!("{prefix} optimized compile time"),
            baseline.timing_reference.optimized_compile_ns,
            candidate.optimized_artifact.compile_ns,
            policy.timing.maximum_optimized_compile_regression_percent,
        );
        check_regression(
            errors,
            &format!("{prefix} runtime compile time"),
            baseline.timing_reference.runtime_compile_ns,
            candidate.runtime_compile_ns,
            policy.timing.maximum_runtime_compile_regression_percent,
        );
        check_regression(
            errors,
            &format!("{prefix} cold instantiation time"),
            baseline.timing_reference.cold_instantiation_ns,
            candidate.cold_instantiation_ns,
            policy.timing.maximum_cold_instantiation_regression_percent,
        );
    }
}

fn compare_artifact(
    errors: &mut Vec<String>,
    workload: &str,
    label: &str,
    baseline: &BaselineArtifact,
    candidate: &CandidateArtifact,
    policy: &RegressionPolicy,
) {
    if policy.require_exact_artifact_hashes {
        require_equal(
            errors,
            &format!("{workload} {label} artifact SHA-256"),
            &baseline.wasm_sha256,
            &candidate.wasm_sha256,
        );
    }
    let maximum_wasm = baseline
        .wasm_bytes
        .saturating_add(policy.maximum_wasm_growth_bytes);
    if candidate.wasm_bytes > maximum_wasm {
        errors.push(format!(
            "{workload} {label} Wasm grew from {} to {} bytes; maximum is {}",
            baseline.wasm_bytes, candidate.wasm_bytes, maximum_wasm
        ));
    }
    let maximum_compressed = baseline
        .zstd_bytes
        .saturating_add(policy.maximum_compressed_growth_bytes);
    if candidate.zstd_bytes > maximum_compressed {
        errors.push(format!(
            "{workload} {label} compressed Wasm grew from {} to {} bytes; maximum is {}",
            baseline.zstd_bytes, candidate.zstd_bytes, maximum_compressed
        ));
    }
}

fn check_regression(
    errors: &mut Vec<String>,
    label: &str,
    baseline: u64,
    candidate: u64,
    maximum_percent: f64,
) {
    let maximum = baseline as f64 * (1.0 + maximum_percent / 100.0);
    if candidate as f64 > maximum {
        let regression = if baseline == 0 {
            f64::INFINITY
        } else {
            (candidate as f64 / baseline as f64 - 1.0) * 100.0
        };
        errors.push(format!(
            "{label} regressed from {baseline} ns to {candidate} ns ({regression:.2}%); policy allows {maximum_percent:.2}%"
        ));
    }
}

fn require_equal<T: PartialEq + Debug>(
    errors: &mut Vec<String>,
    label: &str,
    expected: &T,
    actual: &T,
) {
    if expected != actual {
        errors.push(format!(
            "{label} differs: expected {expected:?}, found {actual:?}"
        ));
    }
}
