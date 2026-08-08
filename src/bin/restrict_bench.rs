use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use wasmi::{Engine, Instance, Linker, Module, Store, Val};
use wasmparser::{Parser, Payload, Validator};

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkManifest {
    schema_version: u32,
    target: String,
    workloads: Vec<Workload>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Workload {
    id: String,
    source: String,
    export: String,
    input: i32,
    expected: i32,
    arena_bytes: u32,
    warmup_iterations: usize,
    iterations: usize,
    #[serde(default)]
    memory_probe: Option<MemoryProbeConfig>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct MemoryProbeConfig {
    exhaustion_arena_bytes: u32,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BenchmarkReport {
    schema_version: u32,
    generated_at_unix_seconds: u64,
    source_revision: String,
    source_dirty: bool,
    target: String,
    mode: String,
    host: HostMetadata,
    tools: ToolMetadata,
    workloads: Vec<WorkloadResult>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct HostMetadata {
    os: String,
    architecture: String,
    cpu: String,
}

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkloadResult {
    id: String,
    source: String,
    source_sha256: String,
    export: String,
    input: i32,
    expected: i32,
    arena_bytes: u32,
    raw_artifact: ArtifactResult,
    optimized_artifact: ArtifactResult,
    instrumented_artifact: ArtifactResult,
    release_reproducible: bool,
    memory: MemoryMetrics,
    runtime_compile_ns: u64,
    cold_instantiation_ns: u64,
    warmup_iterations: usize,
    iterations: usize,
    execution: TimingSummary,
    execution_samples_ns: Vec<u64>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactResult {
    wasm_sha256: String,
    compile_ns: u64,
    wasm_bytes: usize,
    zstd_bytes: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct MemoryMetrics {
    peak_bytes: u32,
    allocation_count: u32,
    reset_count: u32,
    live_bytes_after_call: u32,
    verified_iterations: u32,
    exhaustion: Option<ExhaustionResult>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct ExhaustionResult {
    arena_bytes: u32,
    error_code: u32,
    requested_bytes: u32,
    trapped: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct TimingSummary {
    min_ns: u64,
    median_ns: u64,
    mean_ns: f64,
    max_ns: u64,
}

struct Args {
    manifest: PathBuf,
    compiler: PathBuf,
    output: PathBuf,
    smoke: bool,
}

fn main() -> Result<()> {
    let args = parse_args()?;
    let manifest_text = fs::read_to_string(&args.manifest)
        .with_context(|| format!("failed to read {}", args.manifest.display()))?;
    let manifest: BenchmarkManifest = serde_json::from_str(&manifest_text)
        .with_context(|| format!("failed to parse {}", args.manifest.display()))?;
    if manifest.schema_version != 1 {
        bail!(
            "unsupported benchmark manifest schema {}; expected 1",
            manifest.schema_version
        );
    }
    if manifest.target != "wasm-core" {
        bail!("benchmark manifest target must be 'wasm-core'");
    }

    let manifest_dir = args
        .manifest
        .parent()
        .context("benchmark manifest must have a parent directory")?;
    let repo_root = manifest_dir
        .parent()
        .context("benchmarks directory must be inside the repository")?;
    let artifact_dir = args
        .output
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .join("artifacts");
    fs::create_dir_all(&artifact_dir)?;

    let mut results = Vec::with_capacity(manifest.workloads.len());
    for workload in &manifest.workloads {
        results.push(run_workload(
            workload,
            manifest_dir,
            &artifact_dir,
            &args.compiler,
            args.smoke,
        )?);
    }

    let report = BenchmarkReport {
        schema_version: 4,
        generated_at_unix_seconds: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        source_revision: command_stdout(
            Command::new("git")
                .arg("-C")
                .arg(repo_root)
                .args(["rev-parse", "HEAD"]),
        )
        .unwrap_or_else(|| "unknown".to_string()),
        source_dirty: command_stdout(Command::new("git").arg("-C").arg(repo_root).args([
            "status",
            "--porcelain=v1",
            "--untracked-files=normal",
        ]))
        .is_none_or(|status| !status.is_empty()),
        target: manifest.target,
        mode: if args.smoke { "smoke" } else { "full" }.to_string(),
        host: HostMetadata {
            os: env::consts::OS.to_string(),
            architecture: env::consts::ARCH.to_string(),
            cpu: detect_cpu(),
        },
        tools: ToolMetadata {
            compiler: command_stdout(Command::new(&args.compiler).arg("--version"))
                .unwrap_or_else(|| "unknown".to_string()),
            runner: format!("restrict_bench {} (wasmi 1.1.0)", env!("CARGO_PKG_VERSION")),
            validator: "wasmparser 0.252.0".to_string(),
            encoder: "wat 1.252.0".to_string(),
            optimizer: "restrict release reachability-dce v1 (no external wasm-opt)".to_string(),
            compression: "zstd 0.13.3 level 19".to_string(),
            rustc: command_stdout(Command::new("rustc").arg("--version"))
                .unwrap_or_else(|| "unknown".to_string()),
        },
        workloads: results,
    };

    if let Some(parent) = args.output.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&args.output, serde_json::to_vec_pretty(&report)?)?;
    println!("benchmark report: {}", args.output.display());
    for workload in &report.workloads {
        println!(
            "{}: median={} ns, raw={} bytes, release={} bytes, peak={} bytes, checksum={}",
            workload.id,
            workload.execution.median_ns,
            workload.raw_artifact.wasm_bytes,
            workload.optimized_artifact.wasm_bytes,
            workload.memory.peak_bytes,
            workload.expected
        );
    }
    Ok(())
}

fn parse_args() -> Result<Args> {
    let current_exe = env::current_exe()?;
    let sibling_compiler = current_exe
        .parent()
        .context("benchmark runner path has no parent")?
        .join("restrict_lang");
    let mut args = Args {
        manifest: PathBuf::from("benchmarks/manifest.json"),
        compiler: sibling_compiler,
        output: PathBuf::from("target/benchmark-results/restrict-baseline.json"),
        smoke: false,
    };

    let raw = env::args().skip(1).collect::<Vec<_>>();
    let mut index = 0;
    while index < raw.len() {
        match raw[index].as_str() {
            "--manifest" => {
                index += 1;
                args.manifest = PathBuf::from(raw.get(index).context("--manifest needs a path")?);
            }
            "--compiler" => {
                index += 1;
                args.compiler = PathBuf::from(raw.get(index).context("--compiler needs a path")?);
            }
            "--output" => {
                index += 1;
                args.output = PathBuf::from(raw.get(index).context("--output needs a path")?);
            }
            "--smoke" => args.smoke = true,
            "--help" => {
                println!(
                    "Usage: restrict_bench [--manifest PATH] [--compiler PATH] [--output PATH] [--smoke]"
                );
                std::process::exit(0);
            }
            unknown => bail!("unknown argument '{unknown}'"),
        }
        index += 1;
    }
    Ok(args)
}

fn run_workload(
    workload: &Workload,
    manifest_dir: &Path,
    artifact_dir: &Path,
    compiler: &Path,
    smoke: bool,
) -> Result<WorkloadResult> {
    let source_path = manifest_dir.join(&workload.source);
    let source = fs::read(&source_path)
        .with_context(|| format!("failed to read benchmark source {}", source_path.display()))?;
    let raw_wasm_path = artifact_dir.join(format!("{}.raw.wasm", workload.id));
    let optimized_wasm_path = artifact_dir.join(format!("{}.release.wasm", workload.id));
    let reproducibility_path = artifact_dir.join(format!("{}.release-repeat.wasm", workload.id));

    let raw_compile_ns = compile_artifact(
        compiler,
        workload,
        &source_path,
        &raw_wasm_path,
        workload.arena_bytes,
        false,
        false,
    )?;
    let optimized_compile_ns = compile_artifact(
        compiler,
        workload,
        &source_path,
        &optimized_wasm_path,
        workload.arena_bytes,
        true,
        false,
    )?;
    compile_artifact(
        compiler,
        workload,
        &source_path,
        &reproducibility_path,
        workload.arena_bytes,
        true,
        false,
    )?;

    let raw_wasm = fs::read(&raw_wasm_path)?;
    let wasm = fs::read(&optimized_wasm_path)?;
    let repeated_wasm = fs::read(&reproducibility_path)?;
    fs::remove_file(&reproducibility_path)?;
    if wasm != repeated_wasm {
        bail!(
            "{} release output is not reproducible for identical compiler options",
            workload.id
        );
    }
    for (label, artifact) in [("raw", raw_wasm.as_slice()), ("release", wasm.as_slice())] {
        Validator::new()
            .validate_all(artifact)
            .with_context(|| format!("{} emitted invalid {label} Wasm", workload.id))?;
        reject_imports(&format!("{} {label}", workload.id), artifact)?;
    }

    let engine = Engine::default();
    let runtime_compile_started = Instant::now();
    let module = Module::new(&engine, &wasm[..])?;
    let runtime_compile_ns = elapsed_ns(runtime_compile_started);
    let mut store = Store::new(&engine, ());
    let cold_started = Instant::now();
    let instance = Linker::new(&engine).instantiate_and_start(&mut store, &module)?;
    let cold_instantiation_ns = elapsed_ns(cold_started);
    let function = instance
        .get_typed_func::<i32, i32>(&store, &workload.export)
        .with_context(|| {
            format!(
                "{} does not export an (i32) -> i32 function named '{}'",
                workload.id, workload.export
            )
        })?;

    let warmup_iterations = if smoke {
        workload.warmup_iterations.min(1)
    } else {
        workload.warmup_iterations
    };
    let iterations = if smoke {
        workload.iterations.min(3)
    } else {
        workload.iterations
    };
    for _ in 0..warmup_iterations {
        verify_result(
            &workload.id,
            function.call(&mut store, workload.input)?,
            workload.expected,
        )?;
    }

    let mut samples = Vec::with_capacity(iterations);
    for _ in 0..iterations {
        let started = Instant::now();
        let actual = function.call(&mut store, workload.input)?;
        samples.push(elapsed_ns(started));
        verify_result(&workload.id, actual, workload.expected)?;
    }

    let (instrumented_artifact, memory) =
        measure_memory(compiler, workload, &source_path, artifact_dir)?;
    let raw_compressed = zstd::bulk::compress(&raw_wasm, 19)?;
    let optimized_compressed = zstd::bulk::compress(&wasm, 19)?;
    Ok(WorkloadResult {
        id: workload.id.clone(),
        source: workload.source.clone(),
        source_sha256: sha256(&source),
        export: workload.export.clone(),
        input: workload.input,
        expected: workload.expected,
        arena_bytes: workload.arena_bytes,
        raw_artifact: ArtifactResult {
            wasm_sha256: sha256(&raw_wasm),
            compile_ns: raw_compile_ns,
            wasm_bytes: raw_wasm.len(),
            zstd_bytes: raw_compressed.len(),
        },
        optimized_artifact: ArtifactResult {
            wasm_sha256: sha256(&wasm),
            compile_ns: optimized_compile_ns,
            wasm_bytes: wasm.len(),
            zstd_bytes: optimized_compressed.len(),
        },
        instrumented_artifact,
        release_reproducible: true,
        memory,
        runtime_compile_ns,
        cold_instantiation_ns,
        warmup_iterations,
        iterations,
        execution: summarize(&samples),
        execution_samples_ns: samples,
    })
}

fn compile_artifact(
    compiler: &Path,
    workload: &Workload,
    source_path: &Path,
    wasm_path: &Path,
    arena_bytes: u32,
    release: bool,
    instrument_memory: bool,
) -> Result<u64> {
    let mut command = Command::new(compiler);
    command.args(["--target", "wasm-core", "--emit", "wasm"]);
    if release {
        command.arg("--release");
    }
    if instrument_memory {
        command.arg("--instrument-memory");
    }
    command
        .args(["--arena-bytes", &arena_bytes.to_string()])
        .arg(source_path)
        .arg(wasm_path);
    let compile_started = Instant::now();
    let output = command
        .output()
        .with_context(|| format!("failed to run compiler {}", compiler.display()))?;
    let compile_ns = elapsed_ns(compile_started);
    if !output.status.success() {
        bail!(
            "{} failed to compile in {} mode:\n{}",
            workload.id,
            if release { "release" } else { "raw" },
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(compile_ns)
}

fn measure_memory(
    compiler: &Path,
    workload: &Workload,
    source_path: &Path,
    artifact_dir: &Path,
) -> Result<(ArtifactResult, MemoryMetrics)> {
    let wasm_path = artifact_dir.join(format!("{}.memory.wasm", workload.id));
    let compile_ns = compile_artifact(
        compiler,
        workload,
        source_path,
        &wasm_path,
        workload.arena_bytes,
        true,
        true,
    )?;
    let wasm = fs::read(&wasm_path)?;
    Validator::new()
        .validate_all(&wasm)
        .with_context(|| format!("{} emitted invalid memory-instrumented Wasm", workload.id))?;
    reject_imports(&format!("{} memory", workload.id), &wasm)?;

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm[..])?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate_and_start(&mut store, &module)?;
    let reset = instance
        .get_typed_func::<(), ()>(&store, "__restrict_memory_metrics_reset")
        .with_context(|| format!("{} is missing the memory metrics reset export", workload.id))?;
    let function = instance
        .get_typed_func::<i32, i32>(&store, &workload.export)
        .with_context(|| format!("{} is missing its benchmark export", workload.id))?;

    let mut observed = None;
    for _ in 0..2 {
        reset.call(&mut store, ())?;
        let actual = function.call(&mut store, workload.input)?;
        verify_result(&workload.id, actual, workload.expected)?;
        let current = (
            exported_u32_global(&instance, &store, "__restrict_arena_peak_bytes")?,
            exported_u32_global(&instance, &store, "__restrict_arena_allocation_count")?,
            exported_u32_global(&instance, &store, "__restrict_arena_reset_count")?,
            exported_u32_global(&instance, &store, "__restrict_arena_live_bytes")?,
        );
        if current.2 != 1 || current.3 != 0 {
            bail!(
                "{} did not reset its arena after one host iteration: reset_count={}, live_bytes={}",
                workload.id,
                current.2,
                current.3
            );
        }
        if exported_u32_global(&instance, &store, "__restrict_arena_error_code")? != 0 {
            bail!(
                "{} reported an arena error during its memory probe",
                workload.id
            );
        }
        if let Some(previous) = observed {
            if previous != current {
                bail!(
                    "{} memory metrics changed across identical reset iterations: {:?} then {:?}",
                    workload.id,
                    previous,
                    current
                );
            }
        }
        observed = Some(current);
    }
    let (peak_bytes, allocation_count, reset_count, live_bytes_after_call) =
        observed.context("memory probe did not execute")?;
    if (allocation_count == 0) != (peak_bytes == 0) {
        bail!(
            "{} reported inconsistent allocation and peak-byte metrics",
            workload.id
        );
    }

    let exhaustion = workload
        .memory_probe
        .as_ref()
        .map(|config| run_exhaustion_probe(compiler, workload, source_path, artifact_dir, config))
        .transpose()?;
    let compressed = zstd::bulk::compress(&wasm, 19)?;
    Ok((
        ArtifactResult {
            wasm_sha256: sha256(&wasm),
            compile_ns,
            wasm_bytes: wasm.len(),
            zstd_bytes: compressed.len(),
        },
        MemoryMetrics {
            peak_bytes,
            allocation_count,
            reset_count,
            live_bytes_after_call,
            verified_iterations: 2,
            exhaustion,
        },
    ))
}

fn run_exhaustion_probe(
    compiler: &Path,
    workload: &Workload,
    source_path: &Path,
    artifact_dir: &Path,
    config: &MemoryProbeConfig,
) -> Result<ExhaustionResult> {
    let wasm_path = artifact_dir.join(format!("{}.exhaustion.wasm", workload.id));
    compile_artifact(
        compiler,
        workload,
        source_path,
        &wasm_path,
        config.exhaustion_arena_bytes,
        true,
        false,
    )?;
    let wasm = fs::read(&wasm_path)?;
    Validator::new()
        .validate_all(&wasm)
        .with_context(|| format!("{} emitted invalid exhaustion-probe Wasm", workload.id))?;
    reject_imports(&format!("{} exhaustion", workload.id), &wasm)?;

    let engine = Engine::default();
    let module = Module::new(&engine, &wasm[..])?;
    let mut store = Store::new(&engine, ());
    let instance = Linker::new(&engine).instantiate_and_start(&mut store, &module)?;
    let function = instance.get_typed_func::<i32, i32>(&store, &workload.export)?;
    if let Ok(actual) = function.call(&mut store, workload.input) {
        bail!(
            "{} exhaustion probe returned {actual} instead of trapping",
            workload.id
        );
    }
    let error_code = exported_u32_global(&instance, &store, "__restrict_arena_error_code")?;
    let requested_bytes =
        exported_u32_global(&instance, &store, "__restrict_arena_error_requested_bytes")?;
    let capacity = exported_u32_global(&instance, &store, "__restrict_arena_capacity_bytes")?;
    if error_code != 1 || requested_bytes == 0 || capacity != config.exhaustion_arena_bytes {
        bail!(
            "{} exhaustion diagnostics were invalid: code={}, requested={}, capacity={}",
            workload.id,
            error_code,
            requested_bytes,
            capacity
        );
    }
    Ok(ExhaustionResult {
        arena_bytes: capacity,
        error_code,
        requested_bytes,
        trapped: true,
    })
}

fn exported_u32_global(instance: &Instance, store: &Store<()>, name: &str) -> Result<u32> {
    let global = instance
        .get_global(store, name)
        .with_context(|| format!("missing exported global '{name}'"))?;
    match global.get(store) {
        Val::I32(value) => Ok(value as u32),
        other => bail!("exported global '{name}' is not i32: {other:?}"),
    }
}

fn reject_imports(id: &str, wasm: &[u8]) -> Result<()> {
    for payload in Parser::new(0).parse_all(wasm) {
        if let Payload::ImportSection(section) = payload? {
            if section.count() > 0 {
                bail!("{id} is a wasm-core workload but emitted imports");
            }
        }
    }
    Ok(())
}

fn verify_result(id: &str, actual: i32, expected: i32) -> Result<()> {
    if actual != expected {
        bail!("{id} returned {actual}; expected checksum {expected}");
    }
    Ok(())
}

fn elapsed_ns(started: Instant) -> u64 {
    u64::try_from(started.elapsed().as_nanos()).unwrap_or(u64::MAX)
}

fn summarize(samples: &[u64]) -> TimingSummary {
    let mut sorted = samples.to_vec();
    sorted.sort_unstable();
    let total = sorted.iter().map(|value| *value as f64).sum::<f64>();
    TimingSummary {
        min_ns: sorted.first().copied().unwrap_or(0),
        median_ns: sorted.get(sorted.len() / 2).copied().unwrap_or(0),
        mean_ns: if sorted.is_empty() {
            0.0
        } else {
            total / sorted.len() as f64
        },
        max_ns: sorted.last().copied().unwrap_or(0),
    }
}

fn sha256(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

fn command_stdout(command: &mut Command) -> Option<String> {
    let output = command.output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_string())
}

fn detect_cpu() -> String {
    if cfg!(target_os = "macos") {
        return command_stdout(Command::new("sysctl").args(["-n", "machdep.cpu.brand_string"]))
            .unwrap_or_else(|| "unknown".to_string());
    }
    if cfg!(target_os = "linux") {
        return fs::read_to_string("/proc/cpuinfo")
            .ok()
            .and_then(|cpuinfo| {
                cpuinfo.lines().find_map(|line| {
                    line.strip_prefix("model name")
                        .and_then(|value| value.split_once(':'))
                        .map(|(_, value)| value.trim().to_string())
                })
            })
            .unwrap_or_else(|| "unknown".to_string());
    }
    "unknown".to_string()
}
