use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::io::Write;
use std::path::Path;

const VAULT_SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Visited,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Vault {
    pub version: u32,
    pub packages: BTreeMap<String, PackageLock>,
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageLock {
    pub version: String,
    pub source: LockSource,
    #[serde(default)]
    pub abi_hash: String,
    pub sha256: String,
    pub dependencies: BTreeMap<String, String>, // name -> version
}

#[derive(Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum LockSource {
    Registry { url: String },
    Path { path: String },
    Git { url: String, rev: String },
    Cage { path: String },
}

impl Vault {
    pub fn new() -> Self {
        Self {
            version: VAULT_SCHEMA_VERSION,
            packages: BTreeMap::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read vault from {:?}", path))?;
        toml::from_str(&content).with_context(|| format!("Failed to parse vault from {:?}", path))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self).context("Failed to serialize vault")?;
        let parent = path.parent().unwrap_or_else(|| Path::new("."));
        let mut staged = tempfile::NamedTempFile::new_in(parent)
            .with_context(|| format!("Failed to stage vault next to {:?}", path))?;
        staged
            .write_all(content.as_bytes())
            .with_context(|| format!("Failed to write staged vault for {:?}", path))?;
        staged
            .as_file_mut()
            .sync_all()
            .with_context(|| format!("Failed to flush staged vault for {:?}", path))?;
        staged
            .persist(path)
            .map(|_| ())
            .with_context(|| format!("Failed to atomically replace vault at {:?}", path))
    }

    pub fn add_package(&mut self, name: String, lock: PackageLock) {
        self.packages.insert(name, lock);
    }

    #[allow(dead_code)]
    pub fn remove_package(&mut self, name: &str) -> Option<PackageLock> {
        self.packages.remove(name)
    }

    #[allow(dead_code)]
    pub fn get_package(&self, name: &str) -> Option<&PackageLock> {
        self.packages.get(name)
    }

    pub fn verify_integrity(&self) -> Result<Vec<String>> {
        let mut errors = Vec::new();

        if self.version != VAULT_SCHEMA_VERSION {
            errors.push(format!(
                "Unsupported lock schema version {}: expected {}",
                self.version, VAULT_SCHEMA_VERSION
            ));
        }

        for (package_name, package) in &self.packages {
            if semver::Version::parse(&package.version).is_err() {
                errors.push(format!(
                    "Package '{}' version '{}' is not an exact semantic version",
                    package_name, package.version
                ));
            }

            if !is_canonical_lowercase_sha256(&package.sha256) {
                errors.push(format!(
                    "Package '{}' has a non-canonical SHA-256: expected 64 lowercase hexadecimal characters",
                    package_name
                ));
            }

            if !package.abi_hash.is_empty() && !is_canonical_lowercase_sha256(&package.abi_hash) {
                errors.push(format!(
                    "Package '{}' has a non-canonical ABI hash: expected an empty value or 64 lowercase hexadecimal characters",
                    package_name
                ));
            }

            for (dependency_name, dependency_version) in &package.dependencies {
                if semver::Version::parse(dependency_version).is_err() {
                    errors.push(format!(
                        "Package '{}' dependency '{}' version '{}' is not an exact semantic version",
                        package_name, dependency_name, dependency_version
                    ));
                }
            }
        }

        // Check for dependency cycles
        let cycles = self.find_cycles()?;
        for cycle in cycles {
            errors.push(format!("Dependency cycle detected: {}", cycle.join(" -> ")));
        }

        // Check for version conflicts
        let conflicts = self.find_version_conflicts()?;
        for (pkg, versions) in conflicts {
            errors.push(format!(
                "Version conflict for '{}': multiple versions required: {}",
                pkg,
                versions.join(", ")
            ));
        }

        Ok(errors)
    }

    fn find_cycles(&self) -> Result<Vec<Vec<String>>> {
        let mut states = HashMap::new();
        let mut stack = Vec::new();
        let mut cycles = Vec::new();
        let mut seen_cycles = HashSet::new();

        let mut package_names: Vec<_> = self.packages.keys().collect();
        package_names.sort();

        for name in package_names {
            self.visit_package_for_cycles(
                name,
                &mut states,
                &mut stack,
                &mut cycles,
                &mut seen_cycles,
            );
        }

        Ok(cycles)
    }

    fn visit_package_for_cycles(
        &self,
        name: &str,
        states: &mut HashMap<String, VisitState>,
        stack: &mut Vec<String>,
        cycles: &mut Vec<Vec<String>>,
        seen_cycles: &mut HashSet<String>,
    ) {
        match states.get(name) {
            Some(VisitState::Visiting) => {
                if let Some(start) = stack.iter().position(|pkg| pkg == name) {
                    let mut cycle = stack[start..].to_vec();
                    let key = normalized_cycle_key(&cycle);
                    cycle.push(name.to_string());

                    if seen_cycles.insert(key) {
                        cycles.push(cycle);
                    }
                }
                return;
            }
            Some(VisitState::Visited) => return,
            None => {}
        }

        states.insert(name.to_string(), VisitState::Visiting);
        stack.push(name.to_string());

        if let Some(package) = self.packages.get(name) {
            let mut dependencies: Vec<_> = package
                .dependencies
                .keys()
                .filter(|dep| self.packages.contains_key(*dep))
                .collect();
            dependencies.sort();

            for dependency in dependencies {
                self.visit_package_for_cycles(dependency, states, stack, cycles, seen_cycles);
            }
        }

        stack.pop();
        states.insert(name.to_string(), VisitState::Visited);
    }

