use crate::manifest::{Dependency, Manifest};
use crate::vault::{LockSource, PackageLock, Vault};
use anyhow::{bail, Context, Result};
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, HashMap, HashSet};
use std::ffi::OsString;
use std::fs;
use std::path::{Component, Path, PathBuf};
use walkdir::WalkDir;

const LOCAL_SOURCE_HASH_VERSION: &[u8] = b"warder.local-source.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ResolvedLocalDependency {
    pub(crate) alias: String,
    pub(crate) package_root: PathBuf,
    pub(crate) source_dir: PathBuf,
}

impl ResolvedLocalDependency {
    pub(crate) fn module_root_arg(&self) -> OsString {
        let mut argument = OsString::from(&self.alias);
        argument.push("=");
        argument.push(&self.source_dir);
        argument
    }
}

pub(crate) fn validate_dependency_alias(alias: &str) -> Result<()> {
    if !restrict_lang::lexer::is_source_identifier(alias) {
        bail!(
            "Invalid dependency alias '{}': expected a non-keyword Restrict identifier (hyphens are not converted implicitly)",
            alias
        );
    }
    if alias == "std" {
        bail!("Dependency alias 'std' is reserved for the standard library");
    }
    Ok(())
}

pub(crate) fn resolve_local_dependencies(
    project_root: &Path,
    manifest: &Manifest,
) -> Result<(Vec<ResolvedLocalDependency>, Vault)> {
    let canonical_project_root = fs::canonicalize(project_root).with_context(|| {
        format!(
            "Failed to canonicalize project root {}",
            project_root.display()
        )
    })?;
    let mut aliases = manifest.dependencies.iter().collect::<Vec<_>>();
    aliases.sort_by(|left, right| left.0.cmp(right.0));

    let mut resolved = Vec::with_capacity(aliases.len());
    let mut vault = Vault::new();
    let mut source_roots = HashMap::<PathBuf, String>::new();

    for (alias, dependency) in aliases {
        validate_dependency_alias(alias)?;
        let path = match dependency {
            Dependency::Local { path } => path,
            Dependency::Version(version) => bail!(
                "Registry dependency '{}' ({}) is unsupported by warder build; v0.0.1 currently supports direct local path dependencies only",
                alias,
                version
            ),
            Dependency::Git { git, .. } => bail!(
                "Git dependency '{}' ({}) is unsupported by warder build; v0.0.1 currently supports direct local path dependencies only",
                alias,
                git
            ),
            Dependency::Foreign { wasm, .. } => bail!(
                "Foreign WASM dependency '{}' ({}) is unsupported by warder build; use warder wrap for experimental local evaluation",
                alias,
                wasm
            ),
        };

        let declared_path = Path::new(path);
        let dependency_root = if declared_path.is_absolute() {
            declared_path.to_path_buf()
        } else {
            canonical_project_root.join(declared_path)
        };
        let dependency_root = fs::canonicalize(&dependency_root).with_context(|| {
            format!(
                "Failed to resolve local dependency '{}' at {}",
                alias,
                dependency_root.display()
            )
        })?;
        if !dependency_root.is_dir() {
            bail!(
                "Local dependency '{}' is not a directory: {}",
                alias,
                dependency_root.display()
            );
        }
        if dependency_root == canonical_project_root {
            bail!(
                "Local dependency '{}' points back to the root project",
                alias
            );
        }

        let dependency_manifest_path = dependency_root.join("package.rl.toml");
        let dependency_manifest_metadata = fs::symlink_metadata(&dependency_manifest_path)
            .with_context(|| {
                format!(
                    "Local dependency '{}' is missing package.rl.toml at {}",
                    alias,
                    dependency_manifest_path.display()
                )
            })?;
        if dependency_manifest_metadata.file_type().is_symlink() {
            bail!(
                "Local dependency '{}' package.rl.toml must not be a symlink: {}",
                alias,
                dependency_manifest_path.display()
            );
        }
        if !dependency_manifest_metadata.is_file() {
            bail!(
                "Local dependency '{}' is missing package.rl.toml at {}",
                alias,
                dependency_manifest_path.display()
            );
        }
        let dependency_manifest = Manifest::load(&dependency_manifest_path)
            .with_context(|| format!("Failed to load local dependency '{}' manifest", alias))?;
        semver::Version::parse(&dependency_manifest.package.version).with_context(|| {
            format!(
                "Local dependency '{}' has invalid package version '{}'",
                alias, dependency_manifest.package.version
            )
        })?;
        if !dependency_manifest.dependencies.is_empty() {
            let mut transitive = dependency_manifest
                .dependencies
                .keys()
                .cloned()
                .collect::<Vec<_>>();
            transitive.sort();
            bail!(
                "Local dependency '{}' declares transitive dependencies ({}); transitive package graphs are outside the current direct-local v0.0.1 slice",
                alias,
                transitive.join(", ")
            );
        }

        let declared_source_dir = dependency_root.join("src");
        let source_metadata = fs::symlink_metadata(&declared_source_dir).with_context(|| {
            format!(
                "Local dependency '{}' is missing a readable src directory at {}",
                alias,
                declared_source_dir.display()
            )
        })?;
        if source_metadata.file_type().is_symlink() {
            bail!(
                "Local dependency '{}' src directory must not be a symlink: {}",
                alias,
                declared_source_dir.display()
            );
        }
        let source_dir = fs::canonicalize(&declared_source_dir).with_context(|| {
            format!(
                "Local dependency '{}' is missing a readable src directory at {}",
                alias,
                declared_source_dir.display()
            )
        })?;
        if !source_dir.is_dir() || !source_dir.starts_with(&dependency_root) {
            bail!(
                "Local dependency '{}' src directory escapes its package root: {}",
                alias,
                source_dir.display()
            );
        }
        validate_compiler_module_root_path(alias, &source_dir)?;

        let library_root = source_dir.join("lib.rl");
        if !library_root.is_file() {
            bail!(
                "Local dependency '{}' is missing its library root: {}",
                alias,
                library_root.display()
            );
        }
        let canonical_library_root = fs::canonicalize(&library_root).with_context(|| {
            format!(
                "Failed to canonicalize local dependency '{}' library root {}",
                alias,
                library_root.display()
            )
        })?;
        if !canonical_library_root.starts_with(&source_dir) {
            bail!(
                "Local dependency '{}' library root escapes {} through {}",
                alias,
                source_dir.display(),
                library_root.display()
            );
        }

        if let Some((existing_root, existing_alias)) = source_roots.iter().find(|(existing, _)| {
            source_dir.starts_with(existing) || existing.starts_with(&source_dir)
        }) {
            bail!(
                "Local dependency source {} for '{}' overlaps {} for '{}'; package source roots must be disjoint",
                source_dir.display(),
                alias,
                existing_root.display(),
                existing_alias
            );
        }
        source_roots.insert(source_dir.clone(), alias.clone());

        let source_hash = hash_local_package(&dependency_root, &source_dir)?;
        vault.add_package(
            alias.clone(),
            PackageLock {
                version: dependency_manifest.package.version,
                source: LockSource::Path { path: path.clone() },
                abi_hash: String::new(),
                sha256: source_hash,
                dependencies: BTreeMap::new(),
            },
        );
        resolved.push(ResolvedLocalDependency {
            alias: alias.clone(),
            package_root: dependency_root,
            source_dir,
        });
    }

    Ok((resolved, vault))
}

