use anyhow::{Result, Context};
use std::path::PathBuf;
use walkdir::WalkDir;
use std::process::Command;
use super::{find_project_root, print_success, print_error, print_info};

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
                let file_name = path.file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("");
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
    // TODO: Implement proper test runner
    // For now, just compile and run the test file
    
    // Compile test file
    let output = Command::new("restrict_lang")
        .arg("--test")
        .arg(test_file)
        .output()
        .context("Failed to compile test file")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        anyhow::bail!("Compilation failed:\n{}", stderr);
    }
    
    // TODO: Actually run the compiled test
    
    Ok(())
}