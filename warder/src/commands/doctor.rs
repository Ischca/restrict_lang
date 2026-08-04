use super::{
    find_project_root, load_manifest, print_error, print_info, print_success, print_warning,
};
use crate::dependencies::resolve_local_dependencies;
use crate::manifest::Manifest;
use crate::vault::Vault;
use anyhow::Result;
use colored::*;
use std::io::ErrorKind;
use std::path::Path;

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

    check_dependency_lock(&root, &manifest, &mut issues);

    // Check for unfrozen public APIs
    check_unfrozen_apis(&root, &mut warnings).await?;

    // Check for circular dependencies in local files
    check_circular_deps(&root, &mut warnings).await?;

    // Check cache directory
    let cache_dir = root.join(".restrict-cache");
    if cache_dir.exists() {
        let cache_size = calculate_dir_size(&cache_dir)?;
        if cache_size > 1_000_000_000 {
            // > 1GB
            warnings.push(format!(
                "Cache directory is large: {} MB. Consider running 'warder clean'",
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

fn check_dependency_lock(root: &Path, manifest: &Manifest, issues: &mut Vec<String>) {
    let vault_path = root.join("restrict-lock.toml");
    let mut lock_is_missing = false;

    let loaded_vault = match std::fs::symlink_metadata(&vault_path) {
        Ok(metadata) if metadata.file_type().is_symlink() || !metadata.is_file() => {
            issues.push(format!(
                "Malformed dependency lock: {} must be a regular file",
                vault_path.display()
            ));
            None
        }
        Ok(_) => match Vault::load(&vault_path) {
            Ok(vault) => match vault.verify_integrity() {
                Ok(errors) if errors.is_empty() => Some(vault),
                Ok(errors) => {
                    for error in errors {
                        issues.push(format!("Malformed dependency lock: {error}"));
                    }
                    None
                }
                Err(error) => {
                    issues.push(format!(
                        "Malformed dependency lock: failed to verify {}: {error:#}",
                        vault_path.display()
                    ));
                    None
                }
            },
            Err(error) => {
                issues.push(format!(
                    "Malformed dependency lock at {}: {error:#}",
                    vault_path.display()
                ));
                None
            }
        },
        Err(error) if error.kind() == ErrorKind::NotFound => {
            lock_is_missing = true;
            None
        }
        Err(error) => {
            issues.push(format!(
                "Malformed dependency lock: failed to inspect {}: {error}",
                vault_path.display()
            ));
            None
        }
    };

    let planned_vault = match resolve_local_dependencies(root, manifest) {
        Ok((_, vault)) => Some(vault),
        Err(error) => {
            issues.push(format!("Dependency resolution failed: {error:#}"));
            None
        }
    };

    match (loaded_vault, planned_vault) {
        (Some(loaded), Some(planned)) if loaded != planned => issues.push(format!(
            "Dependency lock is stale: {} does not match the current direct local dependencies; run 'warder build' to refresh it",
            vault_path.display()
        )),
        (None, Some(planned)) if lock_is_missing && !planned.packages.is_empty() => {
            issues.push(format!(
                "Dependency lock is missing: {}; run 'warder build' to create it",
                vault_path.display()
            ));
        }
        _ => {}
    }
}

async fn check_unfrozen_apis(_root: &std::path::Path, warnings: &mut Vec<String>) -> Result<()> {
    // TODO: Implement actual check for unfrozen public APIs
    // This would analyze the AST to find public functions that use mutable prototypes
    warnings.push(
        "Public API freeze analysis is experimental and out-of-scope for v0.0.1; this doctor check is skipped"
            .to_string(),
    );
    Ok(())
}

async fn check_circular_deps(_root: &std::path::Path, warnings: &mut Vec<String>) -> Result<()> {
    // TODO: Implement circular dependency detection in source files
    // This would build a dependency graph of imports and check for cycles
    warnings.push(
        "Circular dependency analysis is experimental and out-of-scope for v0.0.1; this doctor check is skipped"
            .to_string(),
    );
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::Dependency;
    use std::fs;
    use tempfile::TempDir;

    fn direct_local_manifest(path: &str) -> Manifest {
        let mut manifest = Manifest::new("app");
        manifest.dependencies.insert(
            "local_utils".to_string(),
            Dependency::Local {
                path: path.to_string(),
            },
        );
        manifest
    }

    fn write_local_dependency(root: &Path) {
        fs::create_dir_all(root.join("src")).unwrap();
        let mut manifest = Manifest::new("local-utils");
        manifest.package.version = "1.2.3".to_string();
        manifest.package.entry = "src/lib.rl".to_string();
        manifest.save(&root.join("package.rl.toml")).unwrap();
        fs::write(
            root.join("src/lib.rl"),
            "pub fun score: () -> Int32 = { 7 }\n",
        )
        .unwrap();
    }

    fn local_fixture() -> (TempDir, std::path::PathBuf, Manifest) {
        let temp = TempDir::new().unwrap();
        let app = temp.path().join("app");
        let dependency = temp.path().join("local-utils");
        fs::create_dir_all(&app).unwrap();
        write_local_dependency(&dependency);
        let manifest = direct_local_manifest("../local-utils");
        (temp, app, manifest)
    }

    #[test]
    fn dependency_lock_check_reports_missing_lock() {
        let (_temp, app, manifest) = local_fixture();
        let mut issues = Vec::new();

        check_dependency_lock(&app, &manifest, &mut issues);

        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(
            issues[0].contains("Dependency lock is missing"),
            "{issues:?}"
        );
    }

    #[test]
    fn dependency_lock_check_accepts_current_lock_and_reports_stale_source() {
        let (temp, app, manifest) = local_fixture();
        let (_, vault) = resolve_local_dependencies(&app, &manifest).unwrap();
        vault.save(&app.join("restrict-lock.toml")).unwrap();
        let mut issues = Vec::new();

        check_dependency_lock(&app, &manifest, &mut issues);
        assert!(issues.is_empty(), "{issues:?}");

        fs::write(
            temp.path().join("local-utils/src/lib.rl"),
            "pub fun score: () -> Int32 = { 8 }\n",
        )
        .unwrap();
        check_dependency_lock(&app, &manifest, &mut issues);

        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(issues[0].contains("Dependency lock is stale"), "{issues:?}");
    }

    #[test]
    fn dependency_lock_check_reports_malformed_lock_without_stale_duplicate() {
        let (_temp, app, manifest) = local_fixture();
        let (_, mut vault) = resolve_local_dependencies(&app, &manifest).unwrap();
        vault.packages.get_mut("local_utils").unwrap().sha256 = "A".repeat(64);
        vault.save(&app.join("restrict-lock.toml")).unwrap();
        let mut issues = Vec::new();

        check_dependency_lock(&app, &manifest, &mut issues);

        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(
            issues[0].contains("Malformed dependency lock")
                && issues[0].contains("non-canonical SHA-256"),
            "{issues:?}"
        );
    }

    #[test]
    fn dependency_lock_check_reports_resolver_errors_as_diagnostics() {
        let temp = TempDir::new().unwrap();
        let app = temp.path().join("app");
        fs::create_dir_all(&app).unwrap();
        let manifest = direct_local_manifest("../missing");
        let mut issues = Vec::new();

        check_dependency_lock(&app, &manifest, &mut issues);

        assert_eq!(issues.len(), 1, "{issues:?}");
        assert!(
            issues[0].contains("Dependency resolution failed") && issues[0].contains("local_utils"),
            "{issues:?}"
        );
    }

    #[test]
    fn dependency_lock_check_allows_missing_lock_without_dependencies() {
        let temp = TempDir::new().unwrap();
        let app = temp.path().join("app");
        fs::create_dir_all(&app).unwrap();
        let mut issues = Vec::new();

        check_dependency_lock(&app, &Manifest::new("app"), &mut issues);

        assert!(issues.is_empty(), "{issues:?}");
    }
}
