use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
use wasmi::{Engine, Linker, Module, Store};
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
    compression: String,
    rustc: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorkloadResult {
    id: String,
    source: String,
    source_sha256: String,
    wasm_sha256: String,
    export: String,
    input: i32,
    expected: i32,
    arena_bytes: u32,
    compile_ns: u64,
    runtime_compile_ns: u64,
    cold_instantiation_ns: u64,
    wasm_bytes: usize,
    zstd_bytes: usize,
    warmup_iterations: usize,
    iterations: usize,
    execution: TimingSummary,
    execution_samples_ns: Vec<u64>,
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
        schema_version: 1,
        generated_at_unix_seconds: SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs(),
        source_revision: command_stdout(
            Command::new("git")
                .arg("-C")
                .arg(repo_root)
                .args(["rev-parse", "HEAD"]),
        )
        .unwrap_or_else(|| "unknown".to_string()),
        source_dirty: !Command::new("git")
            .arg("-C")
            .arg(repo_root)
            .args(["diff", "--quiet", "HEAD", "--"])
            .status()
            .is_ok_and(|status| status.success()),
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
            compression: "zstd level 19".to_string(),
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
            "{}: median={} ns, wasm={} bytes, checksum={}",
            workload.id, workload.execution.median_ns, workload.wasm_bytes, workload.expected
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
    let wasm_path = artifact_dir.join(format!("{}.wasm", workload.id));

    let compile_started = Instant::now();
    let output = Command::new(compiler)
        .args(["--target", "wasm-core", "--emit", "wasm", "--arena-bytes"])
        .arg(workload.arena_bytes.to_string())
        .arg(&source_path)
        .arg(&wasm_path)
        .output()
        .with_context(|| format!("failed to run compiler {}", compiler.display()))?;
    let compile_ns = elapsed_ns(compile_started);
    if !output.status.success() {
        bail!(
            "{} failed to compile:\n{}",
            workload.id,
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let wasm = fs::read(&wasm_path)?;
    Validator::new()
        .validate_all(&wasm)
        .with_context(|| format!("{} emitted invalid Wasm", workload.id))?;
    reject_imports(&workload.id, &wasm)?;

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

    let compressed = zstd::bulk::compress(&wasm, 19)?;
    Ok(WorkloadResult {
        id: workload.id.clone(),
        source: workload.source.clone(),
        source_sha256: sha256(&source),
        wasm_sha256: sha256(&wasm),
        export: workload.export.clone(),
        input: workload.input,
        expected: workload.expected,
        arena_bytes: workload.arena_bytes,
        compile_ns,
        runtime_compile_ns,
        cold_instantiation_ns,
        wasm_bytes: wasm.len(),
        zstd_bytes: compressed.len(),
        warmup_iterations,
        iterations,
        execution: summarize(&samples),
        execution_samples_ns: samples,
    })
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
