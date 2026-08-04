use super::{find_project_root, load_manifest, print_info, print_success, print_warning};
use crate::cage::Cage;
use crate::dependencies::{
    resolve_local_dependencies, snapshot_local_dependencies, validate_local_dependency_layout,
};
use anyhow::{bail, Context, Result};
use indicatif::{ProgressBar, ProgressStyle};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashSet;
use std::fs::{File, OpenOptions};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::process::Command;
use walkdir::WalkDir;

const APPLICATION_SOURCE_HASH_VERSION: &[u8] = b"warder.application-source.v1";

pub async fn build_project(
    release: bool,
    watch: bool,
    component: bool,
    verify: bool,
    repro: bool,
) -> Result<()> {
    let root = find_project_root()?;
    let manifest = load_manifest()?;
    let root = std::fs::canonicalize(&root)
        .with_context(|| format!("Failed to canonicalize project root {}", root.display()))?;
    validate_utf8_path(&root, "Project root")?;
    validate_package_output_identity(&manifest.package.name, &manifest.package.version)?;
    let entry_path =
        resolve_project_file(&root, Path::new(&manifest.package.entry), "package.entry")?;
    let application_source_root = entry_path
        .parent()
        .context("package.entry must have a source directory")?
        .to_path_buf();

    if watch {
        print_warning(
            "Watch mode is experimental and out-of-scope for v0.0.1; no watcher was started",
        );
        return Ok(());
    }

    let _build_lock = acquire_project_build_lock(&root)?;

    // Resolve dependencies
    print_info("Resolving dependencies...");
    let (resolved_dependencies, vault) = resolve_local_dependencies(&root, &manifest)?;
    validate_local_dependency_layout(&root, &manifest, &resolved_dependencies)?;
    let vault_path = root.join("restrict-lock.toml");

    // Create build directory only after dependency validation succeeds.
    let build_dir =
        ensure_project_directory(&root, Path::new(&manifest.build.output), "build.output")?;
    validate_build_roots_are_disjoint(
        &build_dir,
        &application_source_root,
        &resolved_dependencies,
    )?;
    let output_name = format!("{}-{}", manifest.package.name, manifest.package.version);
    let wat_output = build_dir.join(format!("{}.wat", output_name));
    let wasm_output = build_dir.join(format!("{}.wasm", output_name));
    let cage_output = build_dir.join(format!("{}.rgc", output_name));
    recover_interrupted_artifact_transaction(
        &root,
        &build_dir,
        &[
            wat_output.as_path(),
            wasm_output.as_path(),
            cage_output.as_path(),
            vault_path.as_path(),
        ],
    )?;
    let staging_dir = tempfile::Builder::new()
        .prefix(".warder-build-")
        .tempdir_in(&build_dir)
        .with_context(|| {
            format!(
                "Failed to create staging directory in {}",
                build_dir.display()
            )
        })?;
    let mut application_exclusions = vec![build_dir.as_path()];
    application_exclusions.extend(
        resolved_dependencies
            .iter()
            .map(|dependency| dependency.package_root.as_path()),
    );
    let application_snapshot = snapshot_application_source(
        &root,
        &entry_path,
        &staging_dir.path().join("application"),
        &application_exclusions,
    )?;
    let snapshot_dependencies = snapshot_local_dependencies(
        &resolved_dependencies,
        &vault,
        &staging_dir.path().join("dependencies"),
    )?;

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

    let staged_wat_output = staging_dir.path().join(format!("{}.wat", output_name));
    let staged_wasm_output = staging_dir.path().join(format!("{}.wasm", output_name));
    let staged_cage_output = staging_dir.path().join(format!("{}.rgc", output_name));

    // Call restrict_lang compiler
    let compiler =
        std::env::var("RESTRICT_LANG_BIN").unwrap_or_else(|_| "restrict_lang".to_string());
    let mut cmd = Command::new(compiler);
    for dependency in &snapshot_dependencies {
        cmd.arg("--module-root").arg(dependency.module_root_arg());
    }
    cmd.current_dir(&application_snapshot.source_root)
        .arg(&application_snapshot.entry_path)
        .arg(&staged_wat_output);

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

    let wasm_bytes = wat::parse_file(&staged_wat_output).with_context(|| {
        format!(
            "Failed to convert WAT to WASM: {}",
            staged_wat_output.display()
        )
    })?;
    std::fs::write(&staged_wasm_output, &wasm_bytes).with_context(|| {
        format!(
            "Failed to write staged WASM output: {}",
            staged_wasm_output.display()
        )
    })?;

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
    cage.save(&staged_cage_output)?;

    // Verify if requested
    if verify {
        progress.set_message("Verifying signatures...");
        print_warning(
            "Signature verification is experimental and out-of-scope for v0.0.1; skipping verification",
        );
    }

    let current_manifest = load_manifest()?;
    if current_manifest != manifest {
        bail!("package.rl.toml changed during the build; retry with a stable manifest");
    }
    let current_application_hash = hash_application_source(&root, &application_exclusions)?;
    if current_application_hash != application_snapshot.source_hash {
        bail!("Application sources changed during the build; retry to produce a consistent artifact set");
    }
    let (current_dependencies, current_vault) =
        resolve_local_dependencies(&root, &current_manifest)?;
    if current_dependencies != resolved_dependencies || current_vault != vault {
        bail!("Local dependency sources changed during the build; retry to produce a consistent lock and artifact set");
    }

    let staged_vault_path = staging_dir.path().join("restrict-lock.toml");
    vault.save(&staged_vault_path)?;
    let backup_dir = staging_dir.path().join("backups");
    let transaction_path = root.join(".warder-build-transaction.toml");
    if let Err(error) = commit_staged_files(
        &[
            (&staged_wat_output, &wat_output),
            (&staged_wasm_output, &wasm_output),
            (&staged_cage_output, &cage_output),
            (&staged_vault_path, &vault_path),
        ],
        &backup_dir,
        &transaction_path,
    ) {
        if error.downcast_ref::<ArtifactRollbackFailure>().is_some() {
            let recovery_path = staging_dir.keep();
            return Err(error.context(format!(
                "Recovery files were preserved at {}; rerun warder build to retry automatic recovery",
                recovery_path.display()
            )));
        }
        return Err(error);
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

struct ProjectBuildLock {
    file: File,
}

fn validate_utf8_path(path: &Path, label: &str) -> Result<()> {
    path.to_str()
        .with_context(|| format!("{} is not valid UTF-8: {:?}", label, path))?;
    Ok(())
}

impl Drop for ProjectBuildLock {
    fn drop(&mut self) {
        let _ = File::unlock(&self.file);
    }
}

fn acquire_project_build_lock(root: &Path) -> Result<ProjectBuildLock> {
    let lock_path = root.join(".warder-build.lock");
    match std::fs::symlink_metadata(&lock_path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                bail!(
                    "Project build lock must be a regular file: {}",
                    lock_path.display()
                );
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "Failed to inspect project build lock {}",
                    lock_path.display()
                )
            });
        }
    }
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .with_context(|| format!("Failed to open project build lock {}", lock_path.display()))?;
    match File::try_lock(&file) {
        Ok(()) => {}
        Err(std::fs::TryLockError::WouldBlock) => {
            print_info("Waiting for another Warder build in this project to finish...");
            File::lock(&file).with_context(|| {
                format!(
                    "Failed to wait for project build lock {}",
                    lock_path.display()
                )
            })?;
        }
        Err(std::fs::TryLockError::Error(error)) => {
            return Err(error).with_context(|| {
                format!(
                    "Failed to acquire project build lock {}",
                    lock_path.display()
                )
            });
        }
    }
    Ok(ProjectBuildLock { file })
}

