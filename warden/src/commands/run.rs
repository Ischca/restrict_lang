use anyhow::{Result, Context, bail};
use std::process::Command;
use super::{find_project_root, load_manifest};
use which;

pub async fn run_project(args: Vec<String>) -> Result<()> {
    // First build the project
    super::build::build_project(false, false, false, false, false).await?;
    
    let root = find_project_root()?;
    let manifest = load_manifest()?;
    
    // Find the built WASM file
    let build_dir = root.join(&manifest.build.output);
    let output_name = format!("{}-{}", manifest.package.name, manifest.package.version);
    let wasm_path = build_dir.join(format!("{}.wasm", output_name));
    
    if !wasm_path.exists() {
        bail!("Built WASM file not found. Run 'warden build' first.");
    }
    
    // Determine runtime based on target
    match manifest.build.target.as_str() {
        "wasm32" => {
            // Use wasmtime or similar runtime
            run_wasm(&wasm_path, args)
        }
        "native-x86_64" => {
            // Run native binary
            run_native(&wasm_path, args)
        }
        "host" => {
            // Run with host runtime
            run_host(&wasm_path, args)
        }
        target => {
            bail!("Unsupported target: {}", target)
        }
    }
}

fn run_wasm(wasm_path: &std::path::Path, args: Vec<String>) -> Result<()> {
    // Try wasmtime first
    if which::which("wasmtime").is_ok() {
        let mut cmd = Command::new("wasmtime");
        cmd.arg("run")
            .arg(wasm_path)
            .arg("--")
            .args(args);
        
        let status = cmd.status()
            .context("Failed to run with wasmtime")?;
        
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
        
        Ok(())
    } else if which::which("wasmer").is_ok() {
        // Fallback to wasmer
        let mut cmd = Command::new("wasmer");
        cmd.arg("run")
            .arg(wasm_path)
            .arg("--")
            .args(args);
        
        let status = cmd.status()
            .context("Failed to run with wasmer")?;
        
        if !status.success() {
            std::process::exit(status.code().unwrap_or(1));
        }
        
        Ok(())
    } else {
        bail!("No WASM runtime found. Please install wasmtime or wasmer.");
    }
}

fn run_native(binary_path: &std::path::Path, args: Vec<String>) -> Result<()> {
    // For native builds, the output would be an executable
    let exe_path = binary_path.with_extension(if cfg!(windows) { "exe" } else { "" });
    
    if !exe_path.exists() {
        bail!("Native executable not found. Make sure to build with native target.");
    }
    
    let mut cmd = Command::new(&exe_path);
    cmd.args(args);
    
    let status = cmd.status()
        .context("Failed to run native executable")?;
    
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    
    Ok(())
}

fn run_host(wasm_path: &std::path::Path, args: Vec<String>) -> Result<()> {
    // Use the restrict_lang interpreter/runtime
    let mut cmd = Command::new("restrict_lang");
    cmd.arg("run")
        .arg(wasm_path)
        .args(args);
    
    let status = cmd.status()
        .context("Failed to run with restrict_lang runtime")?;
    
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    
    Ok(())
}