pub(crate) fn validate_local_dependency_layout(
    project_root: &Path,
    manifest: &Manifest,
    dependencies: &[ResolvedLocalDependency],
) -> Result<()> {
    let project_root = fs::canonicalize(project_root).with_context(|| {
        format!(
            "Failed to canonicalize project root {}",
            project_root.display()
        )
    })?;
    let entry =
        fs::canonicalize(project_root.join(&manifest.package.entry)).with_context(|| {
            format!(
                "Failed to resolve package.entry while validating dependency layout: {}",
                manifest.package.entry
            )
        })?;
    if !entry.is_file() || !entry.starts_with(&project_root) {
        bail!("package.entry must be a file inside the project");
    }
    let application_source = entry
        .parent()
        .context("package.entry must have a source directory")?;
    let build_output = prospective_project_directory(
        &project_root,
        Path::new(&manifest.build.output),
        "build.output",
    )?;

    if build_output.starts_with(application_source) || application_source.starts_with(&build_output)
    {
        bail!(
            "build.output {} overlaps application source {}; build and source roots must be disjoint",
            build_output.display(),
            application_source.display()
        );
    }
    for dependency in dependencies {
        if application_source.starts_with(&dependency.source_dir)
            || dependency.source_dir.starts_with(application_source)
        {
            bail!(
                "Application source {} overlaps local dependency source {} for '{}'; package source roots must be disjoint",
                application_source.display(),
                dependency.source_dir.display(),
                dependency.alias
            );
        }
        if build_output.starts_with(&dependency.source_dir)
            || dependency.source_dir.starts_with(&build_output)
        {
            bail!(
                "build.output {} overlaps local dependency source {} for '{}'; build and dependency source roots must be disjoint",
                build_output.display(),
                dependency.source_dir.display(),
                dependency.alias
            );
        }
    }
    Ok(())
}