fn validate_build_roots_are_disjoint(
    build_dir: &Path,
    application_source_root: &Path,
    dependencies: &[crate::dependencies::ResolvedLocalDependency],
) -> Result<()> {
    if build_dir.starts_with(application_source_root)
        || application_source_root.starts_with(build_dir)
    {
        bail!(
            "build.output {} overlaps application source {}; build and source roots must be disjoint",
            build_dir.display(),
            application_source_root.display()
        );
    }
    for dependency in dependencies {
        if build_dir.starts_with(&dependency.source_dir)
            || dependency.source_dir.starts_with(build_dir)
        {
            bail!(
                "build.output {} overlaps local dependency source {} for '{}'; build and dependency source roots must be disjoint",
                build_dir.display(),
                dependency.source_dir.display(),
                dependency.alias
            );
        }
        if application_source_root.starts_with(&dependency.source_dir)
            || dependency.source_dir.starts_with(application_source_root)
        {
            bail!(
                "Application source {} overlaps local dependency source {} for '{}'; package source roots must be disjoint",
                application_source_root.display(),
                dependency.source_dir.display(),
                dependency.alias
            );
        }
    }
    Ok(())
}

struct ApplicationSnapshot {
    source_root: PathBuf,
    entry_path: PathBuf,
    source_hash: String,
}

