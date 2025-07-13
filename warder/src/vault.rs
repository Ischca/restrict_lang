use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

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
        toml::from_str(&content)
            .with_context(|| format!("Failed to parse vault from {:?}", path))
    }
    
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize vault")?;
        std::fs::write(path, content)
            .with_context(|| format!("Failed to write vault to {:?}", path))
    }
    
    pub fn add_package(&mut self, name: String, lock: PackageLock) {
        self.packages.insert(name, lock);
    }
    
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
        // TODO: Implement cycle detection using DFS
        Ok(Vec::new())
    }
    
    fn find_version_conflicts(&self) -> Result<HashMap<String, Vec<String>>> {
        let mut required_versions: HashMap<String, Vec<String>> = HashMap::new();
        
        for (_pkg_name, pkg_lock) in &self.packages {
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