fn prospective_project_directory(root: &Path, relative: &Path, label: &str) -> Result<PathBuf> {
    if relative.as_os_str().is_empty() {
        bail!("{} must not be empty", label);
    }
    let mut current = root.to_path_buf();
    let mut missing_ancestor = false;
    for component in relative.components() {
        let Component::Normal(part) = component else {
            bail!(
                "{} must be a normalized relative path without '.', '..', roots, or prefixes: {}",
                label,
                relative.display()
            );
        };
        current.push(part);
        if missing_ancestor {
            continue;
        }
        match fs::symlink_metadata(&current) {
            Ok(metadata) => {
                if metadata.file_type().is_symlink() || !metadata.is_dir() {
                    bail!(
                        "{} must contain only real directories inside the project: {}",
                        label,
                        current.display()
                    );
                }
                current = fs::canonicalize(&current).with_context(|| {
                    format!(
                        "Failed to canonicalize {} directory {}",
                        label,
                        current.display()
                    )
                })?;
                if !current.starts_with(root) {
                    bail!(
                        "{} escapes the project through {}",
                        label,
                        current.display()
                    );
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                missing_ancestor = true;
            }
            Err(error) => {
                return Err(error)
                    .with_context(|| format!("Failed to inspect {}", current.display()));
            }
        }
    }
    if current == root {
        bail!("{} must be a subdirectory inside the project", label);
    }
    Ok(current)
}

pub(crate) fn snapshot_local_dependencies(
    dependencies: &[ResolvedLocalDependency],
    vault: &Vault,
    snapshot_root: &Path,
) -> Result<Vec<ResolvedLocalDependency>> {
    let canonical_snapshot_root = prospective_canonical_snapshot_root(snapshot_root)?;
    for dependency in dependencies {
        if canonical_snapshot_root.starts_with(&dependency.source_dir) {
            bail!(
                "Dependency snapshot root must not be inside live source for '{}': {}",
                dependency.alias,
                snapshot_root.display()
            );
        }
    }
    fs::create_dir_all(snapshot_root).with_context(|| {
        format!(
            "Failed to create dependency snapshot root {}",
            snapshot_root.display()
        )
    })?;
    let mut snapshots = Vec::with_capacity(dependencies.len());
    let mut ordered_dependencies = dependencies.iter().collect::<Vec<_>>();
    ordered_dependencies.sort_by(|left, right| left.alias.cmp(&right.alias));

    for (slot, dependency) in ordered_dependencies.into_iter().enumerate() {
        let package_snapshot = snapshot_root.join(format!("{slot:04}"));
        let source_snapshot = package_snapshot.join("src");
        validate_compiler_module_root_path(&dependency.alias, &source_snapshot)?;
        fs::create_dir_all(&source_snapshot).with_context(|| {
            format!(
                "Failed to create snapshot directory for dependency '{}'",
                dependency.alias
            )
        })?;
        copy_regular_file(
            &dependency.package_root.join("package.rl.toml"),
            &package_snapshot.join("package.rl.toml"),
        )?;

        for entry in WalkDir::new(&dependency.source_dir).follow_links(false) {
            let entry = entry.with_context(|| {
                format!("Failed to snapshot local dependency '{}'", dependency.alias)
            })?;
            let path = entry.path();
            if entry.file_type().is_symlink() {
                bail!(
                    "Local dependency source changed to a symlink while snapshotting: {}",
                    path.display()
                );
            }
            let relative = path.strip_prefix(&dependency.source_dir).with_context(|| {
                format!(
                    "Dependency source {} escaped {} while snapshotting",
                    path.display(),
                    dependency.source_dir.display()
                )
            })?;
            let destination = source_snapshot.join(relative);
            if entry.file_type().is_dir() {
                fs::create_dir_all(&destination).with_context(|| {
                    format!(
                        "Failed to create snapshot directory {}",
                        destination.display()
                    )
                })?;
            } else if entry.file_type().is_file()
                && path.extension().and_then(|part| part.to_str()) == Some("rl")
            {
                copy_regular_file(path, &destination)?;
            }
        }

        let snapshot_hash = hash_local_package(&package_snapshot, &source_snapshot)?;
        let expected_hash = &vault
            .get_package(&dependency.alias)
            .with_context(|| {
                format!(
                    "Resolved dependency '{}' is missing from the lock plan",
                    dependency.alias
                )
            })?
            .sha256;
        if &snapshot_hash != expected_hash {
            bail!(
                "Local dependency '{}' changed while creating an immutable build snapshot",
                dependency.alias
            );
        }

        snapshots.push(ResolvedLocalDependency {
            alias: dependency.alias.clone(),
            package_root: package_snapshot,
            source_dir: source_snapshot,
        });
    }

    Ok(snapshots)
}

fn prospective_canonical_snapshot_root(snapshot_root: &Path) -> Result<PathBuf> {
    match fs::canonicalize(snapshot_root) {
        Ok(canonical) => Ok(canonical),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let parent = snapshot_root.parent().with_context(|| {
                format!(
                    "Dependency snapshot root has no parent: {}",
                    snapshot_root.display()
                )
            })?;
            let parent = if parent.as_os_str().is_empty() {
                Path::new(".")
            } else {
                parent
            };
            let file_name = snapshot_root.file_name().with_context(|| {
                format!(
                    "Dependency snapshot root has no final component: {}",
                    snapshot_root.display()
                )
            })?;
            let canonical_parent = fs::canonicalize(parent).with_context(|| {
                format!(
                    "Failed to canonicalize dependency snapshot parent {}",
                    parent.display()
                )
            })?;
            Ok(canonical_parent.join(file_name))
        }
        Err(error) => Err(error).with_context(|| {
            format!(
                "Failed to inspect dependency snapshot root {}",
                snapshot_root.display()
            )
        }),
    }
}