fn snapshot_application_source(
    project_root: &Path,
    live_entry_path: &Path,
    snapshot_root: &Path,
    excluded_roots: &[&Path],
) -> Result<ApplicationSnapshot> {
    let entry_relative = live_entry_path
        .strip_prefix(project_root)
        .with_context(|| {
            format!(
                "Application entry {} is outside project root {}",
                live_entry_path.display(),
                project_root.display()
            )
        })?;
    let source_hash = hash_application_source(project_root, excluded_roots)?;
    std::fs::create_dir_all(snapshot_root).with_context(|| {
        format!(
            "Failed to create application source snapshot {}",
            snapshot_root.display()
        )
    })?;

    for entry in WalkDir::new(project_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            application_walk_entry_is_included(project_root, excluded_roots, entry)
        })
    {
        let entry = entry.with_context(|| {
            format!(
                "Failed to snapshot application source {}",
                project_root.display()
            )
        })?;
        let path = entry.path();
        if entry.file_type().is_symlink() {
            bail!(
                "Application source tree must not contain symlinks: {}",
                path.display()
            );
        }
        let relative = path.strip_prefix(project_root).with_context(|| {
            format!(
                "Application source {} escaped {} while snapshotting",
                path.display(),
                project_root.display()
            )
        })?;
        let destination = snapshot_root.join(relative);
        if entry.file_type().is_dir() {
            std::fs::create_dir_all(&destination).with_context(|| {
                format!(
                    "Failed to create application snapshot directory {}",
                    destination.display()
                )
            })?;
        } else if entry.file_type().is_file()
            && path.extension().and_then(|part| part.to_str()) == Some("rl")
        {
            copy_application_source_file(path, &destination)?;
        }
    }

    let snapshot_hash = hash_application_source(snapshot_root, &[])?;
    if snapshot_hash != source_hash {
        bail!("Application sources changed while creating an immutable build snapshot");
    }
    let entry_path = snapshot_root.join(entry_relative);
    if !entry_path.is_file() {
        bail!(
            "Application entry was not copied into the build snapshot: {}",
            entry_path.display()
        );
    }
    validate_utf8_path(snapshot_root, "Application compiler source root")?;
    validate_utf8_path(&entry_path, "Application compiler entry")?;

    Ok(ApplicationSnapshot {
        source_root: snapshot_root.to_path_buf(),
        entry_path,
        source_hash,
    })
}

