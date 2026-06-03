use super::{print_info, print_success, print_warning};
use crate::cage::Cage;
use anyhow::{bail, Context, Result};
use std::fs;
use std::path::Path;

pub fn wrap_wasm(
    wasm_path: &str,
    name: &str,
    version: &str,
    wit_path: Option<String>,
    output: Option<String>,
) -> Result<()> {
    let wasm_path = Path::new(wasm_path);

    if !wasm_path.exists() {
        bail!("WASM file not found: {}", wasm_path.display());
    }

    // Read WASM bytes
    let wasm_bytes = fs::read(wasm_path)
        .with_context(|| format!("Failed to read WASM file: {}", wasm_path.display()))?;

    // Validate WASM magic number
    if wasm_bytes.len() < 4 || &wasm_bytes[0..4] != b"\0asm" {
        bail!("Invalid WASM file: {}", wasm_path.display());
    }

    print_warning(
        "Foreign WASM wrapping is experimental and out-of-scope for v0.0.1; output is for local evaluation",
    );

    // Create cage
    let mut cage = Cage::new(name.to_string(), version.to_string(), wasm_bytes);

    // Add WIT if provided
    if let Some(wit_path) = wit_path {
        let wit_path = Path::new(&wit_path);
        if !wit_path.exists() {
            bail!("WIT file not found: {}", wit_path.display());
        }

        let wit_content = fs::read_to_string(wit_path)
            .with_context(|| format!("Failed to read WIT file: {}", wit_path.display()))?;

        let wit_filename = wit_path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("interface.wit")
            .to_string();

        cage.add_wit(wit_filename, wit_content);
        print_info(&format!("Added WIT interface from {}", wit_path.display()));
    } else {
        print_warning("No WIT file provided. Consider adding one for better interoperability.");
        print_info("You can generate a basic WIT template with 'warder wit-gen'");
    }

    // Calculate ABI hash
    cage.calculate_abi_hash()?;

    // Determine output path
    let output_path = if let Some(output) = output {
        Path::new(&output).to_path_buf()
    } else {
        Path::new(&format!("{}-{}.rgc", name, version)).to_path_buf()
    };

    // Save cage
    cage.save(&output_path)?;

    print_info(&format!(
        "Wrote experimental cage: {}",
        output_path.display()
    ));
    print_info(&format!("ABI hash: {}", &cage.manifest.abi_hash[..8]));
    print_info(&format!("SHA256: {}", &cage.manifest.sha256[..8]));

    Ok(())
}

pub fn unwrap_cage(cage_path: &str, component: bool, output: Option<String>) -> Result<()> {
    let cage_path = Path::new(cage_path);

    if !cage_path.exists() {
        bail!("Cage file not found: {}", cage_path.display());
    }

    // Load cage
    let cage = Cage::load(cage_path)
        .with_context(|| format!("Failed to load cage: {}", cage_path.display()))?;

    // Determine output directory
    let output_dir = if let Some(output) = output {
        Path::new(&output).to_path_buf()
    } else {
        Path::new(&format!("{}-{}", cage.manifest.name, cage.manifest.version)).to_path_buf()
    };

    // Extract cage contents
    cage.extract(&output_dir)?;

    if component {
        print_warning(
            "WASM Component conversion is experimental and out-of-scope for v0.0.1; extracted cage contents without conversion",
        );
        print_info(&format!(
            "Extracted experimental cage contents to: {}",
            output_dir.display()
        ));
    } else {
        print_success(&format!("Unwrapped cage to: {}", output_dir.display()));
    }

    print_info(&format!(
        "Package: {} v{}",
        cage.manifest.name, cage.manifest.version
    ));
    print_info(&format!("ABI hash: {}", &cage.manifest.abi_hash[..8]));

    Ok(())
}
