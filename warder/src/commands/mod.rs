use anyhow::{bail, Context, Result};
use colored::*;
use std::path::{Path, PathBuf};

mod add;
mod build;
mod doctor;
mod init;
mod new;
mod publish;
mod run;
mod test;
mod wrap;

pub use add::{add_dependency, remove_dependency};
pub use build::build_project;
pub use doctor::doctor_check;
pub use init::init_project;
pub use new::new_project;
pub use publish::publish_package;
pub use run::run_project;
pub use test::test_project;
pub use wrap::{unwrap_cage, wrap_wasm};

use crate::manifest::{
    Manifest, LEGACY_LOCK_FILENAME, LEGACY_MANIFEST_FILENAME, WARDER_LOCK_FILENAME,
    WARDER_MANIFEST_FILENAME,
};

pub fn find_project_root() -> Result<PathBuf> {
    let current = std::env::current_dir()?;
    find_project_root_from(&current)
}

pub(crate) fn find_project_root_from(start: &Path) -> Result<PathBuf> {
    find_enclosing_project_root(start)?.with_context(|| {
        format!(
            "Not in a Restrict Language project (no {} found)",
            WARDER_MANIFEST_FILENAME
        )
    })
}

pub(crate) fn find_enclosing_project_root(start: &Path) -> Result<Option<PathBuf>> {
    let mut path = start;
    let mut found_root: Option<PathBuf> = None;

    loop {
        let manifest_path = path.join(WARDER_MANIFEST_FILENAME);
        let legacy_manifest_path = path.join(LEGACY_MANIFEST_FILENAME);
        let has_manifest = inspect_manifest_candidate(&manifest_path)?;
        let has_legacy_manifest = inspect_manifest_candidate(&legacy_manifest_path)?;

        if has_manifest && has_legacy_manifest {
            bail!(
                "Conflicting Warder manifests at {}: remove {} after migrating it to {}",
                path.display(),
                LEGACY_MANIFEST_FILENAME,
                WARDER_MANIFEST_FILENAME
            );
        }
        if has_manifest {
            reject_legacy_lock(path)?;
            if let Some(inner_root) = &found_root {
                bail!(
                    "Multiple Warder roots exist inside one repository boundary: {} and {}. Keep one repository-root {} and remove the nested manifest",
                    inner_root.display(),
                    path.display(),
                    WARDER_MANIFEST_FILENAME
                );
            }
            found_root = Some(path.to_path_buf());
        }
        if has_legacy_manifest {
            bail!(
                "Legacy manifest {} is no longer authoritative; migrate it to {} and, if present, migrate {} to {}",
                legacy_manifest_path.display(),
                manifest_path.display(),
                LEGACY_LOCK_FILENAME,
                WARDER_LOCK_FILENAME
            );
        }

        if has_git_boundary(path)? {
            return Ok(found_root);
        }

        match path.parent() {
            Some(parent) => path = parent,
            None => return Ok(found_root),
        }
    }
}

pub(crate) fn reject_legacy_lock(root: &Path) -> Result<()> {
    let legacy_lock_path = root.join(LEGACY_LOCK_FILENAME);
    let has_legacy_lock = inspect_regular_candidate(&legacy_lock_path, "Legacy dependency lock")?;
    if !has_legacy_lock {
        return Ok(());
    }

    let lock_path = root.join(WARDER_LOCK_FILENAME);
    let has_lock = inspect_regular_candidate(&lock_path, "Dependency lock")?;
    if has_lock {
        bail!(
            "Conflicting dependency locks at {}: remove {} after explicitly migrating it to {}",
            root.display(),
            LEGACY_LOCK_FILENAME,
            WARDER_LOCK_FILENAME
        );
    }
    bail!(
        "Legacy dependency lock {} is no longer authoritative; explicitly migrate it to {} or remove it before Warder writes a lock",
        legacy_lock_path.display(),
        lock_path.display()
    );
}

fn has_git_boundary(path: &Path) -> Result<bool> {
    let git_path = path.join(".git");
    match std::fs::symlink_metadata(&git_path) {
        Ok(_) => Ok(true),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => Err(error)
            .with_context(|| format!("Failed to inspect VCS boundary {}", git_path.display())),
    }
}

fn inspect_manifest_candidate(path: &Path) -> Result<bool> {
    inspect_regular_candidate(path, "Project manifest")
}