fn copy_application_source_file(source: &Path, destination: &Path) -> Result<()> {
    let metadata = std::fs::symlink_metadata(source)
        .with_context(|| format!("Failed to inspect application source {}", source.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!(
            "Application snapshot source must be a regular file: {}",
            source.display()
        );
    }
    std::fs::copy(source, destination).with_context(|| {
        format!(
            "Failed to copy application source {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

fn hash_application_source(source_root: &Path, excluded_roots: &[&Path]) -> Result<String> {
    let canonical_source_root = std::fs::canonicalize(source_root).with_context(|| {
        format!(
            "Failed to canonicalize application source root {}",
            source_root.display()
        )
    })?;
    let mut files = Vec::new();
    let mut canonical_files = HashSet::new();
    for entry in WalkDir::new(source_root)
        .follow_links(false)
        .into_iter()
        .filter_entry(|entry| {
            application_walk_entry_is_included(source_root, excluded_roots, entry)
        })
    {
        let entry = entry.with_context(|| {
            format!(
                "Failed to inspect application source {}",
                source_root.display()
            )
        })?;
        let path = entry.path();
        if entry.file_type().is_symlink() {
            bail!(
                "Application source tree must not contain symlinks: {}",
                path.display()
            );
        }
        if !entry.file_type().is_file()
            || path.extension().and_then(|part| part.to_str()) != Some("rl")
        {
            continue;
        }
        let canonical = std::fs::canonicalize(path)
            .with_context(|| format!("Failed to canonicalize source file {}", path.display()))?;
        if !canonical.starts_with(&canonical_source_root) {
            bail!(
                "Application source escapes {} through {}",
                source_root.display(),
                path.display()
            );
        }
        if !canonical_files.insert(canonical) {
            bail!(
                "Application source file is reachable through multiple paths: {}",
                path.display()
            );
        }
        let relative = normalized_relative_source_path(source_root, path)?;
        files.push((relative, path.to_path_buf()));
    }
    files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hasher = Sha256::new();
    hash_source_frame(&mut hasher, APPLICATION_SOURCE_HASH_VERSION);
    for (relative, path) in files {
        let content = std::fs::read(&path)
            .with_context(|| format!("Failed to read application source {}", path.display()))?;
        hash_source_frame(&mut hasher, relative.as_bytes());
        hash_source_frame(&mut hasher, &content);
    }
    Ok(hex::encode(hasher.finalize()))
}

fn application_walk_entry_is_included(
    source_root: &Path,
    excluded_roots: &[&Path],
    entry: &walkdir::DirEntry,
) -> bool {
    let path = entry.path();
    if path == source_root {
        return true;
    }
    if excluded_roots
        .iter()
        .any(|excluded| path.starts_with(excluded))
    {
        return false;
    }
    let Ok(relative) = path.strip_prefix(source_root) else {
        return false;
    };
    !relative.components().next().is_some_and(|component| {
        matches!(
            component,
            Component::Normal(part)
                if part == std::ffi::OsStr::new(".git")
                    || part == std::ffi::OsStr::new("target")
                    || part == std::ffi::OsStr::new("node_modules")
                    || part == std::ffi::OsStr::new(".restrict-cache")
        )
    })
}

fn normalized_relative_source_path(root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(root).with_context(|| {
        format!(
            "Application source {} is outside {}",
            path.display(),
            root.display()
        )
    })?;
    let mut parts = Vec::new();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            bail!(
                "Application source path is not normalized: {}",
                relative.display()
            );
        };
        parts.push(part.to_str().with_context(|| {
            format!("Application source path is not valid UTF-8: {:?}", relative)
        })?);
    }
    Ok(parts.join("/"))
}

fn hash_source_frame(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[derive(Debug, Serialize, Deserialize)]
struct ArtifactTransactionJournal {
    version: u32,
    entries: Vec<ArtifactTransactionEntry>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ArtifactTransactionEntry {
    destination: PathBuf,
    backup: PathBuf,
    destination_existed: bool,
}

#[derive(Debug)]
struct ArtifactRollbackFailure {
    message: String,
}

impl std::fmt::Display for ArtifactRollbackFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for ArtifactRollbackFailure {}

fn recover_interrupted_artifact_transaction(
    root: &Path,
    build_dir: &Path,
    expected_destinations: &[&Path],
) -> Result<()> {
    let journal_path = root.join(".warder-build-transaction.toml");
    let metadata = match std::fs::symlink_metadata(&journal_path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => {
            return Err(error).with_context(|| {
                format!(
                    "Failed to inspect build transaction journal {}",
                    journal_path.display()
                )
            });
        }
    };
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!(
            "Build transaction journal must be a regular file: {}",
            journal_path.display()
        );
    }
    let serialized = std::fs::read_to_string(&journal_path).with_context(|| {
        format!(
            "Failed to read build transaction journal {}",
            journal_path.display()
        )
    })?;
    let journal: ArtifactTransactionJournal = toml::from_str(&serialized).with_context(|| {
        format!(
            "Failed to parse build transaction journal {}",
            journal_path.display()
        )
    })?;
    if journal.version != 1 {
        bail!(
            "Unsupported build transaction journal version {} in {}",
            journal.version,
            journal_path.display()
        );
    }

    let expected_destinations = expected_destinations
        .iter()
        .map(|path| path.to_path_buf())
        .collect::<HashSet<_>>();
    let mut destinations = HashSet::new();
    let mut backups = HashSet::new();
    for (index, entry) in journal.entries.iter().enumerate() {
        validate_journal_path(root, &entry.destination, "destination")?;
        validate_journal_path(root, &entry.backup, "backup")?;
        validate_transaction_backup_path(build_dir, &entry.backup, index)?;
        if !destinations.insert(entry.destination.clone()) {
            bail!(
                "Build transaction journal repeats destination {}",
                entry.destination.display()
            );
        }
        if !backups.insert(entry.backup.clone()) {
            bail!(
                "Build transaction journal repeats backup {}",
                entry.backup.display()
            );
        }
    }
    if destinations != expected_destinations {
        bail!("Build transaction destinations do not match the current artifact and lock set");
    }

    for entry in &journal.entries {
        match std::fs::symlink_metadata(&entry.backup) {
            Ok(backup_metadata) => {
                if backup_metadata.file_type().is_symlink() || !backup_metadata.is_file() {
                    bail!(
                        "Build transaction backup is not a regular file: {}",
                        entry.backup.display()
                    );
                }
                match std::fs::symlink_metadata(&entry.destination) {
                    Ok(destination_metadata) => {
                        if destination_metadata.file_type().is_symlink()
                            || !destination_metadata.is_file()
                        {
                            bail!(
                                "Cannot recover over non-regular build output {}",
                                entry.destination.display()
                            );
                        }
                        std::fs::remove_file(&entry.destination).with_context(|| {
                            format!(
                                "Failed to remove interrupted build output {}",
                                entry.destination.display()
                            )
                        })?;
                    }
                    Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                    Err(error) => {
                        return Err(error).with_context(|| {
                            format!(
                                "Failed to inspect build output {}",
                                entry.destination.display()
                            )
                        });
                    }
                }
                std::fs::rename(&entry.backup, &entry.destination).with_context(|| {
                    format!(
                        "Failed to restore {} from {}",
                        entry.destination.display(),
                        entry.backup.display()
                    )
                })?;
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                if !entry.destination_existed {
                    match std::fs::symlink_metadata(&entry.destination) {
                        Ok(destination_metadata) => {
                            if destination_metadata.file_type().is_symlink()
                                || !destination_metadata.is_file()
                            {
                                bail!(
                                    "Cannot recover over non-regular build output {}",
                                    entry.destination.display()
                                );
                            }
                            std::fs::remove_file(&entry.destination).with_context(|| {
                                format!(
                                    "Failed to remove interrupted new output {}",
                                    entry.destination.display()
                                )
                            })?;
                        }
                        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
                        Err(error) => {
                            return Err(error).with_context(|| {
                                format!(
                                    "Failed to inspect interrupted output {}",
                                    entry.destination.display()
                                )
                            });
                        }
                    }
                } else if !entry.destination.is_file() {
                    bail!(
                        "Cannot recover prior build output {}; both destination and backup are missing",
                        entry.destination.display()
                    );
                }
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to inspect build backup {}", entry.backup.display())
                });
            }
        }
    }

    std::fs::remove_file(&journal_path).with_context(|| {
        format!(
            "Failed to remove recovered build journal {}",
            journal_path.display()
        )
    })?;
    print_warning("Recovered the previous interrupted Warder artifact transaction");
    Ok(())
}

fn validate_journal_path(root: &Path, path: &Path, label: &str) -> Result<()> {
    let relative = path.strip_prefix(root).with_context(|| {
        format!(
            "Build transaction {} is outside the project: {}",
            label,
            path.display()
        )
    })?;
    if relative.as_os_str().is_empty() {
        bail!(
            "Build transaction {} is not a normalized project path: {}",
            label,
            path.display()
        );
    }
    let mut current = root.to_path_buf();
    let mut missing_ancestor = false;
    let components = relative.components().collect::<Vec<_>>();
    for (index, component) in components.iter().enumerate() {
        let Component::Normal(part) = component else {
            bail!(
                "Build transaction {} is not a normalized project path: {}",
                label,
                path.display()
            );
        };
        current.push(part);
        if missing_ancestor {
            continue;
        }
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() {
                    bail!(
                        "Build transaction {} must not traverse a symlink: {}",
                        label,
                        current.display()
                    );
                }
                if index + 1 != components.len() && !metadata.is_dir() {
                    bail!(
                        "Build transaction {} parent is not a directory: {}",
                        label,
                        current.display()
                    );
                }
                let canonical = std::fs::canonicalize(&current).with_context(|| {
                    format!(
                        "Failed to canonicalize build transaction {} {}",
                        label,
                        current.display()
                    )
                })?;
                if !canonical.starts_with(root) {
                    bail!(
                        "Build transaction {} escapes the project through {}",
                        label,
                        current.display()
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing_ancestor = true;
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!(
                        "Failed to inspect build transaction {} {}",
                        label,
                        current.display()
                    )
                });
            }
        }
    }
    Ok(())
}

