use anyhow::Result;
use colored::*;
use super::{find_project_root, load_manifest, print_success, print_error, print_warning, print_info};
use crate::vault::Vault;
use walkdir;
use which;

pub async fn doctor_check() -> Result<()> {
    println!("{}", "Running project diagnostics...".bold());
    println!();
    
    let mut issues = Vec::new();
    let mut warnings = Vec::new();
    
    // Check project structure
    let root = match find_project_root() {
        Ok(root) => root,
        Err(_) => {
            print_error("Not in a Restrict Language project (no package.rl.toml found)");
            return Ok(());
        }
    };
    
    // Check manifest
    let manifest = match load_manifest() {
        Ok(m) => m,
        Err(e) => {
            issues.push(format!("Invalid manifest: {}", e));
            print_error("Cannot continue checks without valid manifest");
            return Ok(());
        }
    };
    
    // Check entry point exists
    let entry_path = root.join(&manifest.package.entry);
    if !entry_path.exists() {
        issues.push(format!("Entry point not found: {}", entry_path.display()));
    }
    
    // Check vault if it exists
    let vault_path = root.join("restrict-lock.toml");
    if vault_path.exists() {
        match Vault::load(&vault_path) {
            Ok(vault) => {
                // Check vault integrity
                let vault_errors = vault.verify_integrity()?;
                for error in vault_errors {
                    issues.push(error);
                }
            }
            Err(e) => {
                issues.push(format!("Invalid vault file: {}", e));
            }
        }
    }
    
    // Check for unfrozen public APIs
    check_unfrozen_apis(&root, &mut warnings).await?;
    
    // Check for circular dependencies in local files
    check_circular_deps(&root, &mut issues).await?;
    
    // Check cache directory
    let cache_dir = root.join(".restrict-cache");
    if cache_dir.exists() {
        let cache_size = calculate_dir_size(&cache_dir)?;
        if cache_size > 1_000_000_000 {
            // > 1GB
            warnings.push(format!(
                "Cache directory is large: {} MB. Consider running 'warden clean'",
                cache_size / 1_000_000
            ));
        }
    }
    
    // Check for required tools
    check_required_tools(&mut warnings)?;
    
    // Print results
    println!("{}", "=== Diagnostic Results ===".bold());
    println!();
    
    if issues.is_empty() && warnings.is_empty() {
        print_success("No issues found!");
    } else {
        if !issues.is_empty() {
            println!("{} {} found:", "Errors".red().bold(), issues.len());
            for issue in &issues {
                print_error(&format!("  {}", issue));
            }
            println!();
        }
        
        if !warnings.is_empty() {
            println!("{} {} found:", "Warnings".yellow().bold(), warnings.len());
            for warning in &warnings {
                print_warning(&format!("  {}", warning));
            }
        }
    }
    
    // Summary
    println!();
    print_info(&format!(
        "Project: {} v{}",
        manifest.package.name, manifest.package.version
    ));
    print_info(&format!("Dependencies: {}", manifest.dependencies.len()));
    
    Ok(())
}

async fn check_unfrozen_apis(_root: &std::path::Path, _warnings: &mut Vec<String>) -> Result<()> {
    // TODO: Implement actual check for unfrozen public APIs
    // This would analyze the AST to find public functions that use mutable prototypes
    Ok(())
}

async fn check_circular_deps(_root: &std::path::Path, _issues: &mut Vec<String>) -> Result<()> {
    // TODO: Implement circular dependency detection in source files
    // This would build a dependency graph of imports and check for cycles
    Ok(())
}

fn check_required_tools(warnings: &mut Vec<String>) -> Result<()> {
    // Check for restrict_lang compiler
    if which::which("restrict_lang").is_err() {
        warnings.push("'restrict_lang' compiler not found in PATH".to_string());
    }
    
    // Check for WASM runtime
    let has_wasmtime = which::which("wasmtime").is_ok();
    let has_wasmer = which::which("wasmer").is_ok();
    
    if !has_wasmtime && !has_wasmer {
        warnings.push("No WASM runtime found (wasmtime or wasmer recommended)".to_string());
    }
    
    Ok(())
}

fn calculate_dir_size(path: &std::path::Path) -> Result<u64> {
    let mut size = 0;
    
    for entry in walkdir::WalkDir::new(path) {
        let entry = entry?;
        if entry.file_type().is_file() {
            size += entry.metadata()?.len();
        }
    }
    
    Ok(size)
}