    fn find_version_conflicts(&self) -> Result<BTreeMap<String, Vec<String>>> {
        let mut required_versions: BTreeMap<String, Vec<String>> = BTreeMap::new();

        for pkg_lock in self.packages.values() {
            for (dep_name, dep_version) in &pkg_lock.dependencies {
                required_versions
                    .entry(dep_name.clone())
                    .or_default()
                    .push(dep_version.clone());
            }
        }

        let mut conflicts = BTreeMap::new();
        for (pkg, versions) in required_versions {
            let unique_versions: Vec<String> = versions
                .into_iter()
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect();

            if unique_versions.len() > 1 {
                conflicts.insert(pkg, unique_versions);
            }
        }

        Ok(conflicts)
    }
}

fn is_canonical_lowercase_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || matches!(byte, b'a'..=b'f'))
}

fn normalized_cycle_key(cycle: &[String]) -> String {
    if cycle.is_empty() {
        return String::new();
    }

    let start = cycle
        .iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| left.cmp(right))
        .map(|(index, _)| index)
        .unwrap_or(0);

    cycle[start..]
        .iter()
        .chain(cycle[..start].iter())
        .map(String::as_str)
        .collect::<Vec<_>>()
        .join("\0")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn package_lock(dependencies: &[(&str, &str)]) -> PackageLock {
        PackageLock {
            version: "1.0.0".to_string(),
            source: LockSource::Registry {
                url: "https://registry.example.test".to_string(),
            },
            abi_hash: "b".repeat(64),
            sha256: "a".repeat(64),
            dependencies: dependencies
                .iter()
                .map(|(name, version)| ((*name).to_string(), (*version).to_string()))
                .collect(),
        }
    }

    #[test]
    fn verify_integrity_reports_dependency_cycle() {
        let mut vault = Vault::new();
        vault.add_package("a".to_string(), package_lock(&[("b", "1.0.0")]));
        vault.add_package("b".to_string(), package_lock(&[("c", "1.0.0")]));
        vault.add_package("c".to_string(), package_lock(&[("a", "1.0.0")]));

        let errors = vault.verify_integrity().unwrap();

        assert_eq!(errors, vec!["Dependency cycle detected: a -> b -> c -> a"]);
    }

    #[test]
    fn verify_integrity_ignores_dependencies_missing_from_vault() {
        let mut vault = Vault::new();
        vault.add_package(
            "a".to_string(),
            package_lock(&[("external-package", "1.0.0")]),
        );

        let errors = vault.verify_integrity().unwrap();

        assert!(errors.is_empty());
    }

    #[test]
    fn local_lock_serializes_empty_abi_hash_for_v1_readers() {
        let mut vault = Vault::new();
        vault.add_package(
            "local_utils".to_string(),
            PackageLock {
                version: "1.2.3".to_string(),
                source: LockSource::Path {
                    path: "../local-utils".to_string(),
                },
                abi_hash: String::new(),
                sha256: "abc123".to_string(),
                dependencies: BTreeMap::new(),
            },
        );

        let serialized = toml::to_string_pretty(&vault).unwrap();
        assert!(serialized.contains("abi_hash = \"\""), "{serialized}");
        assert_eq!(toml::from_str::<Vault>(&serialized).unwrap(), vault);
    }

    #[test]
    fn lock_without_abi_hash_loads_as_empty_for_forward_compatibility() {
        let serialized = r#"
version = 1

[packages.local_utils]
version = "1.2.3"
sha256 = "abc123"

[packages.local_utils.source]
type = "Path"
path = "../local-utils"

[packages.local_utils.dependencies]
"#;

        let vault: Vault = toml::from_str(serialized).unwrap();
        assert_eq!(vault.packages["local_utils"].abi_hash, "");
    }

    #[test]
    fn verify_integrity_rejects_malformed_schema_versions_and_hashes() {
        let mut vault = Vault::new();
        vault.version = 2;
        vault.add_package(
            "bad".to_string(),
            PackageLock {
                version: "^1.2.3".to_string(),
                source: LockSource::Path {
                    path: "../bad".to_string(),
                },
                abi_hash: "A".repeat(64),
                sha256: "ABCDEF".repeat(10) + "ABCD",
                dependencies: BTreeMap::from([("nested".to_string(), ">=2.0".to_string())]),
            },
        );

        let errors = vault.verify_integrity().unwrap();

        assert_eq!(
            errors,
            vec![
                "Unsupported lock schema version 2: expected 1",
                "Package 'bad' version '^1.2.3' is not an exact semantic version",
                "Package 'bad' has a non-canonical SHA-256: expected 64 lowercase hexadecimal characters",
                "Package 'bad' has a non-canonical ABI hash: expected an empty value or 64 lowercase hexadecimal characters",
                "Package 'bad' dependency 'nested' version '>=2.0' is not an exact semantic version",
            ]
        );
    }

    #[test]
    fn verify_integrity_accepts_exact_semver_and_canonical_hashes() {
        let mut vault = Vault::new();
        vault.add_package(
            "local_utils".to_string(),
            PackageLock {
                version: "1.2.3-beta.1+build.9".to_string(),
                source: LockSource::Path {
                    path: "../local-utils".to_string(),
                },
                abi_hash: String::new(),
                sha256: "0123456789abcdef".repeat(4),
                dependencies: BTreeMap::from([("nested".to_string(), "2.0.0".to_string())]),
            },
        );

        assert!(vault.verify_integrity().unwrap().is_empty());
    }
}