fn validate_transaction_backup_path(build_dir: &Path, path: &Path, index: usize) -> Result<()> {
    let relative = path.strip_prefix(build_dir).with_context(|| {
        format!(
            "Build transaction backup is outside the output directory: {}",
            path.display()
        )
    })?;
    let components = relative.components().collect::<Vec<_>>();
    let valid = match components.as_slice() {
        [Component::Normal(staging), Component::Normal(backups), Component::Normal(file)] => {
            let staging = staging.to_str().unwrap_or_default();
            staging.starts_with(".warder-build-")
                && staging.len() > ".warder-build-".len()
                && *backups == std::ffi::OsStr::new("backups")
                && *file == std::ffi::OsStr::new(&format!("{index}.backup"))
        }
        _ => false,
    };
    if !valid {
        bail!(
            "Build transaction backup is not in a Warder staging backup slot: {}",
            path.display()
        );
    }
    Ok(())
}

fn validate_package_output_identity(name: &str, version: &str) -> Result<()> {
    if !is_portable_file_fragment(name) {
        bail!(
            "Invalid package.name '{}': expected a portable ASCII file-name fragment",
            name
        );
    }
    semver::Version::parse(version)
        .with_context(|| format!("Invalid package.version '{}': expected semver", version))?;
    if !is_portable_file_fragment(version) {
        bail!(
            "Invalid package.version '{}': expected a portable ASCII file-name fragment",
            version
        );
    }
    Ok(())
}

fn is_portable_file_fragment(value: &str) -> bool {
    !value.is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b'+'))
        && value != "."
        && value != ".."
}

fn checked_relative_components<'a>(
    path: &'a Path,
    label: &str,
) -> Result<Vec<&'a std::ffi::OsStr>> {
    if path.as_os_str().is_empty() {
        bail!("{} must not be empty", label);
    }
    let mut components = Vec::new();
    for component in path.components() {
        match component {
            Component::Normal(part) => components.push(part),
            _ => bail!(
                "{} must be a normalized relative path without '.', '..', roots, or prefixes: {}",
                label,
                path.display()
            ),
        }
    }
    Ok(components)
}

fn resolve_project_file(root: &Path, relative: &Path, label: &str) -> Result<PathBuf> {
    let components = checked_relative_components(relative, label)?;
    let mut current = root.to_path_buf();
    for component in components {
        current.push(component);
        let metadata = std::fs::symlink_metadata(&current)
            .with_context(|| format!("{} does not exist: {}", label, current.display()))?;
        if metadata.file_type().is_symlink() {
            bail!(
                "{} must not traverse a symlink: {}",
                label,
                current.display()
            );
        }
    }
    let canonical = std::fs::canonicalize(&current)
        .with_context(|| format!("Failed to canonicalize {}: {}", label, current.display()))?;
    if !canonical.starts_with(root) || !canonical.is_file() {
        bail!("{} must be a file inside {}", label, root.display());
    }
    Ok(canonical)
}

