use anyhow::{Result, Context, bail};
use std::process::Command;
use indicatif::{ProgressBar, ProgressStyle};
use super::{find_project_root, load_manifest, print_success, print_info};
use crate::cage::Cage;
use crate::vault::{Vault, PackageLock, LockSource};
use crate::manifest::{Manifest, Dependency};

pub async fn build_project(
    release: bool,
    watch: bool,
    component: bool,
    verify: bool,
    repro: bool,
) -> Result<()> {
    let root = find_project_root()?;
    let manifest = load_manifest()?;
    
    if watch {
        print_info("Watch mode not implemented yet");
        return Ok(());
    }
    
    // Create build directory
    let build_dir = root.join(&manifest.build.output);
    std::fs::create_dir_all(&build_dir)?;
    
    // Load or create vault
    let vault_path = root.join("restrict-lock.toml");
    let mut vault = if vault_path.exists() {
        Vault::load(&vault_path)?
    } else {
        Vault::new()
    };
    
    // Resolve dependencies
    print_info("Resolving dependencies...");
    resolve_dependencies(&manifest, &mut vault).await?;
    vault.save(&vault_path)?;
    
    // Build the project
    print_info("Building project...");
    let progress = ProgressBar::new(100);
    progress.set_style(
        ProgressStyle::default_bar()
            .template("{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos:>7}/{len:7} {msg}")?
            .progress_chars("##-")
    );
    
    // Compile main entry
    progress.set_message("Compiling main module...");
    progress.set_position(50);
    
    let entry_path = root.join(&manifest.package.entry);
    let output_name = format!("{}-{}", manifest.package.name, manifest.package.version);
    let wasm_output = build_dir.join(format!("{}.wasm", output_name));
    
    // Call restrict_lang compiler
    let mut cmd = Command::new("restrict_lang");
    cmd.arg(&entry_path)
        .arg(&wasm_output);
    
    if release {
        cmd.arg("--release");
    }
    
    if component {
        cmd.arg("--component");
    }
    
    if repro {
        cmd.arg("--deterministic");
    }
    
    let output = cmd.output()
        .context("Failed to run restrict_lang compiler. Is it installed and in PATH?")?;
    
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Compilation failed:\n{}", stderr);
    }
    
    progress.set_position(90);
    
    // Create cage
    progress.set_message("Creating cage...");
    let wasm_bytes = std::fs::read(&wasm_output)?;
    let mut cage = Cage::new(
        manifest.package.name.clone(),
        manifest.package.version.clone(),
        wasm_bytes,
    );
    
    // Add WIT if component mode
    if component {
        // TODO: Extract WIT from component or generate it
        // For now, just calculate ABI hash
    }
    
    cage.calculate_abi_hash()?;
    
    // Save cage
    let cage_output = build_dir.join(format!("{}.rgc", output_name));
    cage.save(&cage_output)?;
    
    // Verify if requested
    if verify {
        progress.set_message("Verifying signatures...");
        // TODO: Implement signature verification
    }
    
    progress.finish_and_clear();
    
    print_success(&format!(
        "Built {} v{} → {}",
        manifest.package.name,
        manifest.package.version,
        cage_output.display()
    ));
    
    // Report build stats
    let cage_size = std::fs::metadata(&cage_output)?.len();
    print_info(&format!(
        "Cage size: {} KB (ABI hash: {})",
        cage_size / 1024,
        &cage.manifest.abi_hash[..8]
    ));
    
    Ok(())
}

async fn resolve_dependencies(manifest: &Manifest, vault: &mut Vault) -> Result<()> {
    // TODO: Implement full dependency resolution
    // For now, just add entries to vault
    
    for (name, dep) in &manifest.dependencies {
        if vault.get_package(name).is_some() {
            // Already resolved
            continue;
        }
        
        let lock = match dep {
            Dependency::Version(ver) => {
                // TODO: Fetch from registry
                PackageLock {
                    version: ver.clone(),
                    source: LockSource::Registry {
                        url: "https://wardhub.restrict-lang.org".to_string(),
                    },
                    abi_hash: String::new(),
                    sha256: String::new(),
                    dependencies: Default::default(),
                }
            }
            Dependency::Local { path } => {
                PackageLock {
                    version: "0.0.0".to_string(),
                    source: LockSource::Path {
                        path: path.clone(),
                    },
                    abi_hash: String::new(),
                    sha256: String::new(),
                    dependencies: Default::default(),
                }
            }
            Dependency::Git { git, tag } => {
                PackageLock {
                    version: tag.as_ref().unwrap_or(&"latest".to_string()).clone(),
                    source: LockSource::Git {
                        url: git.clone(),
                        rev: tag.as_ref().unwrap_or(&"HEAD".to_string()).clone(),
                    },
                    abi_hash: String::new(),
                    sha256: String::new(),
                    dependencies: Default::default(),
                }
            }
            Dependency::Foreign { wasm, wit } => {
                // TODO: Wrap foreign WASM
                PackageLock {
                    version: "0.0.0".to_string(),
                    source: LockSource::Cage {
                        path: format!(".restrict-cache/{}.rgc", name),
                    },
                    abi_hash: String::new(),
                    sha256: String::new(),
                    dependencies: Default::default(),
                }
            }
        };
        
        vault.add_package(name.to_string(), lock);
    }
    
    Ok(())
}