fn copy_regular_file(source: &Path, destination: &Path) -> Result<()> {
    let metadata = fs::symlink_metadata(source)
        .with_context(|| format!("Failed to inspect dependency file {}", source.display()))?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        bail!(
            "Dependency snapshot source must be a regular file: {}",
            source.display()
        );
    }
    fs::copy(source, destination).with_context(|| {
        format!(
            "Failed to copy dependency snapshot file {} to {}",
            source.display(),
            destination.display()
        )
    })?;
    Ok(())
}

fn validate_compiler_module_root_path(alias: &str, path: &Path) -> Result<()> {
    path.to_str().with_context(|| {
        format!(
            "Compiler module root for local dependency '{}' is not valid UTF-8: {:?}",
            alias, path
        )
    })?;
    Ok(())
}

fn hash_local_package(package_root: &Path, source_dir: &Path) -> Result<String> {
    let mut source_files = Vec::new();
    let mut seen_canonical_files = HashSet::new();
    let canonical_source_dir = fs::canonicalize(source_dir).with_context(|| {
        format!(
            "Failed to canonicalize local dependency source tree {}",
            source_dir.display()
        )
    })?;

    for entry in WalkDir::new(source_dir).follow_links(false) {
        let entry = entry.with_context(|| {
            format!(
                "Failed to inspect local dependency source tree {}",
                source_dir.display()
            )
        })?;
        let path = entry.path();
        if entry.file_type().is_symlink() {
            bail!(
                "Local dependency source tree must not contain symlinks: {}",
                path.display()
            );
        }
        if !path.is_file() || path.extension().and_then(|part| part.to_str()) != Some("rl") {
            continue;
        }

        let canonical = fs::canonicalize(path)
            .with_context(|| format!("Failed to canonicalize source file {}", path.display()))?;
        if !canonical.starts_with(&canonical_source_dir) {
            bail!(
                "Local dependency source escapes {} through {}",
                source_dir.display(),
                path.display()
            );
        }
        if !seen_canonical_files.insert(canonical) {
            bail!(
                "Local dependency source file is reachable through multiple paths: {}",
                path.display()
            );
        }
        let relative = package_relative_hash_path(package_root, path)?;
        source_files.push((relative, path.to_path_buf()));
    }
    source_files.sort_by(|left, right| left.0.cmp(&right.0));

    let mut hasher = Sha256::new();
    hash_frame(&mut hasher, LOCAL_SOURCE_HASH_VERSION);
    let manifest_path = package_root.join("package.rl.toml");
    hash_file(&mut hasher, package_root, &manifest_path)?;
    for (relative, source_file) in source_files {
        hash_file_at_relative_path(&mut hasher, &relative, &source_file)?;
    }

    Ok(hex::encode(hasher.finalize()))
}

