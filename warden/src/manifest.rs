use anyhow::{Result, Context};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Serialize, Deserialize)]
pub struct Manifest {
    pub package: Package,
    #[serde(default)]
    pub dependencies: HashMap<String, Dependency>,
    #[serde(default)]
    pub build: Build,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Package {
    pub name: String,
    pub version: String,
    pub entry: String,
    pub edition: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authors: Option<Vec<String>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Dependency {
    Version(String),
    Local { path: String },
    Git { git: String, #[serde(skip_serializing_if = "Option::is_none")] tag: Option<String> },
    Foreign { wasm: String, wit: String },
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Build {
    #[serde(default = "default_target")]
    pub target: String,
    #[serde(default = "default_optimize")]
    pub optimize: bool,
    #[serde(default = "default_output")]
    pub output: String,
}

impl Default for Build {
    fn default() -> Self {
        Self {
            target: default_target(),
            optimize: default_optimize(),
            output: default_output(),
        }
    }
}

fn default_target() -> String {
    "wasm32".to_string()
}

fn default_optimize() -> bool {
    true
}

fn default_output() -> String {
    "dist/".to_string()
}

impl Manifest {
    pub fn new(name: &str) -> Self {
        Self {
            package: Package {
                name: name.to_string(),
                version: "0.1.0".to_string(),
                entry: "src/main.rl".to_string(),
                edition: "2025".to_string(),
                authors: None,
                description: None,
            },
            dependencies: HashMap::new(),
            build: Build::default(),
        }
    }
    
    pub fn load(path: &Path) -> Result<Self> {
        let content = std::fs::read_to_string(path)
            .with_context(|| format!("Failed to read manifest from {:?}", path))?;
        toml::from_str(&content)
            .with_context(|| format!("Failed to parse manifest from {:?}", path))
    }
    
    pub fn save(&self, path: &Path) -> Result<()> {
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize manifest")?;
        std::fs::write(path, content)
            .with_context(|| format!("Failed to write manifest to {:?}", path))
    }
    
    pub fn add_dependency(&mut self, name: String, dep: Dependency) {
        self.dependencies.insert(name, dep);
    }
    
    pub fn remove_dependency(&mut self, name: &str) -> Option<Dependency> {
        self.dependencies.remove(name)
    }
}

#[derive(Debug, Serialize, Deserialize)]
pub struct CageManifest {
    pub name: String,
    pub version: String,
    pub sha256: String,
    pub freeze_ts: u64,
    pub abi_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wit_exports: Option<Vec<String>>,
}

impl CageManifest {
    pub fn new(name: String, version: String) -> Self {
        use std::time::{SystemTime, UNIX_EPOCH};
        
        Self {
            name,
            version,
            sha256: String::new(),
            freeze_ts: SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            abi_hash: String::new(),
            wit_exports: None,
        }
    }
}