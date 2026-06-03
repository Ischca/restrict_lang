use super::{find_project_root, print_error, print_info, print_success};
use anyhow::{Context, Result};
use std::path::PathBuf;
use std::process::Command;
use walkdir::WalkDir;

pub async fn test_project(filter: Option<String>) -> Result<()> {
    let root = find_project_root()?;
    let tests_dir = root.join("tests");

    if !tests_dir.exists() {
        print_info("No tests directory found");
        return Ok(());
    }

    // Find all test files
    let mut test_files = Vec::new();
    for entry in WalkDir::new(&tests_dir) {
        let entry = entry?;
        let path = entry.path();

        if path.extension().and_then(|s| s.to_str()) == Some("rl") {
            if let Some(filter) = &filter {
                let file_name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !file_name.contains(filter) {
                    continue;
                }
            }

            test_files.push(path.to_path_buf());
        }
    }

    if test_files.is_empty() {
        print_info("No test files found");
        return Ok(());
    }

    print_info(&format!("Running {} test file(s)", test_files.len()));

    let mut passed = 0;
    let mut failed = 0;

    for test_file in test_files {
        print!("Testing {} ... ", test_file.display());

        match run_test_file(&test_file).await {
            Ok(_) => {
                println!("PASSED");
                passed += 1;
            }
            Err(e) => {
                println!("FAILED");
                print_error(&format!("  {}", e));
                failed += 1;
            }
        }
    }

    println!();
    if failed == 0 {
        print_success(&format!("All tests passed! ({} total)", passed));
    } else {
        print_error(&format!("{} passed, {} failed", passed, failed));
        std::process::exit(1);
    }

    Ok(())
}

async fn run_test_file(test_file: &PathBuf) -> Result<()> {
    // v0.0.1 does not have a dedicated test declaration syntax yet.
    // Treat test files as type-check smoke tests until the test DSL is designed.
    let compiler =
        std::env::var("RESTRICT_LANG_BIN").unwrap_or_else(|_| "restrict_lang".to_string());
    let output = Command::new(compiler)
        .arg("--check")
        .arg(test_file)
        .output()
        .context("Failed to type-check test file")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Type checking failed:\n{}", stderr);
    }

    Ok(())
}