fn ensure_project_directory(root: &Path, relative: &Path, label: &str) -> Result<PathBuf> {
    let components = checked_relative_components(relative, label)?;
    let mut current = root.to_path_buf();
    for component in components {
        current.push(component);
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    bail!(
                        "{} must contain only real directories inside the project: {}",
                        label,
                        current.display()
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir(&current).with_context(|| {
                    format!("Failed to create {} directory {}", label, current.display())
                })?;
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to inspect {}", current.display()));
            }
        }
    }
    let canonical = std::fs::canonicalize(&current)
        .with_context(|| format!("Failed to canonicalize {}: {}", label, current.display()))?;
    if !canonical.starts_with(root) || canonical == root {
        bail!(
            "{} must resolve to a subdirectory inside {}",
            label,
            root.display()
        );
    }
    Ok(canonical)
}

fn commit_staged_files(
    files: &[(&Path, &Path)],
    backup_dir: &Path,
    journal_path: &Path,
) -> Result<()> {
    std::fs::create_dir_all(backup_dir).with_context(|| {
        format!(
            "Failed to create artifact backup directory {}",
            backup_dir.display()
        )
    })?;
    let mut staged_paths = HashSet::new();
    let mut destination_paths = HashSet::new();
    let mut destination_exists = Vec::with_capacity(files.len());
    for (staged, destination) in files {
        if !staged_paths.insert(staged.to_path_buf()) {
            bail!(
                "Staged build output is listed more than once: {}",
                staged.display()
            );
        }
        if !destination_paths.insert(destination.to_path_buf()) {
            bail!(
                "Build output destination is listed more than once: {}",
                destination.display()
            );
        }
        let staged_metadata = std::fs::symlink_metadata(staged)
            .with_context(|| format!("Staged build output is missing: {}", staged.display()))?;
        if staged_metadata.file_type().is_symlink() || !staged_metadata.is_file() {
            bail!(
                "Staged build output is not a regular file: {}",
                staged.display()
            );
        }
        match std::fs::symlink_metadata(destination) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_file() {
                    bail!(
                        "Refusing to replace non-regular build output {}",
                        destination.display()
                    );
                }
                destination_exists.push(true);
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                destination_exists.push(false);
            }
            Err(error) => {
                return Err(error).with_context(|| {
                    format!("Failed to inspect build output {}", destination.display())
                });
            }
        }
    }

    let mut backups = Vec::<(PathBuf, PathBuf)>::new();
    let journal = ArtifactTransactionJournal {
        version: 1,
        entries: files
            .iter()
            .enumerate()
            .map(|(index, (_, destination))| ArtifactTransactionEntry {
                destination: destination.to_path_buf(),
                backup: backup_dir.join(format!("{index}.backup")),
                destination_existed: destination_exists[index],
            })
            .collect(),
    };
    save_artifact_transaction_journal(&journal, journal_path)?;

    for (index, (_, destination)) in files.iter().enumerate() {
        if destination_exists[index] {
            let backup = backup_dir.join(format!("{index}.backup"));
            if let Err(error) = std::fs::rename(destination, &backup) {
                let rollback_errors = rollback_artifacts(&[], &backups);
                if rollback_errors.is_empty() {
                    remove_transaction_journal(journal_path)?;
                    bail!(
                        "Failed to stage existing output {} for replacement: {}",
                        destination.display(),
                        error
                    );
                }
                return Err(ArtifactRollbackFailure {
                    message: format!(
                        "Failed to stage existing output {} for replacement: {}{}",
                        destination.display(),
                        error,
                        format_rollback_errors(&rollback_errors)
                    ),
                }
                .into());
            }
            backups.push((destination.to_path_buf(), backup));
        }
    }

    let mut committed = Vec::new();
    for (staged, destination) in files {
        if let Err(error) = std::fs::rename(staged, destination) {
            let rollback_errors = rollback_artifacts(&committed, &backups);
            if rollback_errors.is_empty() {
                remove_transaction_journal(journal_path)?;
                bail!(
                    "Failed to commit staged output {} to {}: {}",
                    staged.display(),
                    destination.display(),
                    error
                );
            }
            return Err(ArtifactRollbackFailure {
                message: format!(
                    "Failed to commit staged output {} to {}: {}{}",
                    staged.display(),
                    destination.display(),
                    error,
                    format_rollback_errors(&rollback_errors)
                ),
            }
            .into());
        }
        committed.push(destination.to_path_buf());
    }

    if let Err(error) = remove_transaction_journal(journal_path) {
        let rollback_errors = rollback_artifacts(&committed, &backups);
        if rollback_errors.is_empty() {
            return Err(error.context(
                "Artifact transaction was rolled back because its journal could not be removed",
            ));
        }
        return Err(ArtifactRollbackFailure {
            message: format!(
                "Failed to remove the committed artifact transaction journal: {}{}",
                error,
                format_rollback_errors(&rollback_errors)
            ),
        }
        .into());
    }
    Ok(())
}

