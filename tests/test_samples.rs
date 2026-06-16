//! Integration test: every sample in samples/ must compile successfully.
//!
//! This ensures that documentation examples, the web playground,
//! and the canonical sample files all stay in sync with the compiler.

use restrict_lang::{parse_program, TypeChecker, WasmCodeGen};
use std::fs;
use std::path::Path;

fn compile_sample(path: &Path) -> Result<String, String> {
    let source = fs::read_to_string(path)
        .map_err(|e| format!("Failed to read {}: {}", path.display(), e))?;

    let (_, program) = parse_program(&source)
        .map_err(|e| format!("Parse error in {}: {:?}", path.display(), e))?;

    let mut checker = TypeChecker::new();
    checker
        .check_program(&program)
        .map_err(|e| format!("Type error in {}: {}", path.display(), e))?;

    let mut codegen = WasmCodeGen::new();
    let wat = codegen
        .generate(&program)
        .map_err(|e| format!("Codegen error in {}: {}", path.display(), e))?;

    Ok(wat)
}

#[test]
fn all_samples_compile() {
    let samples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples");
    assert!(
        samples_dir.exists(),
        "samples/ directory not found at {}",
        samples_dir.display()
    );

    let mut entries: Vec<_> = fs::read_dir(&samples_dir)
        .expect("Failed to read samples/ directory")
        .filter_map(|e| e.ok())
        .filter(|e| e.path().extension().map_or(false, |ext| ext == "rl"))
        .collect();

    entries.sort_by_key(|e| e.file_name());

    assert!(
        !entries.is_empty(),
        "No .rl files found in samples/ directory"
    );

    let mut failures = Vec::new();

    for entry in &entries {
        let path = entry.path();
        let name = path.file_stem().unwrap().to_string_lossy();
        match compile_sample(&path) {
            Ok(_) => println!("  ✓ {}", name),
            Err(e) => {
                println!("  ✗ {}", name);
                failures.push(e);
            }
        }
    }

    if !failures.is_empty() {
        panic!(
            "\n{} sample(s) failed to compile:\n\n{}",
            failures.len(),
            failures.join("\n\n")
        );
    }

    println!("\nAll {} samples compiled successfully.", entries.len());
}

#[test]
fn samples_manifest_matches_files() {
    let samples_dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("samples");
    let manifest_path = samples_dir.join("manifest.json");

    let manifest_content =
        fs::read_to_string(&manifest_path).expect("Failed to read samples/manifest.json");

    let manifest: serde_json::Value =
        serde_json::from_str(&manifest_content).expect("Failed to parse manifest.json");

    let sample_entries = manifest["samples"]
        .as_array()
        .expect("manifest.json must have a 'samples' array");

    // Every file in manifest must exist
    for entry in sample_entries {
        let file = entry["file"]
            .as_str()
            .expect("each sample needs a 'file' field");
        let path = samples_dir.join(file);
        assert!(
            path.exists(),
            "manifest.json references '{}' but file not found at {}",
            file,
            path.display()
        );
    }

    // Every .rl file in directory must be in manifest
    let manifest_files: Vec<&str> = sample_entries
        .iter()
        .filter_map(|e| e["file"].as_str())
        .collect();

    for entry in fs::read_dir(&samples_dir).unwrap().filter_map(|e| e.ok()) {
        let path = entry.path();
        if path.extension().map_or(false, |ext| ext == "rl") {
            let filename = path.file_name().unwrap().to_string_lossy();
            assert!(
                manifest_files.contains(&filename.as_ref()),
                "File '{}' exists in samples/ but is not listed in manifest.json",
                filename
            );
        }
    }
}
