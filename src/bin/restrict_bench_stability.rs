use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
struct HostMetadata {
    os: String,
    architecture: String,
    cpu: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
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
struct BenchmarkReport {
    schema_version: u32,
    source_revision: String,
    source_dirty: bool,
    target: String,
    mode: String,
    host: HostMetadata,
    tools: ToolMetadata,
    workloads: Vec<ReportWorkload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ReportWorkload {
    id: String,
    optimized_artifact: ArtifactTiming,
    runtime_compile_ns: u64,
    cold_instantiation_ns: u64,
    execution: ExecutionTiming,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactTiming {
    compile_ns: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExecutionTiming {
    median_ns: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StabilityPolicy {
    schema_version: u32,
    status: String,
    runner_class: String,
    required_report_schema_version: u32,
    required_target: String,
    required_mode: String,
    minimum_reports: usize,
    require_clean_reports: bool,
    require_exact_source_revision: bool,
    require_exact_host: bool,
    require_exact_toolchain: bool,
    metrics: MetricPolicies,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MetricPolicies {
    execution_median: MetricPolicy,
    optimized_compile: MetricPolicy,
    runtime_compile: MetricPolicy,
    cold_instantiation: MetricPolicy,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct MetricPolicy {
    maximum_relative_mad_percent: f64,
    maximum_relative_range_percent: f64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct StabilitySummary {
    schema_version: u32,
    policy_status: String,
    runner_class: String,
    report_count: usize,
    source_revision: String,
    target: String,
    mode: String,
    host: HostMetadata,
    tools: ToolMetadata,
    thresholds_passed: bool,
    promotion_eligible: bool,
    violations: Vec<String>,
    workloads: Vec<WorkloadSummary>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkloadSummary {
    id: String,
    execution_median: MetricSummary,
    optimized_compile: MetricSummary,
    runtime_compile: MetricSummary,
    cold_instantiation: MetricSummary,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MetricSummary {
    samples_ns: Vec<u64>,
    minimum_ns: u64,
    median_ns: f64,
    maximum_ns: u64,
    relative_mad_percent: f64,
    relative_range_percent: f64,
    policy: MetricPolicy,
    passed: bool,
}

struct Args {
    policy: PathBuf,
    output: PathBuf,
    reports: Vec<PathBuf>,
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let policy: StabilityPolicy = read_json(&args.policy)?;
    validate_policy(&policy)?;
    let reports = args
        .reports
        .iter()
        .map(|path| read_json(path))
        .collect::<Result<Vec<BenchmarkReport>>>()?;
    validate_reports(&reports, &policy)?;
    let summary = summarize(&reports, &policy);
    write_json(&args.output, &summary)?;

    println!(
        "benchmark stability evidence: {} reports; thresholds {}; policy {}",
        summary.report_count,
        if summary.thresholds_passed {
            "passed"
        } else {
            "failed"
        },
        summary.policy_status
    );
    println!("stability summary: {}", args.output.display());

    if policy.status == "enforced" && !summary.thresholds_passed {
        bail!(
            "benchmark stability policy failed:\n- {}",
            summary.violations.join("\n- ")
        );
    }
    Ok(())
}

fn parse_args() -> Result<Args> {
    let mut policy = None;
    let mut output = None;
    let mut reports = Vec::new();
    let raw = env::args().skip(1).collect::<Vec<_>>();
    let mut index = 0;
    while index < raw.len() {
        match raw[index].as_str() {
            "--policy" => {
                index += 1;
                policy = Some(PathBuf::from(
                    raw.get(index).context("--policy needs a path")?,
                ));
            }
            "--output" => {
                index += 1;
                output = Some(PathBuf::from(
                    raw.get(index).context("--output needs a path")?,
                ));
            }
            "--help" => {
                println!(
                    "Usage: restrict_bench_stability --policy PATH --output PATH REPORT.json..."
                );
                std::process::exit(0);
            }
            unknown if unknown.starts_with('-') => bail!("unknown argument '{unknown}'"),
            path => reports.push(PathBuf::from(path)),
        }
        index += 1;
    }
    if reports.is_empty() {
        bail!("at least one benchmark report path is required");
    }
    Ok(Args {
        policy: policy.context("--policy is required")?,
        output: output.context("--output is required")?,
        reports,
    })
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Path) -> Result<T> {
    let text =
        fs::read_to_string(path).with_context(|| format!("failed to read {}", path.display()))?;
    serde_json::from_str(&text).with_context(|| format!("failed to parse {}", path.display()))
}

fn write_json(path: &Path, summary: &StabilitySummary) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(summary)?)
        .with_context(|| format!("failed to write {}", path.display()))
}

fn validate_policy(policy: &StabilityPolicy) -> Result<()> {
    if policy.schema_version != 1 {
        bail!(
            "unsupported stability policy schema {}; expected 1",
            policy.schema_version
        );
    }
    if !matches!(policy.status.as_str(), "informational" | "enforced") {
        bail!(
            "unsupported stability policy status '{}'; expected 'informational' or 'enforced'",
            policy.status
        );
    }
    if policy.minimum_reports < 3 {
        bail!("stability policy must require at least three reports");
    }
    if policy.runner_class.trim().is_empty() {
        bail!("stability policy runnerClass must not be empty");
    }
    for (name, metric) in [
        ("executionMedian", &policy.metrics.execution_median),
        ("optimizedCompile", &policy.metrics.optimized_compile),
        ("runtimeCompile", &policy.metrics.runtime_compile),
        ("coldInstantiation", &policy.metrics.cold_instantiation),
    ] {
        if !metric.maximum_relative_mad_percent.is_finite()
            || metric.maximum_relative_mad_percent < 0.0
            || !metric.maximum_relative_range_percent.is_finite()
            || metric.maximum_relative_range_percent < 0.0
        {
            bail!("stability policy metric '{name}' thresholds must be finite and non-negative");
        }
    }
    Ok(())
}

fn validate_reports(reports: &[BenchmarkReport], policy: &StabilityPolicy) -> Result<()> {
    if reports.len() < policy.minimum_reports {
        bail!(
            "stability policy requires at least {} reports; found {}",
            policy.minimum_reports,
            reports.len()
        );
    }
    let reference = reports
        .first()
        .context("at least one benchmark report is required")?;
    let expected_ids = workload_ids(reference)?;
    let mut errors = Vec::new();

    for (index, report) in reports.iter().enumerate() {
        let label = format!("report {}", index + 1);
        if report.schema_version != policy.required_report_schema_version {
            errors.push(format!(
                "{label} schema is {}; expected {}",
                report.schema_version, policy.required_report_schema_version
            ));
        }
        if report.target != policy.required_target {
            errors.push(format!(
                "{label} target is {:?}; expected {:?}",
                report.target, policy.required_target
            ));
        }
        if report.mode != policy.required_mode {
            errors.push(format!(
                "{label} mode is {:?}; expected {:?}",
                report.mode, policy.required_mode
            ));
        }
        if policy.require_clean_reports && report.source_dirty {
            errors.push(format!("{label} was generated from a dirty worktree"));
        }
        if policy.require_exact_source_revision
            && report.source_revision != reference.source_revision
        {
            errors.push(format!(
                "{label} source revision differs: expected {:?}, found {:?}",
                reference.source_revision, report.source_revision
            ));
        }
        if policy.require_exact_host && report.host != reference.host {
            errors.push(format!(
                "{label} host differs: expected {:?}, found {:?}",
                reference.host, report.host
            ));
        }
        if policy.require_exact_toolchain && report.tools != reference.tools {
            errors.push(format!(
                "{label} toolchain differs: expected {:?}, found {:?}",
                reference.tools, report.tools
            ));
        }
        match workload_ids(report) {
            Ok(ids) if ids == expected_ids => {}
            Ok(ids) => errors.push(format!(
                "{label} workload ids differ: expected {expected_ids:?}, found {ids:?}"
            )),
            Err(error) => errors.push(format!("{label} {error:#}")),
        }
    }

    if policy.require_exact_host && reference.host.cpu == "unknown" {
        errors.push(
            "reference report CPU is unknown; exact-host evidence is not attributable".into(),
        );
    }
    if errors.is_empty() {
        Ok(())
    } else {
        bail!("invalid benchmark evidence:\n- {}", errors.join("\n- "))
    }
}

fn workload_ids(report: &BenchmarkReport) -> Result<BTreeSet<String>> {
    let mut ids = BTreeSet::new();
    for workload in &report.workloads {
        if !ids.insert(workload.id.clone()) {
            bail!("contains duplicate workload '{}'", workload.id);
        }
    }
    if ids.is_empty() {
        bail!("contains no workloads");
    }
    Ok(ids)
}

fn summarize(reports: &[BenchmarkReport], policy: &StabilityPolicy) -> StabilitySummary {
    let reference = &reports[0];
    let ids = workload_ids(reference).expect("validated reports should contain workloads");
    let mut violations = Vec::new();
    let mut workloads = Vec::with_capacity(ids.len());

    for id in ids {
        let mut samples = MetricSamples::default();
        for report in reports {
            let workload = report
                .workloads
                .iter()
                .find(|workload| workload.id == id)
                .expect("validated reports should contain identical workloads");
            samples.execution_median.push(workload.execution.median_ns);
            samples
                .optimized_compile
                .push(workload.optimized_artifact.compile_ns);
            samples.runtime_compile.push(workload.runtime_compile_ns);
            samples
                .cold_instantiation
                .push(workload.cold_instantiation_ns);
        }
        workloads.push(WorkloadSummary {
            id: id.clone(),
            execution_median: summarize_metric(
                &format!("workload '{id}' execution median"),
                samples.execution_median,
                &policy.metrics.execution_median,
                &mut violations,
            ),
            optimized_compile: summarize_metric(
                &format!("workload '{id}' optimized compile"),
                samples.optimized_compile,
                &policy.metrics.optimized_compile,
                &mut violations,
            ),
            runtime_compile: summarize_metric(
                &format!("workload '{id}' runtime compile"),
                samples.runtime_compile,
                &policy.metrics.runtime_compile,
                &mut violations,
            ),
            cold_instantiation: summarize_metric(
                &format!("workload '{id}' cold instantiation"),
                samples.cold_instantiation,
                &policy.metrics.cold_instantiation,
                &mut violations,
            ),
        });
    }

    let thresholds_passed = violations.is_empty();
    StabilitySummary {
        schema_version: 1,
        policy_status: policy.status.clone(),
        runner_class: policy.runner_class.clone(),
        report_count: reports.len(),
        source_revision: reference.source_revision.clone(),
        target: reference.target.clone(),
        mode: reference.mode.clone(),
        host: reference.host.clone(),
        tools: reference.tools.clone(),
        thresholds_passed,
        promotion_eligible: policy.status == "enforced" && thresholds_passed,
        violations,
        workloads,
    }
}

#[derive(Default)]
struct MetricSamples {
    execution_median: Vec<u64>,
    optimized_compile: Vec<u64>,
    runtime_compile: Vec<u64>,
    cold_instantiation: Vec<u64>,
}

fn summarize_metric(
    label: &str,
    samples: Vec<u64>,
    policy: &MetricPolicy,
    violations: &mut Vec<String>,
) -> MetricSummary {
    let mut sorted = samples.clone();
    sorted.sort_unstable();
    let minimum_ns = sorted[0];
    let maximum_ns = sorted[sorted.len() - 1];
    let median_ns = median_u64(&sorted);
    let mut deviations = sorted
        .iter()
        .map(|sample| (*sample as f64 - median_ns).abs())
        .collect::<Vec<_>>();
    deviations.sort_by(f64::total_cmp);
    let mad_ns = median_f64(&deviations);
    let relative_mad_percent = relative_percent(mad_ns, median_ns);
    let relative_range_percent = relative_percent((maximum_ns - minimum_ns) as f64, median_ns);
    let passed = relative_mad_percent <= policy.maximum_relative_mad_percent
        && relative_range_percent <= policy.maximum_relative_range_percent;
    if !passed {
        violations.push(format!(
            "{label} varied by MAD {relative_mad_percent:.2}% and range {relative_range_percent:.2}%; limits are {:.2}% and {:.2}%",
            policy.maximum_relative_mad_percent, policy.maximum_relative_range_percent
        ));
    }
    MetricSummary {
        samples_ns: samples,
        minimum_ns,
        median_ns,
        maximum_ns,
        relative_mad_percent,
        relative_range_percent,
        policy: policy.clone(),
        passed,
    }
}

fn median_u64(sorted: &[u64]) -> f64 {
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] as f64 + sorted[middle] as f64) / 2.0
    } else {
        sorted[middle] as f64
    }
}

fn median_f64(sorted: &[f64]) -> f64 {
    let middle = sorted.len() / 2;
    if sorted.len().is_multiple_of(2) {
        (sorted[middle - 1] + sorted[middle]) / 2.0
    } else {
        sorted[middle]
    }
}

fn relative_percent(numerator: f64, denominator: f64) -> f64 {
    if denominator == 0.0 {
        if numerator == 0.0 {
            0.0
        } else {
            f64::MAX
        }
    } else {
        numerator / denominator * 100.0
    }
}