fn hash_file(hasher: &mut Sha256, package_root: &Path, path: &Path) -> Result<()> {
    let relative = package_relative_hash_path(package_root, path)?;
    hash_file_at_relative_path(hasher, &relative, path)
}

fn package_relative_hash_path(package_root: &Path, path: &Path) -> Result<String> {
    let relative = path.strip_prefix(package_root).with_context(|| {
        format!(
            "Local dependency file {} is outside {}",
            path.display(),
            package_root.display()
        )
    })?;
    let mut normalized = Vec::new();
    for component in relative.components() {
        let Component::Normal(part) = component else {
            bail!(
                "Local dependency hash path must be normalized and relative: {:?}",
                relative
            );
        };
        let part = part.to_str().with_context(|| {
            format!(
                "Local dependency hash path is not valid UTF-8: {:?}",
                relative
            )
        })?;
        normalized.push(part);
    }
    if normalized.is_empty() {
        bail!("Local dependency hash path must not be empty");
    }
    Ok(normalized.join("/"))
}

fn hash_file_at_relative_path(hasher: &mut Sha256, relative: &str, path: &Path) -> Result<()> {
    let content = fs::read(path)
        .with_context(|| format!("Failed to read local dependency file {}", path.display()))?;
    hash_frame(hasher, relative.as_bytes());
    hash_frame(hasher, &content);
    Ok(())
}