fn save_artifact_transaction_journal(
    journal: &ArtifactTransactionJournal,
    path: &Path,
) -> Result<()> {
    let serialized = toml::to_string(journal).context("Failed to serialize build transaction")?;
    let parent = path.parent().unwrap_or_else(|| Path::new("."));
    let mut staged = tempfile::NamedTempFile::new_in(parent).with_context(|| {
        format!(
            "Failed to stage build transaction journal next to {}",
            path.display()
        )
    })?;
    staged.write_all(serialized.as_bytes()).with_context(|| {
        format!(
            "Failed to write build transaction journal for {}",
            path.display()
        )
    })?;
    staged.as_file_mut().sync_all().with_context(|| {
        format!(
            "Failed to flush build transaction journal for {}",
            path.display()
        )
    })?;
    staged.persist(path).map(|_| ()).with_context(|| {
        format!(
            "Failed to publish build transaction journal {}",
            path.display()
        )
    })
}

fn remove_transaction_journal(path: &Path) -> Result<()> {
    std::fs::remove_file(path).with_context(|| {
        format!(
            "Failed to remove build transaction journal {}",
            path.display()
        )
    })
}

fn rollback_artifacts(committed: &[PathBuf], backups: &[(PathBuf, PathBuf)]) -> Vec<String> {
    let mut errors = Vec::new();
    for destination in committed.iter().rev() {
        if let Err(error) = std::fs::remove_file(destination) {
            errors.push(format!("remove {}: {}", destination.display(), error));
        }
    }
    for (destination, backup) in backups.iter().rev() {
        if let Err(error) = std::fs::rename(backup, destination) {
            errors.push(format!(
                "restore {} from {}: {}",
                destination.display(),
                backup.display(),
                error
            ));
        }
    }
    errors
}

