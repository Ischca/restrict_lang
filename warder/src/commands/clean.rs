use super::build::checked_relative_components;
use super::{find_project_root, load_manifest, print_info, print_success};
use anyhow::{bail, Context, Result};
use std::path::{Path, PathBuf};

pub async fn clean_project() -> Result<()> {
    let root = find_project_root()?;
    let root = std::fs::canonicalize(&root)
        .with_context(|| format!("Failed to canonicalize project root {}", root.display()))?;
    let manifest = load_manifest()?;

    let mut removed = 0usize;

    if let Some(build_dir) = resolve_existing_project_dir(&root, Path::new(&manifest.build.output), "build.output")? {
        std::fs::remove_dir_all(&build_dir).with_context(|| {
            format!("Failed to remove build output directory {}", build_dir.display())
        })?;
        print_info(&format!("Removed {}", build_dir.display()));
        removed += 1;
    }

    if let Some(cache_dir) = resolve_existing_project_dir(&root, Path::new(".restrict-cache"), ".restrict-cache")? {
        std::fs::remove_dir_all(&cache_dir).with_context(|| {
            format!("Failed to remove cache directory {}", cache_dir.display())
        })?;
        print_info(&format!("Removed {}", cache_dir.display()));
        removed += 1;
    }

    for filename in [".warder-build-transaction.toml", ".warder-build.lock"] {
        let candidate = root.join(filename);
        if remove_regular_file_if_present(&candidate)? {
            print_info(&format!("Removed {}", candidate.display()));
            removed += 1;
        }
    }

    for entry in std::fs::read_dir(&root)
        .with_context(|| format!("Failed to read project root {}", root.display()))?
    {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if name_str.starts_with(".warder-build-") {
            let path = entry.path();
            let metadata = std::fs::symlink_metadata(&path)
                .with_context(|| format!("Failed to inspect {}", path.display()))?;
            if metadata.file_type().is_symlink() {
                continue;
            }
            if metadata.is_dir() {
                std::fs::remove_dir_all(&path)
                    .with_context(|| format!("Failed to remove {}", path.display()))?;
            } else if metadata.is_file() {
                std::fs::remove_file(&path)
                    .with_context(|| format!("Failed to remove {}", path.display()))?;
            }
            print_info(&format!("Removed {}", path.display()));
            removed += 1;
        }
    }

    if removed == 0 {
        print_success("Nothing to clean");
    } else {
        print_success(&format!("Cleaned {removed} build/staging artifact(s)"));
    }

    Ok(())
}

fn resolve_existing_project_dir(
    root: &Path,
    relative: &Path,
    label: &str,
) -> Result<Option<PathBuf>> {
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
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
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
    Ok(Some(canonical))
}

fn remove_regular_file_if_present(path: &Path) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                bail!("Refusing to remove non-regular file {}", path.display());
            }
            std::fs::remove_file(path)
                .with_context(|| format!("Failed to remove {}", path.display()))?;
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error).with_context(|| format!("Failed to inspect {}", path.display())),
    }
}
