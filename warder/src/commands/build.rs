use super::{find_project_root, load_manifest, print_info, print_success, print_warning};
use crate::cage::Cage;
use crate::manifest::{Dependency, Manifest};
use crate::vault::{LockSource, PackageLock, Vault};
use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use std::process::Command;

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
        print_warning(
            "Watch mode is experimental and out-of-scope for v0.0.1; no watcher was started",
        );
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
            .template(
                "{spinner:.green} [{elapsed_precise}] [{bar:40.cyan/blue}] {pos:>7}/{len:7} {msg}",
            )?
            .progress_chars("##-"),
    );

    // Compile main entry
    progress.set_message("Compiling main module...");
    progress.set_position(50);

    let entry_path = root.join(&manifest.package.entry);
    let output_name = format!("{}-{}", manifest.package.name, manifest.package.version);
    let wat_output = build_dir.join(format!("{}.wat", output_name));
    let wasm_output = build_dir.join(format!("{}.wasm", output_name));

    // Call restrict_lang compiler
    let compiler =
        std::env::var("RESTRICT_LANG_BIN").unwrap_or_else(|_| "restrict_lang".to_string());
    let mut cmd = Command::new(compiler);
    cmd.arg(&entry_path).arg(&wat_output);

    if release {
        print_warning(
            "Release optimizations are experimental and out-of-scope for v0.0.1; building without optimizations",
        );
    }

    if component {
        print_warning(
            "WASM Component output is experimental and out-of-scope for v0.0.1; building a core module cage",
        );
    }

    if repro {
        print_warning(
            "Deterministic build mode is experimental and out-of-scope for v0.0.1; using the default compiler output",
        );
    }

    let output = cmd
        .output()
        .context("Failed to run restrict_lang compiler. Is it installed and in PATH?")?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        bail!("Compilation failed:\n{}", stderr);
    }

    progress.set_position(90);

    let wasm_bytes = wat::parse_file(&wat_output)
        .with_context(|| format!("Failed to convert WAT to WASM: {}", wat_output.display()))?;
    std::fs::write(&wasm_output, &wasm_bytes)
        .with_context(|| format!("Failed to write WASM output: {}", wasm_output.display()))?;

    // Create cage
    progress.set_message("Creating cage...");
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
        print_warning(
            "Signature verification is experimental and out-of-scope for v0.0.1; skipping verification",
        );
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
            Dependency::Local { path } => PackageLock {
                version: "0.0.0".to_string(),
                source: LockSource::Path { path: path.clone() },
                abi_hash: String::new(),
                sha256: String::new(),
                dependencies: Default::default(),
            },
            Dependency::Git { git, tag } => PackageLock {
                version: tag.as_ref().unwrap_or(&"latest".to_string()).clone(),
                source: LockSource::Git {
                    url: git.clone(),
                    rev: tag.as_ref().unwrap_or(&"HEAD".to_string()).clone(),
                },
                abi_hash: String::new(),
                sha256: String::new(),
                dependencies: Default::default(),
            },
            Dependency::Foreign { .. } => {
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
