use super::{load_manifest, print_info, print_success, save_manifest};
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

    // Parse dependency specification
    let (name, dependency) = if let Some(path) = path {
        // Local path dependency
        let name = extract_name_from_spec(dep_spec)?;
        (name, Dependency::Local { path })
    } else if let Some(git) = git {
        // Git dependency
        parse_git_dep(dep_spec, git)?
    } else if let (Some(wasm), Some(wit)) = (wasm, wit) {
        // Foreign WASM dependency
        let name = extract_name_from_spec(dep_spec)?;
        (name, Dependency::Foreign { wasm, wit })
    } else {
        // Registry dependency (name@version)
        parse_registry_dep(dep_spec)?
    };

    // Check if dependency already exists
    if manifest.dependencies.contains_key(&name) {
        bail!("Dependency '{}' already exists", name);
    }

    // Add dependency
    manifest.add_dependency(name.clone(), dependency);
    save_manifest(&manifest)?;

    print_success(&format!("Added dependency '{}'", name));
    print_info("Run 'warder build' to download and build dependencies");

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
