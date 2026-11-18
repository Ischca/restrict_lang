use super::{find_project_root, load_manifest, print_info};
use crate::wasi_runner::{run_wasi_module, WasiState};
use anyhow::{bail, Context, Result};
use std::path::Path;
use std::process::Command;

pub async fn run_project(args: Vec<String>) -> Result<()> {
    let root = find_project_root()?;
    let manifest = load_manifest()?;
    if manifest.compiler_component_contract()?.is_some() {
        print_info(
            "warder run currently executes the Core Wasm program profile; forcing a Core build instead of the project's Component artifact",
        );
    }

    // The runtime command is intentionally Core-only until Warder has an
    // explicit Component host selection surface.
    super::build::build_project(false, false, false, true, false, false).await?;

    // Find the built WASM file
    let build_dir = root.join(&manifest.build.output);
    let output_name = format!("{}-{}", manifest.package.name, manifest.package.version);
    let wasm_path = build_dir.join(format!("{}.wasm", output_name));

    if !wasm_path.exists() {
        bail!("Built WASM file not found. Run 'warder build' first.");
    }

    match manifest.build.target.as_str() {
        "wasm32" | "wasm-core" | "host" => run_wasm(&wasm_path, args),
        "native-x86_64" => run_native(&wasm_path, args),
        target => bail!("Unsupported target: {}", target),
    }
}

fn run_wasm(wasm_path: &Path, args: Vec<String>) -> Result<()> {
    let wasm = std::fs::read(wasm_path)
        .with_context(|| format!("Failed to read compiled Wasm file {}", wasm_path.display()))?;
    let mut argv = Vec::with_capacity(args.len() + 1);
    argv.push(wasm_path.display().to_string());
    argv.extend(args);
    let envs: Vec<(String, String)> = std::env::vars().collect();

    let (exit_code, _) = run_wasi_module(&wasm, WasiState::inherited(argv, envs))?;
    if exit_code != 0 {
        std::process::exit(exit_code);
    }
    Ok(())
}

fn run_native(binary_path: &Path, args: Vec<String>) -> Result<()> {
    let exe_path = binary_path.with_extension(if cfg!(windows) { "exe" } else { "" });

    if !exe_path.exists() {
        bail!("Native executable not found. Make sure to build with native target.");
    }

    let mut cmd = Command::new(&exe_path);
    cmd.args(args);

    let status = cmd.status().context("Failed to run native executable")?;

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }

    Ok(())
}
