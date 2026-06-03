use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::Path;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum VisitState {
    Visiting,
    Visited,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Vault {
    pub version: u32,
    pub packages: HashMap<String, PackageLock>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PackageLock {
    pub version: String,
    pub source: LockSource,
    pub abi_hash: String,
    pub sha256: String,
    pub dependencies: HashMap<String, String>, // name -> version
}

#[derive(Debug, Serialize, Deserialize)]
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
            version: 1,
            packages: HashMap::new(),
        }
    }

    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read vault from {:?}", path))?;
        toml::from_str(&content).with_context(|| format!("Failed to parse vault from {:?}", path))
    }

    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self).context("Failed to serialize vault")?;
        std::fs::write(path, content)
            .with_context(|| format!("Failed to write vault to {:?}", path))
    }

    pub fn add_package(&mut self, name: String, lock: PackageLock) {
        self.packages.insert(name, lock);
    }

    #[allow(dead_code)]
    pub fn remove_package(&mut self, name: &str) -> Option<PackageLock> {
        self.packages.remove(name)
    }

    pub fn get_package(&self, name: &str) -> Option<&PackageLock> {
        self.packages.get(name)
    }

    pub fn verify_integrity(&self) -> Result<Vec<String>> {
        let mut errors = Vec::new();

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

    fn find_version_conflicts(&self) -> Result<HashMap<String, Vec<String>>> {
        let mut required_versions: HashMap<String, Vec<String>> = HashMap::new();

        for pkg_lock in self.packages.values() {
            for (dep_name, dep_version) in &pkg_lock.dependencies {
                required_versions
                    .entry(dep_name.clone())
                    .or_default()
                    .push(dep_version.clone());
            }
        }

        let mut conflicts = HashMap::new();
        for (pkg, versions) in required_versions {
            let unique_versions: Vec<String> = versions
                .into_iter()
                .collect::<std::collections::HashSet<_>>()
                .into_iter()
                .collect();

            if unique_versions.len() > 1 {
                conflicts.insert(pkg, unique_versions);
            }
        }

        Ok(conflicts)
    }
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
            abi_hash: "abi".to_string(),
            sha256: "sha".to_string(),
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
}
