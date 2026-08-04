use super::{find_project_root, load_manifest, print_info, print_success, save_manifest};
use crate::dependencies::{
    resolve_local_dependencies, validate_dependency_alias, validate_local_dependency_layout,
};
use crate::manifest::Dependency;
use anyhow::{bail, Result};
use semver::VersionReq;

pub async fn add_dependency(
    dep_spec: &str,
    path: Option<String>,
    git: Option<String>,
    wasm: Option<String>,
    wit: Option<String>,
) -> Result<()> {
    let mut manifest = load_manifest()?;

    let selected_sources = usize::from(path.is_some())
        + usize::from(git.is_some())
        + usize::from(wasm.is_some() || wit.is_some());
    if selected_sources > 1 {
        bail!("Choose exactly one dependency source; local --path cannot be combined with Git or foreign WASM options");
    }
    if wasm.is_some() != wit.is_some() {
        bail!("Foreign WASM dependencies require both --wasm and --wit, and are not buildable in v0.0.1");
    }

    // Parse dependency specification
    let (name, dependency) = if let Some(path) = path {
        // Local path dependency
        if dep_spec.contains('@') {
            bail!(
                "Local path dependencies use the version from their package.rl.toml; pass an alias without @version"
            );
        }
        let name = extract_name_from_spec(dep_spec)?;
        validate_dependency_alias(&name)?;
        (name, Dependency::Local { path })
    } else if let Some(git) = git {
        bail!(
            "Git dependency '{}' ({}) is unsupported by warder add in v0.0.1; only direct local --path dependencies are currently buildable",
            dep_spec,
            git
        )
    } else if let (Some(wasm), Some(wit)) = (wasm, wit) {
        bail!(
            "Foreign WASM dependency '{}' ({}, {}) is unsupported by warder add; use warder wrap for experimental local evaluation",
            dep_spec,
            wasm,
            wit
        )
    } else {
        let (_, requested) = parse_registry_dep(dep_spec)?;
        let Dependency::Version(version) = requested else {
            unreachable!();
        };
        bail!(
            "Registry dependency '{}' ({}) is unsupported by warder add in v0.0.1; only direct local --path dependencies are currently buildable",
            dep_spec,
            version
        )
    };

    // Check if dependency already exists
    if manifest.dependencies.contains_key(&name) {
        bail!("Dependency '{}' already exists", name);
    }

    // Add dependency
    manifest.add_dependency(name.clone(), dependency);
    let root = find_project_root()?;
    let (resolved_dependencies, _) = resolve_local_dependencies(&root, &manifest)?;
    validate_local_dependency_layout(&root, &manifest, &resolved_dependencies)?;
    save_manifest(&manifest)?;

    print_success(&format!("Added dependency '{}'", name));
    print_info("Run 'warder build' to compile the local dependency");

    Ok(())
}

pub fn remove_dependency(name: &str) -> Result<()> {
    let mut manifest = load_manifest()?;

    if manifest.remove_dependency(name).is_none() {
        bail!("Dependency '{}' not found", name);
    }

    save_manifest(&manifest)?;

    print_success(&format!("Removed dependency '{}'", name));
    print_info("Run 'warder build' to update the project");

    Ok(())
}

fn extract_name_from_spec(spec: &str) -> Result<String> {
    // Extract name from spec like "name@version" or just "name"
    Ok(spec.split('@').next().unwrap_or(spec).to_string())
}

#[cfg(test)]
fn parse_git_dep(spec: &str, git: String) -> Result<(String, Dependency)> {
    let (name, tag) = if let Some((name, tag)) = spec.split_once('@') {
        if tag.is_empty() {
            bail!("Invalid git tag specification '{}'. Use name@tag", spec);
        }

        (name.to_string(), Some(tag.to_string()))
    } else {
        (spec.to_string(), None)
    };

    Ok((name, Dependency::Git { git, tag }))
}

fn parse_registry_dep(spec: &str) -> Result<(String, Dependency)> {
    if let Some((name, version)) = spec.split_once('@') {
        // Validate version spec
        if !is_valid_version_spec(version) {
            bail!("Invalid version specification '{}'. Use semver format (e.g., '1.2.3', '^1.0.0', '~1.2', '1.*')", version);
        }
        Ok((name.to_string(), Dependency::Version(version.to_string())))
    } else {
        // No version specified, use latest
        Ok((spec.to_string(), Dependency::Version("*".to_string())))
    }
}

fn is_valid_version_spec(version: &str) -> bool {
    // Simple validation for common version patterns
    if version == "*" || version == "latest" {
        return true;
    }

    // Try parsing as version requirement
    if version.starts_with('^') || version.starts_with('~') || version.starts_with('=') {
        VersionReq::parse(version).is_ok()
    } else if version.contains('*') {
        // Handle patterns like "1.2.*"
        true
    } else {
        // Try parsing as exact version
        semver::Version::parse(version).is_ok()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_git_dep_reads_tag_from_spec() {
        let (name, dependency) =
            parse_git_dep("json@v1.2.3", "https://example.com/json.git".to_string()).unwrap();

        assert_eq!(name, "json");
        match dependency {
            Dependency::Git { git, tag } => {
                assert_eq!(git, "https://example.com/json.git");
                assert_eq!(tag.as_deref(), Some("v1.2.3"));
            }
            _ => panic!("expected git dependency"),
        }
    }

    #[test]
    fn parse_git_dep_without_tag_leaves_tag_empty() {
        let (name, dependency) =
            parse_git_dep("json", "https://example.com/json.git".to_string()).unwrap();

        assert_eq!(name, "json");
        match dependency {
            Dependency::Git { git, tag } => {
                assert_eq!(git, "https://example.com/json.git");
                assert_eq!(tag, None);
            }
            _ => panic!("expected git dependency"),
        }
    }

    #[test]
    fn parse_git_dep_rejects_empty_tag() {
        let err = parse_git_dep("json@", "https://example.com/json.git".to_string()).unwrap_err();

        assert!(err.to_string().contains("Invalid git tag specification"));
    }
}