fn inspect_regular_candidate(path: &Path, label: &str) -> Result<bool> {
    match std::fs::symlink_metadata(path) {
        Ok(metadata) => {
            if metadata.file_type().is_symlink() || !metadata.is_file() {
                bail!(
                    "{} must be a regular file, not a symlink or directory: {}",
                    label,
                    path.display()
                );
            }
            Ok(true)
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(error) => {
            Err(error).with_context(|| format!("Failed to inspect {} {}", label, path.display()))
        }
    }
}

pub fn load_manifest() -> Result<Manifest> {
    let root = find_project_root()?;
    let manifest_path = root.join(WARDER_MANIFEST_FILENAME);
    Manifest::load(&manifest_path)
}

pub fn save_manifest(manifest: &Manifest) -> Result<()> {
    let root = find_project_root()?;
    let manifest_path = root.join(WARDER_MANIFEST_FILENAME);
    manifest.save(&manifest_path)
}

pub fn print_success(message: &str) {
    println!("{} {}", "✓".green().bold(), message);
}

pub fn print_error(message: &str) {
    eprintln!("{} {}", "✗".red().bold(), message);
}

pub fn print_info(message: &str) {
    println!("{} {}", "ℹ".blue().bold(), message);
}

pub fn print_warning(message: &str) {
    println!("{} {}", "⚠".yellow().bold(), message);
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn root_discovery_finds_warder_manifest_from_a_nested_directory() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("project");
        let nested = root.join("src/nested");
        std::fs::create_dir_all(&nested).unwrap();
        std::fs::write(root.join(WARDER_MANIFEST_FILENAME), "").unwrap();

        assert_eq!(find_project_root_from(&nested).unwrap(), root);
    }

    #[test]
    fn root_discovery_rejects_dual_and_legacy_only_manifests() {
        let temp = TempDir::new().unwrap();
        let dual = temp.path().join("dual");
        std::fs::create_dir_all(&dual).unwrap();
        std::fs::write(dual.join(WARDER_MANIFEST_FILENAME), "").unwrap();
        std::fs::write(dual.join(LEGACY_MANIFEST_FILENAME), "").unwrap();

        let dual_error = find_project_root_from(&dual).unwrap_err().to_string();
        assert!(dual_error.contains("Conflicting Warder manifests"));

        let legacy = temp.path().join("legacy");
        std::fs::create_dir_all(&legacy).unwrap();
        std::fs::write(legacy.join(LEGACY_MANIFEST_FILENAME), "").unwrap();

        let legacy_error = find_project_root_from(&legacy).unwrap_err().to_string();
        assert!(legacy_error.contains("Legacy manifest"));
        assert!(legacy_error.contains(WARDER_MANIFEST_FILENAME));
    }

    #[test]
    fn root_discovery_rejects_legacy_lock_with_or_without_the_new_lock() {
        let temp = TempDir::new().unwrap();
        let root = temp.path().join("project");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join(WARDER_MANIFEST_FILENAME), "").unwrap();
        std::fs::write(root.join(LEGACY_LOCK_FILENAME), "").unwrap();

        let legacy_error = find_project_root_from(&root).unwrap_err().to_string();
        assert!(legacy_error.contains("Legacy dependency lock"));
        assert!(legacy_error.contains(WARDER_LOCK_FILENAME));

        std::fs::write(root.join(WARDER_LOCK_FILENAME), "").unwrap();
        let conflict_error = find_project_root_from(&root).unwrap_err().to_string();
        assert!(conflict_error.contains("Conflicting dependency locks"));
    }

    #[test]
    fn root_discovery_does_not_cross_a_git_boundary() {
        let temp = TempDir::new().unwrap();
        std::fs::write(temp.path().join(WARDER_MANIFEST_FILENAME), "").unwrap();
        let repository = temp.path().join("independent");
        let nested = repository.join("src");
        std::fs::create_dir_all(repository.join(".git")).unwrap();
        std::fs::create_dir_all(&nested).unwrap();

        let error = find_project_root_from(&nested).unwrap_err().to_string();
        assert!(error.contains("Not in a Restrict Language project"));
    }

    #[test]
    fn root_discovery_rejects_a_nested_manifest_inside_one_git_boundary() {
        let temp = TempDir::new().unwrap();
        let outer = temp.path().join("repository");
        let inner = outer.join("packages/child");
        std::fs::create_dir_all(outer.join(".git")).unwrap();
        std::fs::create_dir_all(&inner).unwrap();
        std::fs::write(outer.join(WARDER_MANIFEST_FILENAME), "").unwrap();
        std::fs::write(inner.join(WARDER_MANIFEST_FILENAME), "").unwrap();

        let error = find_project_root_from(&inner).unwrap_err().to_string();
        assert!(error.contains("Multiple Warder roots"));
        assert!(error.contains(&outer.display().to_string()));
        assert!(error.contains(&inner.display().to_string()));
    }

    #[test]
    fn root_discovery_allows_an_independent_nested_git_repository() {
        let temp = TempDir::new().unwrap();
        let outer = temp.path().join("repository");
        let inner = outer.join("vendor/independent");
        let inner_source = inner.join("src");
        std::fs::create_dir_all(outer.join(".git")).unwrap();
        std::fs::create_dir_all(inner.join(".git")).unwrap();
        std::fs::create_dir_all(&inner_source).unwrap();
        std::fs::write(outer.join(WARDER_MANIFEST_FILENAME), "").unwrap();
        std::fs::write(inner.join(WARDER_MANIFEST_FILENAME), "").unwrap();

        assert_eq!(find_project_root_from(&inner_source).unwrap(), inner);
    }
}