fn format_rollback_errors(errors: &[String]) -> String {
    if errors.is_empty() {
        String::new()
    } else {
        format!("; rollback errors: {}", errors.join(", "))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn package_output_identity_accepts_portable_names_and_semver() {
        validate_package_output_identity("local-utils", "1.2.3-beta.1+build.7").unwrap();
    }

    #[test]
    fn package_output_identity_rejects_platform_specific_characters() {
        for name in ["foo:bar", "foo*bar", "foo\\bar", "日本語"] {
            assert!(
                validate_package_output_identity(name, "1.2.3").is_err(),
                "{name} should not be accepted as a portable artifact name"
            );
        }
    }

    #[test]
    fn artifact_commit_replaces_the_complete_set() {
        let temp = TempDir::new().unwrap();
        let staged_dir = temp.path().join("staged");
        let output_dir = temp.path().join("output");
        std::fs::create_dir_all(&staged_dir).unwrap();
        std::fs::create_dir_all(&output_dir).unwrap();
        let staged_a = staged_dir.join("a");
        let staged_b = staged_dir.join("b");
        let output_a = output_dir.join("a");
        let output_b = output_dir.join("b");
        std::fs::write(&staged_a, "new-a").unwrap();
        std::fs::write(&staged_b, "new-b").unwrap();
        std::fs::write(&output_a, "old-a").unwrap();
        std::fs::write(&output_b, "old-b").unwrap();

        commit_staged_files(
            &[
                (staged_a.as_path(), output_a.as_path()),
                (staged_b.as_path(), output_b.as_path()),
            ],
            &temp.path().join("backups"),
            &temp.path().join("transaction.toml"),
        )
        .unwrap();

        assert_eq!(std::fs::read_to_string(output_a).unwrap(), "new-a");
        assert_eq!(std::fs::read_to_string(output_b).unwrap(), "new-b");
    }

    #[test]
    fn artifact_commit_validates_every_destination_before_replacing_any() {
        let temp = TempDir::new().unwrap();
        let staged_dir = temp.path().join("staged");
        let output_dir = temp.path().join("output");
        std::fs::create_dir_all(&staged_dir).unwrap();
        std::fs::create_dir_all(&output_dir).unwrap();
        let staged_a = staged_dir.join("a");
        let staged_b = staged_dir.join("b");
        let output_a = output_dir.join("a");
        let invalid_output_b = output_dir.join("b");
        std::fs::write(&staged_a, "new-a").unwrap();
        std::fs::write(&staged_b, "new-b").unwrap();
        std::fs::write(&output_a, "old-a").unwrap();
        std::fs::create_dir(&invalid_output_b).unwrap();

        let error = commit_staged_files(
            &[
                (staged_a.as_path(), output_a.as_path()),
                (staged_b.as_path(), invalid_output_b.as_path()),
            ],
            &temp.path().join("backups"),
            &temp.path().join("transaction.toml"),
        )
        .unwrap_err();

        assert!(error
            .to_string()
            .contains("Refusing to replace non-regular"));
        assert_eq!(std::fs::read_to_string(output_a).unwrap(), "old-a");
        assert_eq!(std::fs::read_to_string(staged_a).unwrap(), "new-a");
    }

    #[test]
    fn artifact_rollback_restores_backups_after_partial_commit() {
        let temp = TempDir::new().unwrap();
        let output_a = temp.path().join("a");
        let output_b = temp.path().join("b");
        let backup_a = temp.path().join("a.backup");
        let backup_b = temp.path().join("b.backup");
        std::fs::write(&output_a, "new-a").unwrap();
        std::fs::write(&output_b, "new-b").unwrap();
        std::fs::write(&backup_a, "old-a").unwrap();
        std::fs::write(&backup_b, "old-b").unwrap();

        let errors = rollback_artifacts(
            &[output_a.clone(), output_b.clone()],
            &[(output_a.clone(), backup_a), (output_b.clone(), backup_b)],
        );

        assert!(errors.is_empty(), "{errors:?}");
        assert_eq!(std::fs::read_to_string(output_a).unwrap(), "old-a");
        assert_eq!(std::fs::read_to_string(output_b).unwrap(), "old-b");
    }

    #[test]
    fn interrupted_transaction_recovery_restores_old_files_and_removes_new_files() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().canonicalize().unwrap();
        let build_dir = root.join("dist");
        let staging = build_dir.join(".warder-build-test");
        let backups = staging.join("backups");
        let output_a = build_dir.join("a");
        let output_b = build_dir.join("b");
        let backup_a = backups.join("0.backup");
        let backup_b = backups.join("1.backup");
        std::fs::create_dir_all(&backups).unwrap();
        std::fs::write(&output_a, "new-a").unwrap();
        std::fs::write(&output_b, "new-b").unwrap();
        std::fs::write(&backup_a, "old-a").unwrap();
        let journal = ArtifactTransactionJournal {
            version: 1,
            entries: vec![
                ArtifactTransactionEntry {
                    destination: output_a.clone(),
                    backup: backup_a,
                    destination_existed: true,
                },
                ArtifactTransactionEntry {
                    destination: output_b.clone(),
                    backup: backup_b,
                    destination_existed: false,
                },
            ],
        };
        let journal_path = root.join(".warder-build-transaction.toml");
        save_artifact_transaction_journal(&journal, &journal_path).unwrap();

        recover_interrupted_artifact_transaction(
            &root,
            &build_dir,
            &[output_a.as_path(), output_b.as_path()],
        )
        .unwrap();

        assert_eq!(std::fs::read_to_string(output_a).unwrap(), "old-a");
        assert!(!output_b.exists());
        assert!(!journal_path.exists());
    }

    #[test]
    fn interrupted_transaction_rejects_paths_outside_the_project() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("project");
        std::fs::create_dir(&root).unwrap();
        let root = root.canonicalize().unwrap();
        let build_dir = root.join("dist");
        std::fs::create_dir(&build_dir).unwrap();
        let outside = temp.path().join("outside");
        std::fs::write(&outside, "keep").unwrap();
        let journal = ArtifactTransactionJournal {
            version: 1,
            entries: vec![ArtifactTransactionEntry {
                destination: outside.clone(),
                backup: build_dir.join(".warder-build-test/backups/0.backup"),
                destination_existed: true,
            }],
        };
        let journal_path = root.join(".warder-build-transaction.toml");
        save_artifact_transaction_journal(&journal, &journal_path).unwrap();

        let error =
            recover_interrupted_artifact_transaction(&root, &build_dir, &[outside.as_path()])
                .unwrap_err();

        assert!(error.to_string().contains("outside the project"));
        assert_eq!(std::fs::read_to_string(outside).unwrap(), "keep");
    }

    #[cfg(unix)]
    #[test]
    fn interrupted_transaction_does_not_follow_parent_symlinks() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let root = temp.path().join("project");
        let build_dir = root.join("dist");
        let outside_dir = temp.path().join("outside");
        std::fs::create_dir_all(&build_dir).unwrap();
        std::fs::create_dir(&outside_dir).unwrap();
        let root = root.canonicalize().unwrap();
        let build_dir = build_dir.canonicalize().unwrap();
        let outside = outside_dir.join("victim");
        std::fs::write(&outside, "keep").unwrap();
        symlink(&outside_dir, root.join("link")).unwrap();
        let destination = root.join("link/victim");
        let backup = build_dir.join(".warder-build-test/backups/0.backup");
        let journal = ArtifactTransactionJournal {
            version: 1,
            entries: vec![ArtifactTransactionEntry {
                destination: destination.clone(),
                backup,
                destination_existed: true,
            }],
        };
        let journal_path = root.join(".warder-build-transaction.toml");
        save_artifact_transaction_journal(&journal, &journal_path).unwrap();

        let error =
            recover_interrupted_artifact_transaction(&root, &build_dir, &[destination.as_path()])
                .unwrap_err();

        assert!(error.to_string().contains("must not traverse a symlink"));
        assert_eq!(std::fs::read_to_string(outside).unwrap(), "keep");
    }
}