fn hash_frame(hasher: &mut Sha256, bytes: &[u8]) {
    hasher.update((bytes.len() as u64).to_le_bytes());
    hasher.update(bytes);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn write_package(root: &Path, name: &str, version: &str, source: &str) {
        fs::create_dir_all(root.join("src")).unwrap();
        fs::write(
            root.join("package.rl.toml"),
            format!(
                "[package]\nname = \"{name}\"\nversion = \"{version}\"\nentry = \"src/main.rl\"\nedition = \"2025\"\n\n[dependencies]\n"
            ),
        )
        .unwrap();
        fs::write(root.join("src/lib.rl"), source).unwrap();
    }

    fn root_manifest(path: &str) -> Manifest {
        let mut manifest = Manifest::new("app");
        manifest.dependencies.insert(
            "local_utils".to_string(),
            Dependency::Local {
                path: path.to_string(),
            },
        );
        manifest
    }

    #[test]
    fn local_dependency_uses_manifest_version_and_source_hash() {
        let temp = TempDir::new().unwrap();
        let app = temp.path().join("app");
        let dependency = temp.path().join("local-utils");
        fs::create_dir_all(&app).unwrap();
        write_package(
            &dependency,
            "local-utils",
            "1.2.3",
            "pub fun score: () -> Int32 = { 7 }\n",
        );

        let (resolved, vault) =
            resolve_local_dependencies(&app, &root_manifest("../local-utils")).unwrap();

        assert_eq!(resolved.len(), 1);
        assert_eq!(resolved[0].alias, "local_utils");
        let package = vault.get_package("local_utils").unwrap();
        assert_eq!(package.version, "1.2.3");
        assert_eq!(package.abi_hash, "");
        assert_eq!(package.sha256.len(), 64);
        assert!(package.dependencies.is_empty());
    }

    #[test]
    fn local_dependency_hash_changes_with_source() {
        let temp = TempDir::new().unwrap();
        let app = temp.path().join("app");
        let dependency = temp.path().join("local-utils");
        fs::create_dir_all(&app).unwrap();
        write_package(
            &dependency,
            "local-utils",
            "1.2.3",
            "pub fun score: () -> Int32 = { 7 }\n",
        );
        let manifest = root_manifest("../local-utils");

        let (_, first) = resolve_local_dependencies(&app, &manifest).unwrap();
        fs::write(
            dependency.join("src/lib.rl"),
            "pub fun score: () -> Int32 = { 8 }\n",
        )
        .unwrap();
        let (_, second) = resolve_local_dependencies(&app, &manifest).unwrap();

        assert_ne!(
            first.get_package("local_utils").unwrap().sha256,
            second.get_package("local_utils").unwrap().sha256
        );
    }

    #[test]
    fn dependency_hash_paths_use_portable_separators() {
        let relative = package_relative_hash_path(
            Path::new("package"),
            &Path::new("package")
                .join("src")
                .join("nested")
                .join("detail.rl"),
        )
        .unwrap();

        assert_eq!(relative, "src/nested/detail.rl");
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_dependency_hash_paths_are_rejected() {
        use std::os::unix::ffi::OsStringExt;

        let package_root = Path::new("package");
        let source = package_root
            .join("src")
            .join(OsString::from_vec(vec![b'n', 0xff]))
            .join("detail.rl");
        let error = package_relative_hash_path(package_root, &source)
            .unwrap_err()
            .to_string();

        assert!(error.contains("hash path is not valid UTF-8"), "{error}");
    }

    #[cfg(unix)]
    #[test]
    fn non_utf8_compiler_module_root_is_rejected() {
        use std::os::unix::ffi::OsStringExt;

        let source_dir =
            PathBuf::from(OsString::from_vec(vec![b'/', b't', b'm', b'p', b'/', 0xff]));
        let error = validate_compiler_module_root_path("local_utils", &source_dir)
            .unwrap_err()
            .to_string();

        assert!(
            error.contains(
                "Compiler module root for local dependency 'local_utils' is not valid UTF-8"
            ),
            "{error}"
        );
    }

    #[test]
    fn transitive_dependency_is_rejected() {
        let temp = TempDir::new().unwrap();
        let app = temp.path().join("app");
        let dependency = temp.path().join("local-utils");
        fs::create_dir_all(&app).unwrap();
        write_package(
            &dependency,
            "local-utils",
            "1.2.3",
            "pub fun score: () -> Int32 = { 7 }\n",
        );
        fs::write(
            dependency.join("package.rl.toml"),
            "[package]\nname = \"local-utils\"\nversion = \"1.2.3\"\nentry = \"src/main.rl\"\nedition = \"2025\"\n\n[dependencies]\nnested = { path = \"../nested\" }\n",
        )
        .unwrap();

        let error = resolve_local_dependencies(&app, &root_manifest("../local-utils"))
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("transitive dependencies (nested)"),
            "{error}"
        );
    }

    #[test]
    fn snapshot_rejects_source_changes_after_lock_planning() {
        let temp = TempDir::new().unwrap();
        let app = temp.path().join("app");
        let dependency = temp.path().join("local-utils");
        fs::create_dir_all(&app).unwrap();
        write_package(
            &dependency,
            "local-utils",
            "1.2.3",
            "pub fun score: () -> Int32 = { 7 }\n",
        );
        let (resolved, vault) =
            resolve_local_dependencies(&app, &root_manifest("../local-utils")).unwrap();
        fs::write(
            dependency.join("src/lib.rl"),
            "pub fun score: () -> Int32 = { 8 }\n",
        )
        .unwrap();

        let error = snapshot_local_dependencies(&resolved, &vault, &temp.path().join("snapshot"))
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("changed while creating an immutable build snapshot"),
            "{error}"
        );
    }

    #[test]
    fn snapshot_root_inside_live_dependency_source_is_rejected_before_creation() {
        let temp = TempDir::new().unwrap();
        let app = temp.path().join("app");
        let dependency = temp.path().join("local-utils");
        fs::create_dir_all(&app).unwrap();
        write_package(
            &dependency,
            "local-utils",
            "1.2.3",
            "pub fun score: () -> Int32 = { 7 }\n",
        );
        let (resolved, vault) =
            resolve_local_dependencies(&app, &root_manifest("../local-utils")).unwrap();
        let snapshot_root = dependency.join("src/snapshots");

        let error = snapshot_local_dependencies(&resolved, &vault, &snapshot_root)
            .unwrap_err()
            .to_string();

        assert!(
            error.contains("Dependency snapshot root must not be inside live source"),
            "{error}"
        );
        assert!(!snapshot_root.exists());
    }

    #[test]
    fn snapshot_slots_do_not_use_case_distinct_aliases_as_paths() {
        let temp = TempDir::new().unwrap();
        let app = temp.path().join("app");
        let upper_dependency = temp.path().join("upper-package");
        let lower_dependency = temp.path().join("lower-package");
        fs::create_dir_all(&app).unwrap();
        write_package(
            &upper_dependency,
            "upper-package",
            "1.0.0",
            "pub val upper: Int32 = 1\n",
        );
        write_package(
            &lower_dependency,
            "lower-package",
            "1.0.0",
            "pub val lower: Int32 = 2\n",
        );
        let mut manifest = Manifest::new("app");
        manifest.dependencies.insert(
            "Foo".to_string(),
            Dependency::Local {
                path: "../upper-package".to_string(),
            },
        );
        manifest.dependencies.insert(
            "foo".to_string(),
            Dependency::Local {
                path: "../lower-package".to_string(),
            },
        );
        let (resolved, vault) = resolve_local_dependencies(&app, &manifest).unwrap();

        let snapshots =
            snapshot_local_dependencies(&resolved, &vault, &temp.path().join("snapshot")).unwrap();

        assert_eq!(
            snapshots
                .iter()
                .map(|dependency| dependency.alias.as_str())
                .collect::<Vec<_>>(),
            vec!["Foo", "foo"]
        );
        assert_eq!(
            snapshots
                .iter()
                .map(|dependency| {
                    dependency
                        .package_root
                        .file_name()
                        .unwrap()
                        .to_str()
                        .unwrap()
                })
                .collect::<Vec<_>>(),
            vec!["0000", "0001"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn local_dependency_source_symlinks_are_rejected() {
        use std::os::unix::fs::symlink;

        let temp = TempDir::new().unwrap();
        let app = temp.path().join("app");
        let dependency = temp.path().join("local-utils");
        fs::create_dir_all(&app).unwrap();
        write_package(
            &dependency,
            "local-utils",
            "1.2.3",
            "pub fun score: () -> Int32 = { 7 }\n",
        );
        fs::write(
            dependency.join("src/detail.rl"),
            "pub fun detail: () -> Int32 = { 1 }\n",
        )
        .unwrap();
        symlink(
            dependency.join("src/detail.rl"),
            dependency.join("src/alias.rl"),
        )
        .unwrap();

        let error = resolve_local_dependencies(&app, &root_manifest("../local-utils"))
            .unwrap_err()
            .to_string();

        assert!(error.contains("source tree must not contain symlinks"));
    }
}
